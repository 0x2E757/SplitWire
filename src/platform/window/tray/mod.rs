//! The tray: a notification icon whose image follows the service's `TrayState`
//! and the taskbar's theme, a tooltip, a left click that opens the window and a right-click menu.
//! The window's process owns it; its view comes from the window's session.

mod abi;
mod icons;
mod menu_theme;
mod message_window;
mod notify_icon;
mod popup_menu;

pub use icons::TrayIcons;
pub use message_window::TrayEvent;
pub use popup_menu::PopupMenu;

use std::io;

use abi::{GetDpiForSystem, GetSystemMetricsForDpi};
use menu_theme::MenuTheme;
use message_window::MessageWindow;
use notify_icon::NotifyIcon;

use crate::ipc::TrayState;
use crate::presentation::tray::{Taskbar, TrayAction, TrayItem, TrayView};

pub struct Tray {
    // The icon goes before its window.
    icon: NotifyIcon,
    window: MessageWindow,
    icons: TrayIcons,
    theme: MenuTheme,
    /// What the icon shows now; `None` until the taskbar took it.
    shown: Option<(TrayState, Taskbar, String)>,
}

impl Tray {
    pub fn new() -> io::Result<Self> {
        let window = MessageWindow::new()?;
        Ok(Self {
            icon: NotifyIcon::new(window.raw()),
            window,
            icons: TrayIcons::new(),
            theme: MenuTheme::load(),
            shown: None,
        })
    }

    /// Shows `view`'s state and tooltip for a taskbar of `taskbar`'s theme, when they changed.
    pub fn show(&mut self, view: &TrayView, taskbar: Taskbar) {
        let wanted = (view.state, taskbar, view.tooltip.clone());
        if self.shown.as_ref() == Some(&wanted) {
            return;
        }
        let Some(icon) = self.icons.get(TrayView::art_key(view.state), taskbar) else {
            return;
        };
        if self.icon.show(icon, &view.tooltip) {
            self.shown = Some(wanted);
        }
    }

    /// The clicks since the last call. A restarted Explorer gets the icon again here.
    pub fn events(&mut self) -> Vec<TrayEvent> {
        let events = MessageWindow::take_events();
        if events.contains(&TrayEvent::TaskbarCreated) {
            self.shown = None;
            self.icon.add_again();
        }
        events
            .into_iter()
            .filter(|e| *e != TrayEvent::TaskbarCreated)
            .collect()
    }

    /// Shows `menu` at the pointer, dark or light as the app; the chosen item's action.
    pub fn menu(&self, menu: &[TrayItem], dark: bool) -> Option<TrayAction> {
        self.theme.apply(dark);
        let popup = PopupMenu::build(menu, dark, Self::mark_size()).ok()?;
        popup.track(self.window.raw())
    }

    /// The marks' side: 16 px at 100 %.
    pub fn mark_size() -> u32 {
        // SAFETY: plain queries; SM_CXSMICON.
        let size = unsafe { GetSystemMetricsForDpi(49, GetDpiForSystem()) };
        size.max(16) as u32
    }
}
