//! The notification icon itself: added once, modified when its icon or tooltip changes, added
//! again after Explorer restarts, removed on drop.

use super::abi::{
    NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NIM_MODIFY, NotifyIconData,
    Shell_NotifyIconW,
};
use super::message_window::MessageWindow;
use crate::platform::abi::Raw;

pub struct NotifyIcon {
    data: NotifyIconData,
    added: bool,
}

impl NotifyIcon {
    pub fn new(window: Raw) -> Self {
        let mut data = NotifyIconData::new(window, 1);
        data.flags = NIF_MESSAGE | NIF_ICON | NIF_TIP;
        data.callback = MessageWindow::CALLBACK;
        Self { data, added: false }
    }

    /// Shows `icon` with `tooltip` (cut to `szTip`); false while the taskbar is not there yet.
    pub fn show(&mut self, icon: Raw, tooltip: &str) -> bool {
        self.data.icon = icon;
        self.data.tip = [0; 128];
        for (slot, unit) in self
            .data
            .tip
            .iter_mut()
            .zip(tooltip.encode_utf16().take(127))
        {
            *slot = unit;
        }
        self.apply()
    }

    /// Explorer started again and forgot the icon.
    pub fn add_again(&mut self) -> bool {
        self.added = false;
        self.apply()
    }

    fn apply(&mut self) -> bool {
        // SAFETY: the data names this process's window and lives through the call.
        unsafe {
            if self.added {
                return Shell_NotifyIconW(NIM_MODIFY, &self.data) != 0;
            }
            // An icon with this id may linger after an Explorer restart: then it is modified.
            self.added = Shell_NotifyIconW(NIM_ADD, &self.data) != 0
                || Shell_NotifyIconW(NIM_MODIFY, &self.data) != 0;
            self.added
        }
    }
}

impl Drop for NotifyIcon {
    fn drop(&mut self) {
        if self.added {
            // SAFETY: removes the icon this process added.
            unsafe {
                Shell_NotifyIconW(NIM_DELETE, &self.data);
            }
        }
    }
}
