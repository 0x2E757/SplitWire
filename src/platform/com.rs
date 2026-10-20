//! Small owning COM and HSTRING wrappers (TaskbarMetrics' approach). A `Com` is one reference to
//! an interface; it is neither `Send` nor `Sync`, because XAML objects stay on their thread.
//! Vtable slots are the SDK's ABI order, checked by `tools/verify-abi.ps1`.

use std::marker::PhantomData;
use std::ptr;
use std::rc::Rc;

use super::abi::{Guid, Raw};

pub type Hr = i32;
pub type ComResult<T> = Result<T, Hr>;

pub const E_FAIL: Hr = 0x8000_4005_u32 as i32;
pub const E_POINTER: Hr = 0x8000_4003_u32 as i32;
pub const E_NOINTERFACE: Hr = 0x8000_4002_u32 as i32;

pub fn check(hr: Hr) -> ComResult<()> {
    if hr < 0 { Err(hr) } else { Ok(()) }
}

/// `IUnknown`.
pub const UNKNOWN: Guid = Guid::from_u128(0x00000000_0000_0000_c000_000000000046);
/// `IInspectable`.
pub const INSPECTABLE: Guid = Guid::from_u128(0xaf86e2e0_b12d_4c6a_9c5a_d7aa65101e90);

/// The `IUnknown` part of a vtable.
#[repr(C)]
pub struct UnknownVtbl {
    pub query: unsafe extern "system" fn(Raw, *const Guid, *mut Raw) -> Hr,
    pub add_ref: unsafe extern "system" fn(Raw) -> u32,
    pub release: unsafe extern "system" fn(Raw) -> u32,
}

pub struct Com {
    raw: Raw,
    _thread: PhantomData<Rc<()>>,
}

impl Com {
    /// Takes ownership of one reference; null is `E_POINTER`.
    ///
    /// # Safety
    /// `raw` must be a COM interface pointer whose reference the caller hands over.
    pub unsafe fn owned(raw: Raw) -> ComResult<Self> {
        if raw.is_null() {
            Err(E_POINTER)
        } else {
            Ok(Self {
                raw,
                _thread: PhantomData,
            })
        }
    }

    /// Takes a new reference to a borrowed pointer, such as an event's sender.
    ///
    /// # Safety
    /// `raw` must be a live COM interface pointer.
    pub unsafe fn borrowed(raw: Raw) -> ComResult<Self> {
        if raw.is_null() {
            return Err(E_POINTER);
        }
        // SAFETY: a live interface pointer starts with its vtable; slot 1 is AddRef.
        unsafe {
            let table = *(raw as *const *const Raw);
            let add: unsafe extern "system" fn(Raw) -> u32 = std::mem::transmute(*table.add(1));
            add(raw);
            Self::owned(raw)
        }
    }

    pub fn raw(&self) -> Raw {
        self.raw
    }

    /// The function at `index` of the vtable.
    ///
    /// # Safety
    /// `T` must be the exact `extern "system"` signature of that slot in the SDK.
    pub unsafe fn slot<T: Copy>(&self, index: usize) -> T {
        assert_eq!(std::mem::size_of::<T>(), std::mem::size_of::<Raw>());
        // SAFETY: the object is live while `self` holds its reference.
        unsafe {
            let table = *(self.raw as *const *const Raw);
            std::mem::transmute_copy(&*table.add(index))
        }
    }

    pub fn query(&self, iid: &Guid) -> ComResult<Self> {
        // SAFETY: slot 0 is QueryInterface.
        unsafe {
            let call: unsafe extern "system" fn(Raw, *const Guid, *mut Raw) -> Hr = self.slot(0);
            let mut result = ptr::null_mut();
            check(call(self.raw, iid, &mut result))?;
            Self::owned(result)
        }
    }

    /// A getter at `slot` that returns an interface.
    pub fn object(&self, slot: usize) -> ComResult<Self> {
        // SAFETY: the caller names a getter slot of this interface.
        unsafe {
            let call: unsafe extern "system" fn(Raw, *mut Raw) -> Hr = self.slot(slot);
            let mut result = ptr::null_mut();
            check(call(self.raw, &mut result))?;
            Self::owned(result)
        }
    }

    /// A getter at `slot` that returns an HSTRING.
    pub fn string(&self, slot: usize) -> ComResult<String> {
        // SAFETY: the caller names an HSTRING getter slot of this interface.
        unsafe {
            let call: unsafe extern "system" fn(Raw, *mut Raw) -> Hr = self.slot(slot);
            let mut result = ptr::null_mut();
            check(call(self.raw, &mut result))?;
            Ok(HString(result).text())
        }
    }

    /// A setter at `slot` that takes an HSTRING.
    pub fn set_string(&self, slot: usize, value: &str) -> ComResult<()> {
        let value = HString::new(value)?;
        // SAFETY: the caller names an HSTRING setter slot of this interface.
        unsafe {
            let call: unsafe extern "system" fn(Raw, Raw) -> Hr = self.slot(slot);
            check(call(self.raw, value.0))
        }
    }

    /// A getter at `slot` of a plain value (`bool` as `u8`, `f64`, `i32`).
    pub fn value<T: Default + Copy>(&self, slot: usize) -> ComResult<T> {
        // SAFETY: the caller names a getter slot returning `T`.
        unsafe {
            let call: unsafe extern "system" fn(Raw, *mut T) -> Hr = self.slot(slot);
            let mut value = T::default();
            check(call(self.raw, &mut value))?;
            Ok(value)
        }
    }

    /// A setter at `slot` of a plain value.
    pub fn set_value<T: Copy>(&self, slot: usize, value: T) -> ComResult<()> {
        // SAFETY: the caller names a setter slot taking `T`.
        unsafe {
            let call: unsafe extern "system" fn(Raw, T) -> Hr = self.slot(slot);
            check(call(self.raw, value))
        }
    }
}

impl Clone for Com {
    fn clone(&self) -> Self {
        // SAFETY: slot 1 is AddRef; the new value owns that reference.
        unsafe {
            let call: unsafe extern "system" fn(Raw) -> u32 = self.slot(1);
            call(self.raw);
        }
        Self {
            raw: self.raw,
            _thread: PhantomData,
        }
    }
}

impl Drop for Com {
    fn drop(&mut self) {
        // SAFETY: slot 2 is Release of the reference this value owns.
        unsafe {
            let call: unsafe extern "system" fn(Raw) -> u32 = self.slot(2);
            call(self.raw);
        }
    }
}

/// An owned HSTRING.
pub struct HString(pub Raw);

impl HString {
    pub fn new(text: &str) -> ComResult<Self> {
        let units: Vec<u16> = text.encode_utf16().collect();
        let mut result = ptr::null_mut();
        // SAFETY: the buffer and its length describe `units`.
        unsafe {
            check(WindowsCreateString(
                units.as_ptr(),
                units.len() as u32,
                &mut result,
            ))?;
        }
        Ok(Self(result))
    }

    pub fn text(&self) -> String {
        let mut length = 0;
        // SAFETY: the buffer lives as long as the string; an empty string may be null.
        unsafe {
            let raw = WindowsGetStringRawBuffer(self.0, &mut length);
            if length == 0 || raw.is_null() {
                String::new()
            } else {
                String::from_utf16_lossy(std::slice::from_raw_parts(raw, length as usize))
            }
        }
    }
}

impl Drop for HString {
    fn drop(&mut self) {
        // SAFETY: the string is owned; deleting null is allowed.
        unsafe {
            WindowsDeleteString(self.0);
        }
    }
}

/// A new instance of a runtime class, as `iid`.
pub fn activate(class: &str, iid: &Guid) -> ComResult<Com> {
    let name = HString::new(class)?;
    let mut result = ptr::null_mut();
    // SAFETY: the class name is a valid HSTRING; the result is one owned reference.
    unsafe {
        check(RoActivateInstance(name.0, &mut result))?;
        Com::owned(result)?.query(iid)
    }
}

/// A runtime class's activation factory or statics interface.
pub fn factory(class: &str, iid: &Guid) -> ComResult<Com> {
    let name = HString::new(class)?;
    let mut result = ptr::null_mut();
    // SAFETY: as `activate`.
    unsafe {
        check(RoGetActivationFactory(name.0, iid, &mut result))?;
        Com::owned(result)
    }
}

/// A classic COM object of `class`, as `iid`, in this process.
pub fn create(class: &Guid, iid: &Guid) -> ComResult<Com> {
    let mut result = ptr::null_mut();
    // SAFETY: CLSCTX_INPROC_SERVER; the result is one owned reference.
    unsafe {
        check(CoCreateInstance(
            class,
            ptr::null_mut(),
            1,
            iid,
            &mut result,
        ))?;
        Com::owned(result)
    }
}

/// The calling thread joined to the single-threaded apartment (XAML needs one); left on drop.
pub struct Apartment(PhantomData<Rc<()>>);

impl Apartment {
    pub fn single_threaded() -> ComResult<Self> {
        // SAFETY: RO_INIT_SINGLETHREADED; undone by Drop on the same thread.
        unsafe {
            check(RoInitialize(0))?;
        }
        Ok(Self(PhantomData))
    }
}

impl Drop for Apartment {
    fn drop(&mut self) {
        // SAFETY: pairs the RoInitialize of `single_threaded` on this thread.
        unsafe {
            RoUninitialize();
        }
    }
}

#[link(name = "runtimeobject")]
unsafe extern "system" {
    fn RoInitialize(kind: u32) -> Hr;
    fn RoUninitialize();
    fn RoActivateInstance(name: Raw, result: *mut Raw) -> Hr;
    fn RoGetActivationFactory(name: Raw, iid: *const Guid, result: *mut Raw) -> Hr;
    fn WindowsCreateString(text: *const u16, length: u32, result: *mut Raw) -> Hr;
    fn WindowsDeleteString(text: Raw) -> Hr;
    fn WindowsGetStringRawBuffer(text: Raw, length: *mut u32) -> *const u16;
}

#[link(name = "ole32")]
unsafe extern "system" {
    fn CoCreateInstance(
        class: *const Guid,
        outer: Raw,
        context: u32,
        iid: *const Guid,
        result: *mut Raw,
    ) -> Hr;
    pub fn CoTaskMemFree(memory: Raw);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hstrings_round_trip() {
        let text = HString::new("SplitWire · Ö").unwrap();
        assert_eq!(text.text(), "SplitWire · Ö");
        assert_eq!(HString::new("").unwrap().text(), "");
        assert_eq!(check(-1), Err(-1));
        assert_eq!(check(1), Ok(()));
    }
}
