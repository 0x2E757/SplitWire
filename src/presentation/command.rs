//! What the user asks for (`Command`), the text fields the window reads (`InputId`) and what the
//! session asks the platform to do (`Effect`). The platform maps events to commands; the session
//! turns them into requests and state.

use std::path::PathBuf;
use std::sync::Arc;

use super::{Page, ThemeChoice, WindowSettings};
use crate::ipc::{ActionTarget, ConfText, SettingField};
use crate::profile::Pin;

/// A text field of a page. The window polls each shown field and reports a change as
/// `Command::Input`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum InputId {
    /// The profile page's Add field.
    ProfileAdd,
    ProfileSearch,
    RoutesSearch,
    ListsSearch,
    /// The Lists page's Add field of one profile.
    ListsAdd(String),
    /// The combined text of every list.
    ListsText,
    /// The text of the Edit as text dialog.
    ListDialog,
    LogFilter,
    Setting(SettingField),
    BypassNew,
    /// The Name field of the review's file at this index.
    ReviewName(usize),
}

impl InputId {
    /// The element name of the field.
    pub fn element(&self) -> String {
        match self {
            Self::ProfileAdd => "ProfileAdd".into(),
            Self::ProfileSearch => "ProfileSearch".into(),
            Self::RoutesSearch => "RoutesSearch".into(),
            Self::ListsSearch => "ListsSearch".into(),
            Self::ListsAdd(name) => format!("ListsAdd_{}", Self::safe(name)),
            Self::ListsText => "ListsText".into(),
            Self::ListDialog => "ListDialogText".into(),
            Self::LogFilter => "LogFilter".into(),
            Self::Setting(field) => format!(
                "Setting_{}_{}",
                match &field.scope {
                    crate::config::SettingsScope::Global => "global".to_owned(),
                    crate::config::SettingsScope::Profile(name) => Self::safe(name),
                },
                field.key
            ),
            Self::BypassNew => "BypassNew".into(),
            Self::ReviewName(index) => format!("ReviewName{index}"),
        }
    }

    /// A profile name in an element name: `.` and `-` are not allowed there.
    pub fn safe(name: &str) -> String {
        name.chars()
            .map(|c| match c {
                '.' => "_d".to_owned(),
                '-' => "_h".to_owned(),
                '_' => "__".to_owned(),
                c => c.to_string(),
            })
            .collect()
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ListsMode {
    #[default]
    Rows,
    Text,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum LogSource {
    /// The service's ring buffer.
    #[default]
    Live,
    /// `logs\splitwire.log`: warnings and errors, kept across restarts.
    File,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum LevelFilter {
    #[default]
    All,
    Info,
    Warnings,
    Errors,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum ProfileFilter {
    #[default]
    All,
    /// Lines of no profile: the service, the DNS proxy, NRPT.
    Global,
    Profile(String),
}

/// A `.conf` file the platform read for an import. Its text holds a private key: it stays in a
/// zeroed, redacted `ConfText`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PickedFile {
    pub file_name: String,
    pub text: Result<Arc<ConfText>, String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Command {
    Go(Page),
    /// The full navigation over the content in the narrow layout.
    NavOverlay(bool),
    /// A Connect switch or menu item: connect, or disconnect.
    SetConnected(String, bool),
    OpenLog(String),
    AskRemove(String),
    Remove(String),
    CloseDialog,
    /// Drag a profile to a new position in the order.
    MoveProfile(String, usize),
    Dismiss(String),
    DismissAll,
    RestoreAlerts,
    /// An alert's action link; a suggestion fills its field.
    FollowAlert(String),
    /// Opens or closes an expander or a group by its id.
    Toggle(String),
    Input(InputId, String),
    /// Enter in a field.
    Submit(InputId),
    Copy(String),
    AddEntry(String),
    RemoveLine {
        profile: String,
        line: usize,
        text: String,
    },
    EditListAsText(String),
    SaveListDialog,
    ListsMode(ListsMode),
    SaveAllLists,
    RevertAllLists,
    LogSource(LogSource),
    LogLevel(LevelFilter),
    LogProfile(ProfileFilter),
    LogFollow,
    LogLatest,
    OpenLogFolder,
    SettingToggle(SettingField),
    SettingUse(SettingField, String),
    BypassRemove(String),
    BypassAdd,
    SettingsRevert,
    SettingsApply,
    StartAtSignIn(bool),
    OpenWindowAtSignIn(bool),
    Theme(ThemeChoice),
    /// Choose files; `replace` names the profile a Replace .conf… is for.
    PickFiles {
        replace: Option<String>,
    },
    /// The platform read the picked or dropped files.
    FilesRead(Vec<PickedFile>),
    /// A dragged set of files is over the drop zone; its file names when known.
    DragOver(Vec<String>),
    DragLeave,
    ReviewColour(usize, Pin),
    Import,
    CancelImport,
    /// The platform read `logs\splitwire.log` and `.1` (older first).
    LogFileRead(String),
    /// The window was minimized or hidden, or shown again.
    Visible(bool),
    /// Empties a profile's DNS cache (the tray's Reset DNS cache).
    ResetCache(String),
    /// Asks for the status now, out of schedule (the tray's menu opens).
    Refresh,
}

impl Command {
    /// The action target of an alert as a command.
    pub fn target(target: &ActionTarget) -> Self {
        match target {
            ActionTarget::Status => Self::Go(Page::Status),
            ActionTarget::Profile { name, .. } => Self::Go(Page::Profile(name.clone())),
            ActionTarget::Settings(_) => Self::Go(Page::Settings),
            ActionTarget::Logs { profile } => match profile {
                Some(name) => Self::OpenLog(name.clone()),
                None => Self::Go(Page::Logs),
            },
        }
    }
}

/// What the session asks the platform to do.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Effect {
    Copy(String),
    OpenFolder(PathBuf),
    /// Show the multi-select `*.conf` picker; the platform answers with `FilesRead`.
    PickFiles,
    /// Read the log file; the platform answers with `LogFileRead`.
    ReadLogFile,
    SaveWindowSettings(WindowSettings),
    /// Writes or deletes the HKCU Run value that starts the tray at sign-in.
    StartAtSignIn(bool),
    /// Scroll the page back to its top.
    ScrollTop,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::SettingsScope;

    #[test]
    fn element_names_are_xaml_names() {
        assert_eq!(
            InputId::ListsAdd("Acme.Corp-1_x".into()).element(),
            "ListsAdd_Acme_dCorp_h1__x"
        );
        let field = SettingField {
            scope: SettingsScope::Profile("HomeLab".into()),
            key: "mtu".into(),
        };
        assert_eq!(InputId::Setting(field).element(), "Setting_HomeLab_mtu");
        assert_eq!(InputId::ReviewName(2).element(), "ReviewName2");
    }

    #[test]
    fn alert_targets_navigate() {
        assert_eq!(
            Command::target(&ActionTarget::Logs {
                profile: Some("AcmeCorp".into())
            }),
            Command::OpenLog("AcmeCorp".into())
        );
        assert_eq!(
            Command::target(&ActionTarget::Settings(None)),
            Command::Go(Page::Settings)
        );
    }
}
