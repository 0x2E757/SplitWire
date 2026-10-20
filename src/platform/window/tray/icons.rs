//! The tray icons `build/` embedded in the executable, loaded at the small-icon size of the
//! system DPI and kept until the tray ends.

use std::collections::HashMap;
use std::ptr;

use super::abi::{
    DestroyIcon, GetDpiForSystem, GetSystemMetricsForDpi, IMAGE_ICON, LoadImageW, SM_CXSMICON,
};
use crate::platform::abi::Raw;
use crate::presentation::tray::{Taskbar, TrayArt};

pub struct TrayIcons {
    size: i32,
    loaded: HashMap<u16, Raw>,
}

impl TrayIcons {
    pub fn new() -> Self {
        // SAFETY: plain queries.
        let size = unsafe { GetSystemMetricsForDpi(SM_CXSMICON, GetDpiForSystem()) };
        Self {
            size: size.max(16),
            loaded: HashMap::new(),
        }
    }

    /// The icon of the art state `state` for `taskbar`.
    pub fn get(&mut self, state: &str, taskbar: Taskbar) -> Option<Raw> {
        let id = TrayArt::resource_id(state, taskbar);
        if let Some(icon) = self.loaded.get(&id) {
            return Some(*icon);
        }
        let icon = Self::load(id, self.size)?;
        self.loaded.insert(id, icon);
        Some(icon)
    }

    fn load(id: u16, size: i32) -> Option<Raw> {
        // SAFETY: an integer resource id of this module's icon group (MAKEINTRESOURCE).
        let icon = unsafe {
            LoadImageW(
                GetModuleHandleW(ptr::null()),
                usize::from(id) as *const u16,
                IMAGE_ICON,
                size,
                size,
                0,
            )
        };
        (!icon.is_null()).then_some(icon)
    }

    /// Loads every state for both taskbars at every embedded size, without showing any: the
    /// check of `--verify-pages`. The number loaded, or the first one missing.
    pub fn verify() -> Result<usize, String> {
        let mut count = 0;
        for state in TrayArt::STATES {
            for taskbar in Taskbar::BOTH {
                for size in TrayArt::SIZES {
                    let id = TrayArt::resource_id(state, taskbar);
                    let icon = Self::load(id, size as i32).ok_or_else(|| {
                        format!("tray icon {state} {taskbar:?} {size} px (group {id}) is missing")
                    })?;
                    // SAFETY: loaded above, not shared.
                    unsafe {
                        DestroyIcon(icon);
                    }
                    count += 1;
                }
            }
        }
        Ok(count)
    }
}

impl Drop for TrayIcons {
    fn drop(&mut self) {
        for icon in self.loaded.values() {
            // SAFETY: loaded by this cache, not shared.
            unsafe {
                DestroyIcon(*icon);
            }
        }
    }
}

#[link(name = "kernel32")]
unsafe extern "system" {
    fn GetModuleHandleW(name: *const u16) -> Raw;
}
