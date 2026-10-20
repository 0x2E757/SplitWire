//! What the window reads from and hands to Windows besides XAML: the app and taskbar themes, the
//! local time offset, the clipboard, Explorer, the user's folders and the console a check prints
//! to.

use std::path::PathBuf;
use std::ptr;

use super::winrt::{UI_SETTINGS_COLOR, UI_SETTINGS3};
use crate::platform::abi::{Raw, wide};
use crate::platform::com::{ComResult, activate, check};
use crate::platform::registry::{RegistryKey, RegistryRoot};
use crate::presentation::tray::Taskbar;

/// The Windows app theme: dark when the system's background colour is dark.
pub struct SystemTheme;

impl SystemTheme {
    pub fn dark() -> ComResult<bool> {
        let settings = activate("Windows.UI.ViewManagement.UISettings", &UI_SETTINGS3)?;
        // Windows.UI.Color is A, R, G, B bytes; UIColorType.Background is 0.
        let mut color = [0u8; 4];
        // SAFETY: GetColorValue(type, *Color).
        unsafe {
            let get: unsafe extern "system" fn(Raw, i32, *mut [u8; 4]) -> i32 =
                settings.slot(UI_SETTINGS_COLOR);
            check(get(settings.raw(), 0, &mut color))?;
        }
        Ok(u32::from(color[1]) + u32::from(color[2]) + u32::from(color[3]) < 384)
    }

    /// The taskbar's theme: the Windows mode (`SystemUsesLightTheme`), which the tray icon
    /// follows, not the app mode. Dark when the value is missing, as Windows 11's default.
    pub fn taskbar() -> Taskbar {
        let light = RegistryKey::open(RegistryRoot::CurrentUser, Self::PERSONALIZE, false)
            .ok()
            .flatten()
            .and_then(|key| key.dword("SystemUsesLightTheme").ok().flatten());
        match light {
            Some(1) => Taskbar::Light,
            _ => Taskbar::Dark,
        }
    }

    const PERSONALIZE: &'static str =
        r"Software\Microsoft\Windows\CurrentVersion\Themes\Personalize";
}

#[repr(C)]
struct TimeZoneInformation {
    bias: i32,
    standard_name: [u16; 32],
    standard_date: [u16; 8],
    standard_bias: i32,
    daylight_name: [u16; 32],
    daylight_date: [u16; 8],
    daylight_bias: i32,
}

/// Seconds to add to UTC for the local time now, daylight saving included.
pub struct LocalOffset;

impl LocalOffset {
    pub fn now() -> i64 {
        // SAFETY: fills the struct; the result says which bias is in effect.
        unsafe {
            let mut zone: TimeZoneInformation = std::mem::zeroed();
            let state = GetTimeZoneInformation(&mut zone);
            let extra = match state {
                1 => zone.standard_bias,
                2 => zone.daylight_bias,
                _ => 0,
            };
            -i64::from(zone.bias + extra) * 60
        }
    }
}

/// Puts text on the clipboard as `CF_UNICODETEXT`.
pub struct Clipboard;

impl Clipboard {
    pub fn copy(owner: Raw, text: &str) -> bool {
        let units = wide(text);
        let bytes = units.len() * 2;
        // SAFETY: the memory is handed to the clipboard on success and freed otherwise.
        unsafe {
            if OpenClipboard(owner) == 0 {
                return false;
            }
            EmptyClipboard();
            let memory = GlobalAlloc(0x0002, bytes);
            let mut done = false;
            if !memory.is_null() {
                let target = GlobalLock(memory) as *mut u16;
                if !target.is_null() {
                    ptr::copy_nonoverlapping(units.as_ptr(), target, units.len());
                    GlobalUnlock(memory);
                    done = !SetClipboardData(13, memory).is_null();
                }
                if !done {
                    GlobalFree(memory);
                }
            }
            CloseClipboard();
            done
        }
    }
}

/// Opens a folder in Explorer.
pub struct Shell;

impl Shell {
    pub fn open(owner: Raw, path: &std::path::Path) {
        let path = crate::platform::abi::wide_os(path.as_os_str());
        // SAFETY: the "open" verb on a folder path; SW_SHOWNORMAL.
        unsafe {
            ShellExecuteW(
                owner,
                wide("open").as_ptr(),
                path.as_ptr(),
                ptr::null(),
                ptr::null(),
                1,
            );
        }
    }
}

/// The user's and the machine's folders.
pub struct Folders;

impl Folders {
    fn env(name: &str, fallback: &str) -> PathBuf {
        std::env::var_os(name).map_or_else(|| PathBuf::from(fallback), PathBuf::from)
    }

    /// `%ProgramData%\SplitWire`.
    pub fn data() -> PathBuf {
        Self::env("ProgramData", "C:\\ProgramData").join("SplitWire")
    }

    /// `%LOCALAPPDATA%\SplitWire`.
    pub fn local() -> PathBuf {
        Self::env("LOCALAPPDATA", "C:\\").join("SplitWire")
    }
}

/// Output of a check started from a terminal: the window program has no console of its own.
pub struct ParentConsole;

impl ParentConsole {
    pub fn attach() {
        // SAFETY: STD_OUTPUT_HANDLE; ATTACH_PARENT_PROCESS.
        unsafe {
            if GetStdHandle(-11i32 as u32).is_null() {
                AttachConsole(u32::MAX);
            }
        }
    }
}

#[link(name = "kernel32")]
unsafe extern "system" {
    fn GetTimeZoneInformation(zone: *mut TimeZoneInformation) -> u32;
    fn GlobalAlloc(flags: u32, bytes: usize) -> Raw;
    fn GlobalLock(memory: Raw) -> Raw;
    fn GlobalUnlock(memory: Raw) -> i32;
    fn GlobalFree(memory: Raw) -> Raw;
    fn GetStdHandle(kind: u32) -> Raw;
    fn AttachConsole(process: u32) -> i32;
}

#[link(name = "user32")]
unsafe extern "system" {
    fn OpenClipboard(owner: Raw) -> i32;
    fn EmptyClipboard() -> i32;
    fn SetClipboardData(format: u32, memory: Raw) -> Raw;
    fn CloseClipboard() -> i32;
}

#[link(name = "shell32")]
unsafe extern "system" {
    fn ShellExecuteW(
        owner: Raw,
        verb: *const u16,
        file: *const u16,
        parameters: *const u16,
        directory: *const u16,
        show: i32,
    ) -> Raw;
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::mem::{offset_of, size_of};

    #[test]
    fn the_offset_is_whole_minutes_within_a_day() {
        let offset = LocalOffset::now();
        assert_eq!(offset % 60, 0);
        assert!(offset.abs() <= 14 * 3600);
        assert_eq!(size_of::<TimeZoneInformation>(), 172); // TIME_ZONE_INFORMATION
        assert_eq!(offset_of!(TimeZoneInformation, daylight_bias), 168); // TIME_ZONE_INFORMATION.DaylightBias
        assert!(Folders::data().ends_with("SplitWire"));
        // A read-only query of the user's theme.
        assert!(Taskbar::BOTH.contains(&SystemTheme::taskbar()));
    }
}
