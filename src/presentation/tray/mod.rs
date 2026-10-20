//! The tray: the icon's state, its tooltip and the right-click menu, built
//! from the same `Status` the window shows, and the app icon drawn with the tray's arrow. Pure:
//! the platform draws the menu, shows the icon and hands the chosen item's action back.

pub mod app_art;
mod menu;
mod menu_art;
pub mod raster;
mod split_arrow;
mod tooltip;
pub mod tray_art;

pub use app_art::AppArt;
pub use menu::TrayMenu;
pub use menu_art::MenuArt;
pub use tooltip::TrayTooltip;
pub use tray_art::{Taskbar, TrayArt};

use super::{Command, Dot};
use crate::ipc::{Severity, StatusReport, TrayState};

/// The mark in front of a menu item.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TrayLead {
    None,
    /// The tray icon of a state: the menu's header.
    State(TrayState),
    /// A profile's state dot.
    Dot(Dot),
    Severity(Severity),
}

/// What choosing an item does.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TrayAction {
    /// A command for the session, without opening the window.
    Session(Command),
    /// Opens the window, or brings it to the front, then runs the command when there is one.
    Open(Option<Command>),
    /// Ends the window and the tray; the service keeps running.
    Exit,
}

/// One line of the menu.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TrayEntry {
    pub label: String,
    /// Quiet text on the right: a state, a profile.
    pub key: Option<String>,
    pub lead: TrayLead,
    pub enabled: bool,
    /// Drawn bold: Open SplitWire, what a left click does.
    pub default: bool,
    /// `None` for a header line.
    pub action: Option<TrayAction>,
}

impl TrayEntry {
    pub fn action(label: impl Into<String>, action: TrayAction) -> Self {
        Self {
            label: label.into(),
            key: None,
            lead: TrayLead::None,
            enabled: true,
            default: false,
            action: Some(action),
        }
    }

    /// A line that says something and does nothing.
    pub fn header(label: impl Into<String>, lead: TrayLead) -> Self {
        Self {
            label: label.into(),
            key: None,
            lead,
            enabled: false,
            default: false,
            action: None,
        }
    }

    pub fn key(mut self, key: impl Into<String>) -> Self {
        let key = key.into();
        self.key = (!key.is_empty()).then_some(key);
        self
    }

    pub fn lead(mut self, lead: TrayLead) -> Self {
        self.lead = lead;
        self
    }

    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TrayItem {
    Separator,
    Entry(TrayEntry),
    /// An entry opening its own menu.
    Submenu(TrayEntry, Vec<TrayItem>),
}

/// Everything the tray shows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TrayView {
    pub state: TrayState,
    /// At most `TrayTooltip::MAX` UTF-16 units: `NOTIFYICONDATAW.szTip`.
    pub tooltip: String,
    pub menu: Vec<TrayItem>,
}

impl TrayView {
    /// The tray of a service that answered with `status`, or of none (`None`: the pipe is absent).
    pub fn of(status: Option<&StatusReport>) -> Self {
        let state = status.map_or(TrayState::Service, |s| s.tray_state);
        Self {
            state,
            tooltip: TrayTooltip::of(status),
            menu: TrayMenu::of(status),
        }
    }

    /// The `TrayArt` state of `state`: without the service the icon is the error's.
    pub fn art_key(state: TrayState) -> &'static str {
        match state {
            TrayState::Connected => "connected",
            TrayState::Warning => "warning",
            TrayState::Error | TrayState::Service => "error",
            TrayState::Off => "off",
        }
    }
}

#[cfg(test)]
mod approved;
#[cfg(test)]
mod tests;
