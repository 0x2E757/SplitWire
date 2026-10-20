use super::settings_wire::SettingsWire;
use super::{Attempt, Reader, WireError, Writer};
use crate::config::{KeyValueEntry, SettingProblem, SettingsScope};

/// One settings file: its values with their lines, and its text for marking a bad line.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScopeSettings {
    pub scope: SettingsScope,
    pub text: String,
    pub values: Vec<KeyValueEntry>,
}

impl ScopeSettings {
    fn write(&self, w: &mut Writer) {
        SettingsWire::scope(&self.scope, w);
        w.str(&self.text);
        w.list(&self.values, SettingsWire::entry);
    }

    fn read(r: &mut Reader) -> Result<Self, WireError> {
        Ok(Self {
            scope: SettingsWire::read_scope(r)?,
            text: r.string()?,
            values: r.list(SettingsWire::read_entry)?,
        })
    }
}

/// The answer to `GetSettings`: every scope, global first, then the profiles in order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SettingsView {
    /// The `base_rev` the next `SaveSettings` sends.
    pub rev: u64,
    pub scopes: Vec<ScopeSettings>,
    pub problems: Vec<SettingProblem>,
}

impl SettingsView {
    pub fn write(&self, w: &mut Writer) {
        w.u64(self.rev);
        w.list(&self.scopes, ScopeSettings::write);
        w.list(&self.problems, SettingsWire::problem);
    }

    pub fn read(r: &mut Reader) -> Result<Self, WireError> {
        Ok(Self {
            rev: r.u64()?,
            scopes: r.list(ScopeSettings::read)?,
            problems: r.list(SettingsWire::read_problem)?,
        })
    }
}

/// How the reconnect of a connected profile whose `upstream` or `mtu` changed went.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ReconnectOutcome {
    Done,
    Failed(Attempt),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Reconnect {
    pub name: String,
    pub outcome: ReconnectOutcome,
}

impl Reconnect {
    fn write(&self, w: &mut Writer) {
        w.str(&self.name);
        match &self.outcome {
            ReconnectOutcome::Done => w.u64(0),
            ReconnectOutcome::Failed(attempt) => {
                w.u64(1);
                attempt.write(w);
            }
        }
    }

    fn read(r: &mut Reader) -> Result<Self, WireError> {
        let name = r.string()?;
        let outcome = match r.u64()? {
            0 => ReconnectOutcome::Done,
            1 => ReconnectOutcome::Failed(Attempt::read(r)?),
            _ => return Err(WireError::Invalid("an unknown reconnect outcome")),
        };
        Ok(Self { name, outcome })
    }
}

/// The answer to a `SaveSettings` that was written: the new values and the reconnects.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SettingsSaved {
    pub settings: SettingsView,
    pub reconnects: Vec<Reconnect>,
}

impl SettingsSaved {
    pub fn write(&self, w: &mut Writer) {
        self.settings.write(w);
        w.list(&self.reconnects, Reconnect::write);
    }

    pub fn read(r: &mut Reader) -> Result<Self, WireError> {
        Ok(Self {
            settings: SettingsView::read(r)?,
            reconnects: r.list(Reconnect::read)?,
        })
    }
}
