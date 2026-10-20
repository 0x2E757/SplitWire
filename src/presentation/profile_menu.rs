//! The profile menu: right click on a profile item, or "…" on
//! its cards and header.

use super::{Command, Dot, StateText};
use crate::ipc::ProfileSummary;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MenuEntry {
    pub label: String,
    /// One of the window's own icons.
    pub icon: &'static str,
    pub enabled: bool,
    pub danger: bool,
    pub command: Command,
}

impl MenuEntry {
    fn new(label: &str, icon: &'static str, command: Command) -> Self {
        Self {
            label: label.into(),
            icon,
            enabled: true,
            danger: false,
            command,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProfileMenu {
    pub name: String,
    /// `AcmeCorp · Connected`: the status line at the top, after the state dot.
    pub header: String,
    pub dot: Dot,
    /// The items in groups, a separator between two groups: Connect or Disconnect; Replace .conf…
    /// and Open log; Remove….
    pub groups: Vec<Vec<MenuEntry>>,
}

impl ProfileMenu {
    pub fn of(profile: &ProfileSummary, global_invalid: bool) -> Self {
        let name = &profile.name;
        let state = &profile.state;
        let connect = if StateText::active(state) {
            MenuEntry::new(
                "Disconnect",
                "power",
                Command::SetConnected(name.clone(), false),
            )
        } else {
            MenuEntry {
                enabled: StateText::can_connect(state, global_invalid),
                ..MenuEntry::new(
                    "Connect",
                    "power",
                    Command::SetConnected(name.clone(), true),
                )
            }
        };
        let replace = MenuEntry::new(
            "Replace .conf…",
            "import",
            Command::PickFiles {
                replace: Some(name.clone()),
            },
        );
        let open_log = MenuEntry::new("Open log", "logs", Command::OpenLog(name.clone()));
        let remove = MenuEntry {
            danger: true,
            ..MenuEntry::new("Remove…", "trash", Command::AskRemove(name.clone()))
        };
        Self {
            name: name.clone(),
            header: format!("{name} · {}", StateText::short(state)),
            dot: StateText::dot(state),
            groups: vec![vec![connect], vec![replace, open_log], vec![remove]],
        }
    }

    /// Every item, in order.
    pub fn entries(&self) -> impl Iterator<Item = &MenuEntry> {
        self.groups.iter().flatten()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::presentation::samples::Samples;

    fn labels(menu: &ProfileMenu) -> Vec<&str> {
        menu.entries().map(|e| e.label.as_str()).collect()
    }

    #[test]
    fn a_connected_profile_offers_disconnect_and_nothing_timed() {
        let status = Samples::status("normal");
        let menu = ProfileMenu::of(&status.profiles[1], false);
        assert_eq!(menu.header, "AcmeCorp · On");
        assert_eq!(menu.dot, Dot::Ok);
        assert_eq!(
            labels(&menu),
            ["Disconnect", "Replace .conf…", "Open log", "Remove…"]
        );
        assert_eq!(menu.groups.len(), 3);
        assert_eq!(
            menu.groups[0][0].command,
            Command::SetConnected("AcmeCorp".into(), false)
        );
        assert_eq!(
            menu.entries().map(|e| e.icon).collect::<Vec<_>>(),
            ["power", "import", "logs", "trash"]
        );
        assert!(menu.groups[2][0].danger);
    }

    #[test]
    fn connect_is_disabled_while_blocked_or_held_off() {
        let status = Samples::status("normal");
        let menu = ProfileMenu::of(&status.profiles[2], false);
        assert_eq!(menu.groups[0][0].label, "Connect");
        assert!(!menu.groups[0][0].enabled);
        let held = Samples::status("settingsinvalid");
        assert!(!ProfileMenu::of(&held.profiles[0], true).groups[0][0].enabled);
    }
}
