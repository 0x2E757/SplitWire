//! Dark or light popup menus, following the app theme. Windows has no documented switch: uxtheme's
//! `SetPreferredAppMode` (ordinal 135) and `FlushMenuThemes` (ordinal 136) are what Explorer and
//! other Win32 apps use since Windows 10 1903. Without them the menus stay light.

use crate::platform::abi::{
    GetProcAddress, LOAD_LIBRARY_SEARCH_SYSTEM32, LoadLibraryExW, Raw, wide,
};

type SetPreferredAppMode = unsafe extern "system" fn(i32) -> i32;
type FlushMenuThemes = unsafe extern "system" fn();

pub struct MenuTheme {
    set_mode: Option<SetPreferredAppMode>,
    flush: Option<FlushMenuThemes>,
}

impl MenuTheme {
    const FORCE_DARK: i32 = 2;
    const FORCE_LIGHT: i32 = 3;

    pub fn load() -> Self {
        let name = wide("uxtheme.dll");
        // SAFETY: a system DLL from System32 only, never freed; ordinals looked up by number
        // (MAKEINTRESOURCEA), each checked before it is cast to its signature.
        unsafe {
            let module = LoadLibraryExW(
                name.as_ptr(),
                std::ptr::null_mut(),
                LOAD_LIBRARY_SEARCH_SYSTEM32,
            );
            if module.is_null() {
                return Self {
                    set_mode: None,
                    flush: None,
                };
            }
            let ordinal = |n: usize| -> Raw { GetProcAddress(module, n as *const u8) };
            let (set_mode, flush) = (ordinal(135), ordinal(136));
            Self {
                set_mode: (!set_mode.is_null())
                    .then(|| std::mem::transmute::<Raw, SetPreferredAppMode>(set_mode)),
                flush: (!flush.is_null())
                    .then(|| std::mem::transmute::<Raw, FlushMenuThemes>(flush)),
            }
        }
    }

    /// The next menus are dark, or light.
    pub fn apply(&self, dark: bool) {
        if let (Some(set_mode), Some(flush)) = (self.set_mode, self.flush) {
            // SAFETY: the process-wide preference for menus; then the cached themes are dropped.
            unsafe {
                set_mode(if dark {
                    Self::FORCE_DARK
                } else {
                    Self::FORCE_LIGHT
                });
                flush();
            }
        }
    }
}
