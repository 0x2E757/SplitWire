//! The tray icon's own hidden window: `Shell_NotifyIconW` reports clicks to a window, and only a
//! top-level window hears "TaskbarCreated" when Explorer starts again. Its procedure only queues
//! what happened; the frame loop takes the events.

use std::cell::{Cell, RefCell};
use std::io;
use std::ptr;

use super::abi::RegisterWindowMessageW;
use crate::platform::abi::{Raw, wide};
use crate::platform::window::native_window::WindowClass;

/// What the user or the shell did to the icon.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TrayEvent {
    /// A left click: the window opens or comes to the front.
    Open,
    /// A right click: the menu.
    Menu,
    /// Explorer started again: the icon must be added again.
    TaskbarCreated,
}

thread_local! {
    static EVENTS: RefCell<Vec<TrayEvent>> = const { RefCell::new(Vec::new()) };
    static TASKBAR_CREATED: Cell<u32> = const { Cell::new(0) };
}

pub struct MessageWindow(Raw);

impl MessageWindow {
    const CLASS: &'static str = "SplitWire.Tray";
    /// The icon's callback message, `WM_APP + 1`.
    pub const CALLBACK: u32 = 0x8001;

    pub fn new() -> io::Result<Self> {
        let name = wide(Self::CLASS);
        // SAFETY: plain Win32 calls with NUL-terminated strings; the window is never shown.
        unsafe {
            let instance = GetModuleHandleW(ptr::null());
            let class = WindowClass {
                size: std::mem::size_of::<WindowClass>() as u32,
                style: 0,
                procedure: Self::procedure,
                class_extra: 0,
                window_extra: 0,
                instance,
                icon: ptr::null_mut(),
                cursor: ptr::null_mut(),
                background: ptr::null_mut(),
                menu: ptr::null(),
                name: name.as_ptr(),
                small_icon: ptr::null_mut(),
            };
            // A second registration (the check builds the tray twice) fails harmlessly.
            RegisterClassExW(&class);
            TASKBAR_CREATED
                .with(|m| m.set(RegisterWindowMessageW(wide("TaskbarCreated").as_ptr())));
            let window = CreateWindowExW(
                0,
                name.as_ptr(),
                wide("SplitWire").as_ptr(),
                0,
                0,
                0,
                0,
                0,
                ptr::null_mut(),
                ptr::null_mut(),
                instance,
                ptr::null_mut(),
            );
            if window.is_null() {
                return Err(io::Error::last_os_error());
            }
            Ok(Self(window))
        }
    }

    pub fn raw(&self) -> Raw {
        self.0
    }

    /// What happened since the last call.
    pub fn take_events() -> Vec<TrayEvent> {
        EVENTS.with(|events| std::mem::take(&mut *events.borrow_mut()))
    }

    unsafe extern "system" fn procedure(
        window: Raw,
        message: u32,
        wparam: usize,
        lparam: isize,
    ) -> isize {
        const WM_LBUTTONUP: isize = 0x0202;
        const WM_RBUTTONUP: isize = 0x0205;
        let event = match message {
            // Without NOTIFYICON_VERSION_4, lParam is the mouse message itself.
            Self::CALLBACK => match lparam & 0xFFFF {
                WM_LBUTTONUP => Some(TrayEvent::Open),
                WM_RBUTTONUP => Some(TrayEvent::Menu),
                _ => None,
            },
            _ if message != 0 && message == TASKBAR_CREATED.with(Cell::get) => {
                Some(TrayEvent::TaskbarCreated)
            }
            _ => None,
        };
        match event {
            Some(event) => {
                EVENTS.with(|events| events.borrow_mut().push(event));
                0
            }
            // SAFETY: everything else is the default procedure's.
            None => unsafe { DefWindowProcW(window, message, wparam, lparam) },
        }
    }
}

impl Drop for MessageWindow {
    fn drop(&mut self) {
        // SAFETY: the window is ours.
        unsafe {
            DestroyWindow(self.0);
        }
    }
}

#[link(name = "kernel32")]
unsafe extern "system" {
    fn GetModuleHandleW(name: *const u16) -> Raw;
}

#[link(name = "user32")]
unsafe extern "system" {
    fn RegisterClassExW(class: *const WindowClass) -> u16;
    fn CreateWindowExW(
        ex: u32,
        class: *const u16,
        title: *const u16,
        style: u32,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
        parent: Raw,
        menu: Raw,
        instance: Raw,
        param: Raw,
    ) -> Raw;
    fn DefWindowProcW(hwnd: Raw, message: u32, wparam: usize, lparam: isize) -> isize;
    fn DestroyWindow(hwnd: Raw) -> i32;
}
