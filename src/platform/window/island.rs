//! System XAML on this thread (`WindowsXamlManager`), and a XAML island in a window
//! (`DesktopWindowXamlSource`).

use std::ptr;

use super::winrt::*;
use crate::platform::abi::Raw;
use crate::platform::com::{Com, ComResult, activate, check, factory};

/// Closes an `IClosable` on drop: the XAML manager or the island's source.
struct Closing(Com);

impl Drop for Closing {
    fn drop(&mut self) {
        if let Ok(closable) = self.0.query(&CLOSABLE) {
            // SAFETY: Close takes no arguments.
            unsafe {
                let close: unsafe extern "system" fn(Raw) -> i32 = closable.slot(CLOSABLE_CLOSE);
                let _ = close(closable.raw());
            }
        }
    }
}

/// XAML initialized for the calling thread; markup can be loaded without any window.
pub struct XamlRuntime {
    _manager: Closing,
}

impl XamlRuntime {
    pub fn start() -> ComResult<Self> {
        let statics = factory(
            "Windows.UI.Xaml.Hosting.WindowsXamlManager",
            &XAML_MANAGER_STATICS,
        )?;
        Ok(Self {
            _manager: Closing(statics.object(XAML_MANAGER_INITIALIZE)?),
        })
    }
}

/// An island attached to a window: its content is the window's client area.
pub struct Island {
    source: Closing,
    native: Com,
    /// The island's own child window.
    pub child: Raw,
}

impl Island {
    pub fn attach(window: Raw) -> ComResult<Self> {
        let source = Closing(activate(
            "Windows.UI.Xaml.Hosting.DesktopWindowXamlSource",
            &XAML_SOURCE,
        )?);
        let attached = source.0.query(&XAML_SOURCE_NATIVE)?;
        // SAFETY: AttachToWindow and get_WindowHandle take and return window handles.
        let child = unsafe {
            let attach: unsafe extern "system" fn(Raw, Raw) -> i32 =
                attached.slot(XAML_SOURCE_ATTACH);
            check(attach(attached.raw(), window))?;
            let get: unsafe extern "system" fn(Raw, *mut Raw) -> i32 =
                attached.slot(XAML_SOURCE_WINDOW);
            let mut child = ptr::null_mut();
            check(get(attached.raw(), &mut child))?;
            child
        };
        let native = source.0.query(&XAML_SOURCE_NATIVE2)?;
        Ok(Self {
            source,
            native,
            child,
        })
    }

    /// Shows `root` (a UIElement) in the island.
    pub fn set_content(&self, root: &Com) -> ComResult<()> {
        let element = root.query(&UI_ELEMENT)?;
        self.source
            .0
            .query(&XAML_SOURCE)?
            .set_value(XAML_SOURCE_PUT_CONTENT, element.raw())
    }

    /// Lets XAML see a message first (keyboard focus, Tab, accelerators); true when it took it.
    pub fn pre_translate(&self, message: *const std::ffi::c_void) -> bool {
        let mut handled = 0i32;
        // SAFETY: PreTranslateMessage takes a MSG pointer and a BOOL it sets when it took it.
        unsafe {
            let pre: unsafe extern "system" fn(Raw, *const std::ffi::c_void, *mut i32) -> i32 =
                self.native.slot(XAML_SOURCE_PRETRANSLATE);
            pre(self.native.raw(), message, &mut handled) >= 0 && handled != 0
        }
    }
}
