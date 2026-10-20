use super::{Reconnected, SettingsFiles, StoredProfile};
use crate::config::{KeyValueFile, ProfileSettings, Settings, SettingsScope};
use crate::ipc::{Reconnect, ReconnectOutcome, ScopeSettings, SettingsSaved, SettingsView};

use super::CoordinatorError;
use super::wire_attempts::WireAttempts;

/// The settings files as the Settings page reads them: every scope's values with
/// their lines and its text, global first and then the profiles in order, with every problem.
pub struct SettingsViewBuilder<'a> {
    files: &'a SettingsFiles,
}

impl<'a> SettingsViewBuilder<'a> {
    pub fn new(files: &'a SettingsFiles) -> Self {
        Self { files }
    }

    pub fn view(&self, rev: u64, catalog: &[StoredProfile]) -> SettingsView {
        let mut scopes = Vec::new();
        let mut problems = Vec::new();
        let global = self.files.global_text().unwrap_or_default();
        scopes.push(Self::scope(SettingsScope::Global, Settings::FILE, global));
        if let Ok(Err(found)) = self.files.global() {
            problems.extend(found);
        }
        for stored in catalog {
            let name = stored.name.as_str();
            let text = self.files.profile_text(&stored.name).unwrap_or_default();
            let file = ProfileSettings::file_label(name);
            scopes.push(Self::scope(
                SettingsScope::Profile(name.into()),
                &file,
                text,
            ));
            if let Ok(Err(found)) = self.files.profile(&stored.name) {
                problems.extend(found);
            }
        }
        SettingsView {
            rev,
            scopes,
            problems,
        }
    }

    pub fn saved(settings: SettingsView, reconnects: Vec<Reconnected>) -> SettingsSaved {
        SettingsSaved {
            settings,
            reconnects: reconnects
                .into_iter()
                .map(|(name, outcome)| Reconnect {
                    name,
                    outcome: match outcome {
                        Ok(()) => ReconnectOutcome::Done,
                        Err(CoordinatorError::ConnectFailed(attempt)) => {
                            ReconnectOutcome::Failed(WireAttempts::attempt(&attempt))
                        }
                        Err(CoordinatorError::UnknownProfile(_)) => ReconnectOutcome::Done,
                    },
                })
                .collect(),
        }
    }

    fn scope(scope: SettingsScope, file: &str, text: String) -> ScopeSettings {
        let values = KeyValueFile::parse(file, &text).entries;
        ScopeSettings {
            scope,
            text,
            values,
        }
    }
}
