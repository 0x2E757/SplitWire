use std::time::SystemTime;

use super::settings_wire::SettingsWire;
use super::{Reader, WireError, Writer};
use crate::config::SettingsScope;

/// The kinds of alert.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AlertKind {
    SettingsInvalid,
    ProfileSettingsInvalid,
    ConnectFailed,
    RecoverFailed,
    UpstreamUnreachable,
    NrptWriteFailed,
    HandshakeStale,
    AddressHeld,
    ListProblem,
    GpNrpt,
    RouteRefused,
}

impl AlertKind {
    pub const ALL: [Self; 11] = [
        Self::SettingsInvalid,
        Self::ProfileSettingsInvalid,
        Self::ConnectFailed,
        Self::RecoverFailed,
        Self::UpstreamUnreachable,
        Self::NrptWriteFailed,
        Self::HandshakeStale,
        Self::AddressHeld,
        Self::ListProblem,
        Self::GpNrpt,
        Self::RouteRefused,
    ];
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    Error,
    Warning,
    Info,
}

impl Severity {
    pub const ALL: [Self; 3] = [Self::Error, Self::Warning, Self::Info];
}

/// One setting: a field the window can navigate to and fill.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SettingField {
    pub scope: SettingsScope,
    pub key: String,
}

impl SettingField {
    fn write(&self, w: &mut Writer) {
        SettingsWire::scope(&self.scope, w);
        w.str(&self.key);
    }

    fn read(r: &mut Reader) -> Result<Self, WireError> {
        Ok(Self {
            scope: SettingsWire::read_scope(r)?,
            key: r.string()?,
        })
    }
}

/// Where an alert's action leads: `status`, `profile:<name>[#field]`, `settings[#scope.key]` or
/// `logs[:profile]`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ActionTarget {
    Status,
    Profile { name: String, field: Option<String> },
    Settings(Option<SettingField>),
    Logs { profile: Option<String> },
}

impl ActionTarget {
    fn write(&self, w: &mut Writer) {
        match self {
            Self::Status => w.u64(0),
            Self::Profile { name, field } => {
                w.u64(1);
                w.str(name);
                w.opt_str(field.as_ref());
            }
            Self::Settings(field) => {
                w.u64(2);
                w.option(field.as_ref(), SettingField::write);
            }
            Self::Logs { profile } => {
                w.u64(3);
                w.opt_str(profile.as_ref());
            }
        }
    }

    fn read(r: &mut Reader) -> Result<Self, WireError> {
        Ok(match r.u64()? {
            0 => Self::Status,
            1 => Self::Profile {
                name: r.string()?,
                field: r.opt_string()?,
            },
            2 => Self::Settings(r.option(SettingField::read)?),
            3 => Self::Logs {
                profile: r.opt_string()?,
            },
            _ => return Err(WireError::Invalid("an unknown action target")),
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AlertAction {
    pub label: String,
    pub target: ActionTarget,
}

impl AlertAction {
    fn write(&self, w: &mut Writer) {
        w.str(&self.label);
        self.target.write(w);
    }

    fn read(r: &mut Reader) -> Result<Self, WireError> {
        Ok(Self {
            label: r.string()?,
            target: ActionTarget::read(r)?,
        })
    }
}

/// A one-click fix ("Use 1420"): the window fills the field; nothing is written until Apply.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Suggestion {
    pub field: SettingField,
    pub value: String,
}

impl Suggestion {
    fn write(&self, w: &mut Writer) {
        self.field.write(w);
        w.str(&self.value);
    }

    fn read(r: &mut Reader) -> Result<Self, WireError> {
        Ok(Self {
            field: SettingField::read(r)?,
            value: r.string()?,
        })
    }
}

/// An alert as the window, the tray and the CLI see it. The fingerprint stays in the
/// service.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Alert {
    /// `kind:profile:subject`, stable for the condition.
    pub id: String,
    pub kind: AlertKind,
    pub severity: Severity,
    /// `None` for a global alert.
    pub profile: Option<String>,
    pub since: SystemTime,
    pub cleared: Option<SystemTime>,
    pub title: String,
    pub message: String,
    pub action: Option<AlertAction>,
    pub suggest: Option<Suggestion>,
    /// Keeps a profile, or every profile, off; it cannot be dismissed.
    pub blocking: bool,
    pub dismissed: bool,
}

impl Alert {
    pub fn write(&self, w: &mut Writer) {
        w.str(&self.id);
        w.choice(&self.kind, &AlertKind::ALL);
        w.choice(&self.severity, &Severity::ALL);
        w.opt_str(self.profile.as_ref());
        w.time(self.since);
        w.option(self.cleared.as_ref(), |at, w| w.time(*at));
        w.str(&self.title);
        w.str(&self.message);
        w.option(self.action.as_ref(), AlertAction::write);
        w.option(self.suggest.as_ref(), Suggestion::write);
        w.bool(self.blocking);
        w.bool(self.dismissed);
    }

    pub fn read(r: &mut Reader) -> Result<Self, WireError> {
        Ok(Self {
            id: r.string()?,
            kind: r.choice(&AlertKind::ALL)?,
            severity: r.choice(&Severity::ALL)?,
            profile: r.opt_string()?,
            since: r.time()?,
            cleared: r.option(Reader::time)?,
            title: r.string()?,
            message: r.string()?,
            action: r.option(AlertAction::read)?,
            suggest: r.option(Suggestion::read)?,
            blocking: r.bool()?,
            dismissed: r.bool()?,
        })
    }
}
