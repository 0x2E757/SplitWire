//! XAML event subscriptions: a delegate object of our own per handler, removed on drop. Handlers
//! never unwind across the ABI.

use std::ptr;
use std::rc::Rc;
use std::sync::atomic::{AtomicU32, Ordering};

use super::winrt::*;
use crate::platform::abi::{Guid, Raw};
use crate::platform::com::{
    Com, ComResult, E_FAIL, E_NOINTERFACE, E_POINTER, INSPECTABLE, UNKNOWN, UnknownVtbl, check,
    factory,
};

type Action = Box<dyn Fn(Raw, Raw) -> ComResult<()>>;

/// A delegate: `IUnknown` plus `Invoke(sender, args)`.
#[repr(C)]
struct Vtable {
    base: UnknownVtbl,
    invoke: unsafe extern "system" fn(Raw, Raw, Raw) -> i32,
}

#[repr(C)]
struct Handler {
    vtable: &'static Vtable,
    refs: AtomicU32,
    iid: Guid,
    action: Action,
}

unsafe extern "system" fn query(raw: Raw, iid: *const Guid, out: *mut Raw) -> i32 {
    if iid.is_null() || out.is_null() {
        return E_POINTER;
    }
    // SAFETY: `raw` is one of our handlers; `iid` and `out` were checked.
    unsafe {
        *out = ptr::null_mut();
        let handler = &*(raw as *const Handler);
        if *iid != UNKNOWN && *iid != handler.iid {
            return E_NOINTERFACE;
        }
        *out = raw;
        add_ref(raw);
    }
    0
}

unsafe extern "system" fn add_ref(raw: Raw) -> u32 {
    // SAFETY: `raw` is one of our handlers.
    unsafe {
        (*(raw as *const Handler))
            .refs
            .fetch_add(1, Ordering::Relaxed)
            + 1
    }
}

unsafe extern "system" fn release(raw: Raw) -> u32 {
    // SAFETY: `raw` is one of our handlers, freed with its last reference.
    unsafe {
        let count = (*(raw as *const Handler))
            .refs
            .fetch_sub(1, Ordering::AcqRel)
            - 1;
        if count == 0 {
            drop(Box::from_raw(raw as *mut Handler));
        }
        count
    }
}

unsafe extern "system" fn invoke(raw: Raw, sender: Raw, args: Raw) -> i32 {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        // SAFETY: `raw` is one of our handlers.
        unsafe { ((*(raw as *const Handler)).action)(sender, args) }
    }))
    .unwrap_or(Err(E_FAIL))
    .err()
    .unwrap_or(0)
}

static VTABLE: Vtable = Vtable {
    base: UnknownVtbl {
        query,
        add_ref,
        release,
    },
    invoke,
};

fn handler(iid: Guid, action: impl Fn(Raw, Raw) -> ComResult<()> + 'static) -> ComResult<Com> {
    let handler = Box::new(Handler {
        vtable: &VTABLE,
        refs: AtomicU32::new(1),
        iid,
        action: Box::new(action),
    });
    // SAFETY: a fresh object with one reference, handed to `Com`.
    unsafe { Com::owned(Box::into_raw(handler).cast()) }
}

/// `IReference<PointerEventHandler>`: `UIElement.AddHandler` takes the delegate boxed.
#[repr(C)]
struct BoxVtable {
    base: UnknownVtbl,
    iids: unsafe extern "system" fn(Raw, *mut u32, *mut Raw) -> i32,
    class_name: unsafe extern "system" fn(Raw, *mut Raw) -> i32,
    trust: unsafe extern "system" fn(Raw, *mut i32) -> i32,
    value: unsafe extern "system" fn(Raw, *mut Raw) -> i32,
}

#[repr(C)]
struct Boxed {
    vtable: &'static BoxVtable,
    refs: AtomicU32,
    value: Com,
}

unsafe extern "system" fn box_query(raw: Raw, iid: *const Guid, out: *mut Raw) -> i32 {
    if iid.is_null() || out.is_null() {
        return E_POINTER;
    }
    // SAFETY: `raw` is one of our boxes; `iid` and `out` were checked.
    unsafe {
        *out = ptr::null_mut();
        if ![UNKNOWN, INSPECTABLE, POINTER_HANDLER_REFERENCE].contains(&*iid) {
            return E_NOINTERFACE;
        }
        *out = raw;
        box_add(raw);
    }
    0
}

unsafe extern "system" fn box_add(raw: Raw) -> u32 {
    // SAFETY: `raw` is one of our boxes.
    unsafe {
        (*(raw as *const Boxed))
            .refs
            .fetch_add(1, Ordering::Relaxed)
            + 1
    }
}

unsafe extern "system" fn box_release(raw: Raw) -> u32 {
    // SAFETY: `raw` is one of our boxes, freed with its last reference.
    unsafe {
        let count = (*(raw as *const Boxed)).refs.fetch_sub(1, Ordering::AcqRel) - 1;
        if count == 0 {
            drop(Box::from_raw(raw as *mut Boxed));
        }
        count
    }
}

unsafe extern "system" fn box_iids(_: Raw, count: *mut u32, iids: *mut Raw) -> i32 {
    // SAFETY: the caller passes valid out pointers.
    unsafe {
        *count = 0;
        *iids = ptr::null_mut();
    }
    0
}

unsafe extern "system" fn box_class_name(_: Raw, name: *mut Raw) -> i32 {
    // SAFETY: the caller passes a valid out pointer.
    unsafe {
        *name = ptr::null_mut();
    }
    0
}

unsafe extern "system" fn box_trust(_: Raw, level: *mut i32) -> i32 {
    // SAFETY: the caller passes a valid out pointer.
    unsafe {
        *level = 0;
    }
    0
}

unsafe extern "system" fn box_value(raw: Raw, value: *mut Raw) -> i32 {
    // SAFETY: `raw` is one of our boxes; the caller gets its own reference.
    unsafe {
        let inner = (*(raw as *const Boxed)).value.clone();
        *value = inner.raw();
        std::mem::forget(inner);
    }
    0
}

static BOX_VTABLE: BoxVtable = BoxVtable {
    base: UnknownVtbl {
        query: box_query,
        add_ref: box_add,
        release: box_release,
    },
    iids: box_iids,
    class_name: box_class_name,
    trust: box_trust,
    value: box_value,
};

enum Registration {
    Token {
        source: Com,
        remove: usize,
        token: i64,
    },
    Routed {
        source: Com,
        event: Com,
        handler: Com,
    },
}

impl Drop for Registration {
    fn drop(&mut self) {
        // SAFETY: removes exactly what was added, through the matching slot.
        unsafe {
            match self {
                Self::Token {
                    source,
                    remove,
                    token,
                } => {
                    let remove: unsafe extern "system" fn(Raw, i64) -> i32 = source.slot(*remove);
                    let _ = remove(source.raw(), *token);
                }
                Self::Routed {
                    source,
                    event,
                    handler,
                } => {
                    let remove: unsafe extern "system" fn(Raw, Raw, Raw) -> i32 =
                        source.slot(UI_ELEMENT_REMOVE_HANDLER);
                    let _ = remove(source.raw(), event.raw(), handler.raw());
                }
            }
        }
    }
}

/// An event handler while it is held.
#[derive(Clone)]
pub struct Subscription {
    _registration: Rc<Registration>,
}

impl Subscription {
    fn add(
        source: Com,
        iid: Guid,
        add: usize,
        remove: usize,
        action: impl Fn(Raw, Raw) -> ComResult<()> + 'static,
    ) -> ComResult<Self> {
        let handler = handler(iid, action)?;
        // SAFETY: `add` is the slot of an `add_X(handler, *token)` of `source`.
        unsafe {
            let add: unsafe extern "system" fn(Raw, Raw, *mut i64) -> i32 = source.slot(add);
            let mut token = 0;
            check(add(source.raw(), handler.raw(), &mut token))?;
            Ok(Self {
                _registration: Rc::new(Registration::Token {
                    source,
                    remove,
                    token,
                }),
            })
        }
    }

    pub fn click(source: &Com, action: impl Fn() -> ComResult<()> + 'static) -> ComResult<Self> {
        Self::add(
            source.query(&BUTTON_BASE)?,
            ROUTED_EVENT_HANDLER,
            BUTTON_ADD_CLICK,
            BUTTON_REMOVE_CLICK,
            move |_, _| action(),
        )
    }

    pub fn menu_click(
        source: &Com,
        action: impl Fn() -> ComResult<()> + 'static,
    ) -> ComResult<Self> {
        Self::add(
            source.query(&MENU_ITEM)?,
            ROUTED_EVENT_HANDLER,
            MENU_ITEM_ADD_CLICK,
            MENU_ITEM_REMOVE_CLICK,
            move |_, _| action(),
        )
    }

    pub fn toggled(source: &Com, action: impl Fn() -> ComResult<()> + 'static) -> ComResult<Self> {
        Self::add(
            source.query(&TOGGLE_SWITCH)?,
            ROUTED_EVENT_HANDLER,
            TOGGLE_SWITCH_ADD_TOGGLED,
            TOGGLE_SWITCH_REMOVE_TOGGLED,
            move |_, _| action(),
        )
    }

    pub fn got_focus(
        source: &Com,
        action: impl Fn() -> ComResult<()> + 'static,
    ) -> ComResult<Self> {
        Self::add(
            source.query(&UI_ELEMENT)?,
            ROUTED_EVENT_HANDLER,
            UI_ELEMENT_ADD_GOT_FOCUS,
            UI_ELEMENT_REMOVE_GOT_FOCUS,
            move |_, _| action(),
        )
    }

    pub fn lost_focus(
        source: &Com,
        action: impl Fn() -> ComResult<()> + 'static,
    ) -> ComResult<Self> {
        Self::add(
            source.query(&UI_ELEMENT)?,
            ROUTED_EVENT_HANDLER,
            UI_ELEMENT_ADD_LOST_FOCUS,
            UI_ELEMENT_REMOVE_LOST_FOCUS,
            move |_, _| action(),
        )
    }

    /// Enter in a field; the key is handled so a multi-line field does not also get it.
    pub fn enter(source: &Com, action: impl Fn() -> ComResult<()> + 'static) -> ComResult<Self> {
        Self::add(
            source.query(&UI_ELEMENT)?,
            KEY_EVENT_HANDLER,
            UI_ELEMENT_ADD_KEY_DOWN,
            UI_ELEMENT_REMOVE_KEY_DOWN,
            move |_, args| {
                // SAFETY: the args of a KeyDown handler are borrowed for the call.
                let args = unsafe { Com::borrowed(args)? }.query(&KEY_ARGS)?;
                if args.value::<i32>(KEY_ARGS_KEY)? == 13 {
                    args.set_value(KEY_ARGS_PUT_HANDLED, 1u8)?;
                    action()?;
                }
                Ok(())
            },
        )
    }

    /// DragEnter, DragOver, DragLeave or Drop (`add` and `remove` slots of `IUIElement`).
    pub fn drag(
        source: &Com,
        add: usize,
        remove: usize,
        action: impl Fn(Com) -> ComResult<()> + 'static,
    ) -> ComResult<Self> {
        Self::add(
            source.query(&UI_ELEMENT)?,
            DRAG_EVENT_HANDLER,
            add,
            remove,
            move |_, args| {
                // SAFETY: the args of a drag handler are borrowed for the call.
                action(unsafe { Com::borrowed(args)? })
            },
        )
    }

    /// A pointer event of `source` and its children, also when a child handled it (a Button
    /// handles its own presses); `event` is the `IUIElementStatics` getter of the routed event.
    pub fn pointer(
        source: &Com,
        event: usize,
        action: impl Fn(Com) -> ComResult<()> + 'static,
    ) -> ComResult<Self> {
        let source = source.query(&UI_ELEMENT)?;
        let statics = factory("Windows.UI.Xaml.UIElement", &UI_ELEMENT_STATICS)?;
        let inner = handler(POINTER_EVENT_HANDLER, move |_, args| {
            // SAFETY: the args of a pointer handler are borrowed for the call.
            action(unsafe { Com::borrowed(args)? })
        })?;
        let boxed = Box::new(Boxed {
            vtable: &BOX_VTABLE,
            refs: AtomicU32::new(1),
            value: inner,
        });
        // SAFETY: a fresh box with one reference; the getters and AddHandler follow the SDK.
        unsafe {
            let handler = Com::owned(Box::into_raw(boxed).cast())?;
            let event = statics.object(event)?;
            let add: unsafe extern "system" fn(Raw, Raw, Raw, u8) -> i32 =
                source.slot(UI_ELEMENT_ADD_HANDLER);
            check(add(source.raw(), event.raw(), handler.raw(), 1))?;
            Ok(Self {
                _registration: Rc::new(Registration::Routed {
                    source,
                    event,
                    handler,
                }),
            })
        }
    }

    /// The pointer's position relative to `element`, from pointer event args.
    pub fn position(args: &Com, element: &Com) -> ComResult<(f32, f32)> {
        #[repr(C)]
        #[derive(Clone, Copy, Default)]
        struct Point {
            x: f32,
            y: f32,
        }
        let args = args.query(&POINTER_ARGS)?;
        let element = element.query(&UI_ELEMENT)?;
        // SAFETY: GetCurrentPoint(relativeTo) returns a PointerPoint; its Position is a Point.
        unsafe {
            let get: unsafe extern "system" fn(Raw, Raw, *mut Raw) -> i32 =
                args.slot(POINTER_ARGS_CURRENT_POINT);
            let mut point = ptr::null_mut();
            check(get(args.raw(), element.raw(), &mut point))?;
            let point = Com::owned(point)?.query(&POINTER_POINT)?;
            let position = point.value::<Point>(POINTER_POINT_POSITION)?;
            Ok((position.x, position.y))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn delegates_invoke_from_their_slot() {
        let size = std::mem::size_of::<usize>();
        assert_eq!(std::mem::offset_of!(Vtable, invoke) / size, HANDLER_INVOKE);
        assert_eq!(HANDLER_INVOKE, POINTER_HANDLER_INVOKE);
        assert_eq!(KEY_HANDLER_INVOKE, DRAG_HANDLER_INVOKE);
        assert_eq!(std::mem::offset_of!(BoxVtable, value) / size, 6);
    }

    #[test]
    fn a_handler_counts_its_references_and_runs_its_action() {
        let ran = Rc::new(std::cell::Cell::new(0));
        let seen = ran.clone();
        let handler = handler(ROUTED_EVENT_HANDLER, move |_, _| {
            seen.set(seen.get() + 1);
            Ok(())
        })
        .unwrap();
        let again = handler.query(&ROUTED_EVENT_HANDLER).unwrap();
        assert!(handler.query(&POINTER_EVENT_HANDLER).is_err());
        // SAFETY: slot 3 is Invoke(sender, args).
        unsafe {
            let call: unsafe extern "system" fn(Raw, Raw, Raw) -> i32 = again.slot(HANDLER_INVOKE);
            assert_eq!(call(again.raw(), ptr::null_mut(), ptr::null_mut()), 0);
        }
        assert_eq!(ran.get(), 1);
    }
}
