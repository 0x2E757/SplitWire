use super::{Reader, WireError, Writer};
use crate::config::{KeyValueEntry, SettingChange, SettingProblem, SettingsScope};

/// The wire form of the settings types the messages reuse.
pub struct SettingsWire;

impl SettingsWire {
    pub fn scope(scope: &SettingsScope, w: &mut Writer) {
        match scope {
            SettingsScope::Global => w.u64(0),
            SettingsScope::Profile(name) => {
                w.u64(1);
                w.str(name);
            }
        }
    }

    pub fn read_scope(r: &mut Reader) -> Result<SettingsScope, WireError> {
        match r.u64()? {
            0 => Ok(SettingsScope::Global),
            1 => Ok(SettingsScope::Profile(r.string()?)),
            _ => Err(WireError::Invalid("an unknown settings scope")),
        }
    }

    pub fn problem(problem: &SettingProblem, w: &mut Writer) {
        w.str(&problem.file);
        w.usize(problem.line);
        w.str(&problem.key);
        w.str(&problem.value);
        w.str(&problem.reason);
        w.opt_str(problem.suggest.as_ref());
    }

    pub fn read_problem(r: &mut Reader) -> Result<SettingProblem, WireError> {
        Ok(SettingProblem {
            file: r.string()?,
            line: r.usize()?,
            key: r.string()?,
            value: r.string()?,
            reason: r.string()?,
            suggest: r.opt_string()?,
        })
    }

    /// `unset` is a missing value.
    pub fn change(change: &SettingChange, w: &mut Writer) {
        Self::scope(&change.scope, w);
        w.str(&change.key);
        w.opt_str(change.value.as_ref());
    }

    pub fn read_change(r: &mut Reader) -> Result<SettingChange, WireError> {
        Ok(SettingChange {
            scope: Self::read_scope(r)?,
            key: r.string()?,
            value: r.opt_string()?,
        })
    }

    pub fn entry(entry: &KeyValueEntry, w: &mut Writer) {
        w.usize(entry.line);
        w.str(&entry.key);
        w.str(&entry.value);
    }

    pub fn read_entry(r: &mut Reader) -> Result<KeyValueEntry, WireError> {
        Ok(KeyValueEntry {
            line: r.usize()?,
            key: r.string()?,
            value: r.string()?,
        })
    }
}
