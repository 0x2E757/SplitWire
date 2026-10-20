//! The text WinRT keeps for the last error of this thread, such as XamlReader's line and position
//! of bad markup.

use std::ptr;

use super::winrt::{RESTRICTED_ERROR_DETAILS, RESTRICTED_ERROR_INFO};
use crate::platform::abi::Raw;
use crate::platform::com::Com;

pub struct ErrorInfo;

impl ErrorInfo {
    /// The description of the thread's last WinRT error, taken (and so cleared).
    pub fn take() -> Option<String> {
        // SAFETY: GetRestrictedErrorInfo hands over a reference or null; the BSTRs it returns are
        // ours to free.
        unsafe {
            let mut raw = ptr::null_mut();
            if GetRestrictedErrorInfo(&mut raw) < 0 || raw.is_null() {
                return None;
            }
            let info = Com::owned(raw).ok()?.query(&RESTRICTED_ERROR_INFO).ok()?;
            let details: unsafe extern "system" fn(
                Raw,
                *mut *mut u16,
                *mut i32,
                *mut *mut u16,
                *mut *mut u16,
            ) -> i32 = info.slot(RESTRICTED_ERROR_DETAILS);
            let (mut description, mut error, mut restricted, mut sid) =
                (ptr::null_mut(), 0, ptr::null_mut(), ptr::null_mut());
            if details(
                info.raw(),
                &mut description,
                &mut error,
                &mut restricted,
                &mut sid,
            ) < 0
            {
                return None;
            }
            let text = [restricted, description]
                .iter()
                .map(|s| Self::bstr(*s))
                .find(|s| !s.is_empty());
            for s in [description, restricted, sid] {
                SysFreeString(s);
            }
            text
        }
    }

    unsafe fn bstr(text: *mut u16) -> String {
        if text.is_null() {
            return String::new();
        }
        // SAFETY: a BSTR carries its length in bytes before the text.
        unsafe {
            let length = SysStringLen(text) as usize;
            String::from_utf16_lossy(std::slice::from_raw_parts(text, length))
        }
    }
}

#[link(name = "runtimeobject")]
unsafe extern "system" {
    fn GetRestrictedErrorInfo(info: *mut Raw) -> i32;
}

#[link(name = "oleaut32")]
unsafe extern "system" {
    fn SysFreeString(text: *mut u16);
    fn SysStringLen(text: *mut u16) -> u32;
}
