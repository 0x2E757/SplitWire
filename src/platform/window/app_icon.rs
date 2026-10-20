//! The app icon `build/` embedded in both executables: the window class's big and small icon,
//! which the title bar, the taskbar and Alt+Tab show.

use std::ptr;

use crate::platform::abi::Raw;
use crate::presentation::tray::AppArt;

pub struct AppIcon;

impl AppIcon {
    const IMAGE_ICON: u32 = 1;
    /// `LR_SHARED`: the system frees the icon, so the class can keep it for good.
    const SHARED: u32 = 0x8000;
    const SM_CXICON: i32 = 11;
    const SM_CXSMICON: i32 = 49;

    /// The big and the small icon at the system DPI's sizes, for `WNDCLASSEXW`; null when missing.
    pub fn for_class(instance: Raw) -> (Raw, Raw) {
        let size = |metric: i32| {
            // SAFETY: plain queries.
            unsafe { GetSystemMetricsForDpi(metric, GetDpiForSystem()) }
        };
        (
            Self::load(instance, size(Self::SM_CXICON), Self::SHARED),
            Self::load(instance, size(Self::SM_CXSMICON), Self::SHARED),
        )
    }

    fn load(instance: Raw, size: i32, flags: u32) -> Raw {
        // SAFETY: an integer resource id of the module's icon group (MAKEINTRESOURCE).
        unsafe {
            LoadImageW(
                instance,
                usize::from(AppArt::RESOURCE_ID) as *const u16,
                Self::IMAGE_ICON,
                size,
                size,
                flags,
            )
        }
    }

    /// Loads every embedded size without showing any: the check of `--verify-pages`. The number
    /// loaded, or the first one missing.
    pub fn verify() -> Result<usize, String> {
        // SAFETY: the handle of this process's executable.
        let instance = unsafe { GetModuleHandleW(ptr::null()) };
        for size in AppArt::SIZES {
            let icon = Self::load(instance, size as i32, 0);
            if icon.is_null() {
                return Err(format!("the app icon at {size} px is missing"));
            }
            // SAFETY: loaded above, not shared.
            unsafe {
                DestroyIcon(icon);
            }
        }
        Ok(AppArt::SIZES.len())
    }
}

#[link(name = "kernel32")]
unsafe extern "system" {
    fn GetModuleHandleW(name: *const u16) -> Raw;
}

#[link(name = "user32")]
unsafe extern "system" {
    fn GetDpiForSystem() -> u32;
    fn GetSystemMetricsForDpi(index: i32, dpi: u32) -> i32;
    fn LoadImageW(
        instance: Raw,
        name: *const u16,
        kind: u32,
        width: i32,
        height: i32,
        flags: u32,
    ) -> Raw;
    fn DestroyIcon(icon: Raw) -> i32;
}
