//! The structures and calls of the notification icon, its menu and the menu's bitmaps, written from
//! `shellapi.h`, `winuser.h` and `wingdi.h`.

use crate::platform::abi::{Guid, Raw};

/// `NOTIFYICONDATAW`.
#[repr(C)]
pub struct NotifyIconData {
    pub size: u32,
    pub window: Raw,
    pub id: u32,
    pub flags: u32,
    pub callback: u32,
    pub icon: Raw,
    pub tip: [u16; 128],
    pub state: u32,
    pub state_mask: u32,
    pub info: [u16; 256],
    pub version: u32,
    pub info_title: [u16; 64],
    pub info_flags: u32,
    pub guid: Guid,
    pub balloon_icon: Raw,
}

impl NotifyIconData {
    pub fn new(window: Raw, id: u32) -> Self {
        Self {
            size: std::mem::size_of::<Self>() as u32,
            window,
            id,
            flags: 0,
            callback: 0,
            icon: std::ptr::null_mut(),
            tip: [0; 128],
            state: 0,
            state_mask: 0,
            info: [0; 256],
            version: 0,
            info_title: [0; 64],
            info_flags: 0,
            guid: Guid::from_u128(0),
            balloon_icon: std::ptr::null_mut(),
        }
    }
}

/// `MENUITEMINFOW`.
#[repr(C)]
pub struct MenuItemInfo {
    pub size: u32,
    pub mask: u32,
    pub kind: u32,
    pub state: u32,
    pub id: u32,
    pub submenu: Raw,
    pub checked: Raw,
    pub unchecked: Raw,
    pub data: usize,
    pub text: *mut u16,
    pub length: u32,
    pub bitmap: Raw,
}

impl MenuItemInfo {
    /// An item of no kind yet: the mask names what is set.
    pub fn new() -> Self {
        Self {
            size: std::mem::size_of::<Self>() as u32,
            mask: MIIM_FTYPE,
            kind: 0,
            state: 0,
            id: 0,
            submenu: std::ptr::null_mut(),
            checked: std::ptr::null_mut(),
            unchecked: std::ptr::null_mut(),
            data: 0,
            text: std::ptr::null_mut(),
            length: 0,
            bitmap: std::ptr::null_mut(),
        }
    }
}

/// `BITMAPINFOHEADER`.
#[repr(C)]
pub struct BitmapInfoHeader {
    pub size: u32,
    pub width: i32,
    pub height: i32,
    pub planes: u16,
    pub bit_count: u16,
    pub compression: u32,
    pub image_size: u32,
    pub x_per_meter: i32,
    pub y_per_meter: i32,
    pub used: u32,
    pub important: u32,
}

#[repr(C)]
#[derive(Default)]
pub struct Point {
    pub x: i32,
    pub y: i32,
}

pub const NIM_ADD: u32 = 0;
pub const NIM_MODIFY: u32 = 1;
pub const NIM_DELETE: u32 = 2;
pub const NIF_MESSAGE: u32 = 0x1;
pub const NIF_ICON: u32 = 0x2;
pub const NIF_TIP: u32 = 0x4;

pub const MIIM_STATE: u32 = 0x1;
pub const MIIM_ID: u32 = 0x2;
pub const MIIM_SUBMENU: u32 = 0x4;
pub const MIIM_STRING: u32 = 0x40;
pub const MIIM_BITMAP: u32 = 0x80;
pub const MIIM_FTYPE: u32 = 0x100;
pub const MFT_SEPARATOR: u32 = 0x800;
pub const MFS_DISABLED: u32 = 0x3;
pub const MFS_DEFAULT: u32 = 0x1000;

pub const TPM_RIGHTBUTTON: u32 = 0x2;
pub const TPM_BOTTOMALIGN: u32 = 0x20;
pub const TPM_RETURNCMD: u32 = 0x100;

pub const IMAGE_ICON: u32 = 1;
pub const SM_CXSMICON: i32 = 49;

#[link(name = "shell32")]
unsafe extern "system" {
    pub fn Shell_NotifyIconW(message: u32, data: *const NotifyIconData) -> i32;
}

#[link(name = "user32")]
unsafe extern "system" {
    pub fn CreatePopupMenu() -> Raw;
    pub fn InsertMenuItemW(
        menu: Raw,
        item: u32,
        by_position: i32,
        info: *const MenuItemInfo,
    ) -> i32;
    pub fn GetMenuItemCount(menu: Raw) -> i32;
    pub fn DestroyMenu(menu: Raw) -> i32;
    pub fn TrackPopupMenuEx(menu: Raw, flags: u32, x: i32, y: i32, window: Raw, params: Raw)
    -> i32;
    pub fn GetCursorPos(point: *mut Point) -> i32;
    pub fn SetForegroundWindow(window: Raw) -> i32;
    pub fn PostMessageW(window: Raw, message: u32, wparam: usize, lparam: isize) -> i32;
    pub fn RegisterWindowMessageW(name: *const u16) -> u32;
    pub fn GetDpiForSystem() -> u32;
    pub fn GetSystemMetricsForDpi(index: i32, dpi: u32) -> i32;
    pub fn LoadImageW(
        instance: Raw,
        name: *const u16,
        kind: u32,
        width: i32,
        height: i32,
        flags: u32,
    ) -> Raw;
    pub fn DestroyIcon(icon: Raw) -> i32;
}

#[link(name = "gdi32")]
unsafe extern "system" {
    pub fn CreateDIBSection(
        dc: Raw,
        info: *const BitmapInfoHeader,
        usage: u32,
        bits: *mut Raw,
        section: Raw,
        offset: u32,
    ) -> Raw;
    pub fn DeleteObject(object: Raw) -> i32;
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::mem::{offset_of, size_of};

    #[test]
    fn layouts_match_the_sdk() {
        assert_eq!(size_of::<NotifyIconData>(), 976); // NOTIFYICONDATAW
        assert_eq!(offset_of!(NotifyIconData, icon), 32); // NOTIFYICONDATAW.hIcon
        assert_eq!(offset_of!(NotifyIconData, tip), 40); // NOTIFYICONDATAW.szTip
        assert_eq!(offset_of!(NotifyIconData, state), 296); // NOTIFYICONDATAW.dwState
        assert_eq!(offset_of!(NotifyIconData, guid), 952); // NOTIFYICONDATAW.guidItem
        assert_eq!(offset_of!(NotifyIconData, balloon_icon), 968); // NOTIFYICONDATAW.hBalloonIcon
        assert_eq!(size_of::<MenuItemInfo>(), 80); // MENUITEMINFOW
        assert_eq!(offset_of!(MenuItemInfo, submenu), 24); // MENUITEMINFOW.hSubMenu
        assert_eq!(offset_of!(MenuItemInfo, text), 56); // MENUITEMINFOW.dwTypeData
        assert_eq!(offset_of!(MenuItemInfo, bitmap), 72); // MENUITEMINFOW.hbmpItem
        assert_eq!(size_of::<BitmapInfoHeader>(), 40); // BITMAPINFOHEADER
        assert_eq!(offset_of!(BitmapInfoHeader, compression), 16); // BITMAPINFOHEADER.biCompression
    }
}
