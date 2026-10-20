//! `SaveSettings`: merged and validated by the `config` rules, all or none.

use super::DemoEngine;
use crate::config::{SettingChange, SettingsMerge, SettingsScope};
use crate::ipc::{ErrorCode, ProfileState, RefusedProblem, Response, SettingsSaved};
use crate::presentation::transport::demo::reports::DemoReports;

impl DemoEngine {
    pub(super) fn save_settings(&mut self, base_rev: u64, changes: &[SettingChange]) -> Response {
        if base_rev != self.revs.settings {
            return Self::error(
                ErrorCode::StaleRev,
                "the settings changed on disk; reloaded",
            );
        }
        let model = &self.model;
        let plan = SettingsMerge::plan(
            changes,
            |scope| match scope {
                SettingsScope::Global => Some(model.global.clone()),
                SettingsScope::Profile(name) => model.profile(name).map(|p| p.settings.clone()),
            },
            false,
        );
        let files = match plan {
            Ok(files) => files,
            Err(problems) => {
                return Self::error(
                    ErrorCode::Refused(problems.into_iter().map(RefusedProblem::Setting).collect()),
                    "nothing was written",
                );
            }
        };
        for file in files {
            match &file.scope {
                SettingsScope::Global => self.model.global = file.text,
                SettingsScope::Profile(name) => {
                    if let Some(profile) = self.model.profile_mut(name) {
                        profile.settings = file.text;
                    }
                }
            }
        }
        self.settle_states();
        self.revs.settings += 1;
        self.revs.profiles += 1;
        self.revs.alerts += 1;
        Response::SettingsSaved(Box::new(SettingsSaved {
            settings: DemoReports::settings(&self.model, self.revs.settings),
            reconnects: Vec::new(),
        }))
    }

    /// After a settings save: fixed files let their profiles connect again (they stay off).
    pub(super) fn settle_states(&mut self) {
        let global_valid = self.model.global_problems().is_empty();
        if global_valid {
            self.model
                .alerts
                .retain(|a| a.kind != crate::ipc::AlertKind::SettingsInvalid);
        }
        for profile in &mut self.model.profiles {
            let valid = profile.settings_problems().is_empty();
            profile.state = match profile.state {
                ProfileState::HeldOff if global_valid => {
                    if valid {
                        ProfileState::Off
                    } else {
                        ProfileState::Blocked
                    }
                }
                ProfileState::Blocked if valid => ProfileState::Off,
                ProfileState::Off if !valid => ProfileState::Blocked,
                state => state,
            };
            if valid {
                let name = profile.name.clone();
                self.model.alerts.retain(|a| {
                    !(a.kind == crate::ipc::AlertKind::ProfileSettingsInvalid
                        && a.profile.as_deref() == Some(name.as_str()))
                });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::DemoEngine;
    use crate::config::{SettingChange, SettingsScope};
    use crate::ipc::{ProfileState, Request, Response};
    use crate::presentation::transport::demo::Scenario;

    #[test]
    pub(super) fn fixing_the_settings_lets_profiles_connect_again() {
        let mut engine = DemoEngine::new(Scenario::SettingsInvalid);
        let bad = engine.handle(Request::SaveSettings {
            base_rev: 1,
            changes: vec![SettingChange {
                scope: SettingsScope::Global,
                key: "max_ttl".into(),
                value: Some("5".into()),
            }],
        });
        assert!(matches!(bad, Response::Error(_)));
        let good = engine.handle(Request::SaveSettings {
            base_rev: 1,
            changes: vec![SettingChange {
                scope: SettingsScope::Global,
                key: "max_ttl".into(),
                value: Some("300".into()),
            }],
        });
        assert!(matches!(good, Response::SettingsSaved(_)));
        assert_eq!(engine.model().profiles[0].state, ProfileState::Off);
        assert_eq!(engine.model().profiles[2].state, ProfileState::Blocked);
        assert!(
            engine
                .model()
                .alerts
                .iter()
                .all(|a| !a.blocking || a.profile.is_some())
        );
    }
}
