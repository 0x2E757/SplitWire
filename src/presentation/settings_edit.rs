//! Pending settings changes: edits collect here, are checked as typed with the
//! service's own rules, and are written by Apply through `SaveSettings`, all or none. Fixes such as
//! "Use 1420" only fill a field.

use crate::config::{
    ProfileSettings, SettingChange, SettingProblem, SettingValue, Settings, SettingsScope,
};
use crate::ipc::{ProfileState, SettingField, SettingsView, StatusReport};

/// The bar pinned under the page.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApplyBarView {
    pub text: String,
    pub bad: bool,
    pub can_apply: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SettingsEdit {
    pending: Vec<(SettingField, String)>,
}

impl SettingsEdit {
    pub fn field(scope: SettingsScope, key: &str) -> SettingField {
        SettingField {
            scope,
            key: key.into(),
        }
    }

    /// The value in the file, or the key's default.
    pub fn saved(view: Option<&SettingsView>, field: &SettingField) -> String {
        let value = view
            .and_then(|v| v.scopes.iter().find(|s| s.scope == field.scope))
            .and_then(|s| s.values.iter().find(|e| e.key == field.key))
            .map(|e| e.value.clone());
        match (value, field.key.as_str()) {
            (Some(value), _) => value,
            (None, "connect_at_boot") => "false".into(),
            (None, "max_ttl") => Settings::DEFAULT_MAX_TTL.to_string(),
            (None, "idle_expiry") => Settings::DEFAULT_IDLE_EXPIRY.to_string(),
            (None, _) => String::new(),
        }
    }

    pub fn value(&self, view: Option<&SettingsView>, field: &SettingField) -> String {
        self.pending
            .iter()
            .find(|(f, _)| f == field)
            .map_or_else(|| Self::saved(view, field), |(_, v)| v.clone())
    }

    pub fn set(&mut self, field: SettingField, value: String) {
        self.pending.retain(|(f, _)| *f != field);
        self.pending.push((field, value));
    }

    pub fn toggle(&mut self, view: Option<&SettingsView>, field: SettingField) {
        let on = self.value(view, &field) == "true";
        self.set(field, (!on).to_string());
    }

    pub fn revert(&mut self) {
        self.pending.clear();
    }

    /// Forgets the changes of `scope`, such as a profile that was removed.
    pub fn retain(&mut self, keep: impl Fn(&SettingField) -> bool) {
        self.pending.retain(|(f, _)| keep(f));
    }

    pub fn changed(&self, view: Option<&SettingsView>, field: &SettingField) -> bool {
        self.pending
            .iter()
            .any(|(f, v)| f == field && *v != Self::saved(view, field))
    }

    /// The fields whose pending value differs from the file.
    pub fn changed_fields(&self, view: Option<&SettingsView>) -> Vec<&SettingField> {
        self.pending
            .iter()
            .filter(|(f, _)| self.changed(view, f))
            .map(|(f, _)| f)
            .collect()
    }

    /// The service's rule for one value, as typed.
    pub fn check(field: &SettingField, value: &str) -> Result<(), String> {
        let value = value.trim();
        match field.key.as_str() {
            "fallback" => SettingValue::addresses(value, Settings::MAX_FALLBACK).map(drop),
            "bypass" => SettingValue::names(value).map(drop),
            "max_ttl" => SettingValue::number(value, 30, 86_400).map(drop),
            "idle_expiry" => SettingValue::number(value, 600, 604_800).map(drop),
            "connect_at_boot" => SettingValue::boolean(value).map(drop),
            "upstream" if value.is_empty() => Ok(()),
            "upstream" => SettingValue::servers(value, 1, ProfileSettings::MAX_UPSTREAM).map(drop),
            "mtu" if value.is_empty() => Ok(()),
            "mtu" => SettingValue::number(value, 1280, 1500).map(drop),
            _ => Ok(()),
        }
    }

    /// The saved file's problem of `field`.
    pub fn file_problem<'a>(
        view: Option<&'a SettingsView>,
        field: &SettingField,
    ) -> Option<&'a SettingProblem> {
        let file = match &field.scope {
            SettingsScope::Global => Settings::FILE.to_owned(),
            SettingsScope::Profile(name) => ProfileSettings::file_label(name),
        };
        view?
            .problems
            .iter()
            .find(|p| p.file == file && p.key == field.key)
    }

    /// The problem shown under a field: a pending value's, or the file's own while unchanged.
    pub fn problem(&self, view: Option<&SettingsView>, field: &SettingField) -> Option<String> {
        if let Some((_, value)) = self.pending.iter().find(|(f, _)| f == field) {
            return Self::check(field, value).err();
        }
        Self::file_problem(view, field).map(|p| {
            format!(
                "{} line {}: {} = {} {}",
                Self::short_file(&p.file),
                p.line,
                p.key,
                p.value,
                p.reason
            )
        })
    }

    fn short_file(file: &str) -> &str {
        file.rsplit('\\').next().unwrap_or(file)
    }

    /// The changes Apply sends; an empty override deletes its line.
    pub fn changes(&self, view: Option<&SettingsView>) -> Vec<SettingChange> {
        self.changed_fields(view)
            .into_iter()
            .map(|field| {
                let value = self.value(view, field).trim().to_owned();
                SettingChange {
                    scope: field.scope.clone(),
                    key: field.key.clone(),
                    value: (!value.is_empty()).then_some(value),
                }
            })
            .collect()
    }

    /// The connected profiles a change reconnects (`upstream`, `mtu`).
    pub fn reconnects(
        &self,
        view: Option<&SettingsView>,
        status: Option<&StatusReport>,
    ) -> Vec<String> {
        let mut names: Vec<String> = self
            .changed_fields(view)
            .into_iter()
            .filter(|f| ProfileSettings::RECONNECT_KEYS.contains(&f.key.as_str()))
            .filter_map(|f| match &f.scope {
                SettingsScope::Profile(name) => Some(name.clone()),
                SettingsScope::Global => None,
            })
            .filter(|name| {
                status.is_some_and(|s| {
                    s.profiles
                        .iter()
                        .any(|p| p.name == *name && matches!(p.state, ProfileState::Connected(_)))
                })
            })
            .collect();
        names.dedup();
        names
    }

    /// The apply bar; `None` when nothing is pending and nothing is wrong.
    pub fn bar(
        &self,
        view: Option<&SettingsView>,
        status: Option<&StatusReport>,
        scope: Option<&SettingsScope>,
    ) -> Option<ApplyBarView> {
        let changed: Vec<&SettingField> = self
            .changed_fields(view)
            .into_iter()
            .filter(|f| scope.is_none_or(|s| f.scope == *s))
            .collect();
        if changed.is_empty() {
            let problem = view?.problems.iter().find(|p| match scope {
                None => p.file == Settings::FILE,
                Some(SettingsScope::Profile(name)) => p.file == ProfileSettings::file_label(name),
                Some(SettingsScope::Global) => p.file == Settings::FILE,
            })?;
            let fix = problem
                .suggest
                .as_ref()
                .map(|s| format!(" · use {s} or type a valid value"))
                .unwrap_or_default();
            let blocks = if is_global(problem) {
                "blocks every profile".to_owned()
            } else {
                format!(
                    "must be fixed before {} can connect",
                    scope_name(scope).unwrap_or_default()
                )
            };
            return Some(ApplyBarView {
                text: format!(
                    "{} line {}: {} = {} {blocks}{fix}",
                    Self::short_file(&problem.file),
                    problem.line,
                    problem.key,
                    problem.value
                ),
                bad: true,
                can_apply: false,
            });
        }
        let invalid = changed
            .iter()
            .filter(|f| self.problem(view, f).is_some())
            .count();
        let count = if changed.len() == 1 {
            "1 change".to_owned()
        } else {
            format!("{} changes", changed.len())
        };
        if invalid > 0 {
            let values = if invalid == 1 {
                "one value is".to_owned()
            } else {
                format!("{invalid} values are")
            };
            return Some(ApplyBarView {
                text: format!("{count} · {values} not valid yet"),
                bad: true,
                can_apply: false,
            });
        }
        let reconnects = self.reconnects(view, status);
        let tail = if reconnects.is_empty() {
            " · profile changes take effect at their next Connect".to_owned()
        } else {
            format!(" · applying reconnects {}", reconnects.join(", "))
        };
        Some(ApplyBarView {
            text: format!("{count} not applied{tail}"),
            bad: false,
            can_apply: true,
        })
    }
}

fn is_global(problem: &SettingProblem) -> bool {
    problem.file == Settings::FILE
}

fn scope_name(scope: Option<&SettingsScope>) -> Option<&str> {
    match scope {
        Some(SettingsScope::Profile(name)) => Some(name),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::presentation::samples::Samples;

    fn global(key: &str) -> SettingField {
        SettingsEdit::field(SettingsScope::Global, key)
    }

    #[test]
    fn values_come_from_the_file_until_edited() {
        let view = Samples::settings("normal");
        let mut edit = SettingsEdit::default();
        assert_eq!(edit.value(Some(&view), &global("max_ttl")), "300");
        assert_eq!(edit.value(Some(&view), &global("fallback")), "");
        edit.set(global("max_ttl"), "600".into());
        assert!(edit.changed(Some(&view), &global("max_ttl")));
        edit.set(global("max_ttl"), "300".into());
        assert!(!edit.changed(Some(&view), &global("max_ttl")));
        assert!(edit.bar(Some(&view), None, None).is_none());
    }

    #[test]
    fn the_bar_counts_changes_and_blocks_invalid_ones() {
        let view = Samples::settings("normal");
        let status = Samples::status("normal");
        let mut edit = SettingsEdit::default();
        edit.set(global("max_ttl"), "5".into());
        let bar = edit.bar(Some(&view), Some(&status), None).unwrap();
        assert_eq!(bar.text, "1 change · one value is not valid yet");
        assert!(bar.bad && !bar.can_apply);
        edit.set(global("max_ttl"), "600".into());
        let acme = SettingsEdit::field(SettingsScope::Profile("AcmeCorp".into()), "mtu");
        edit.set(acme.clone(), "1400".into());
        let bar = edit.bar(Some(&view), Some(&status), None).unwrap();
        assert_eq!(
            bar.text,
            "2 changes not applied · applying reconnects AcmeCorp"
        );
        assert!(bar.can_apply);
        let changes = edit.changes(Some(&view));
        assert_eq!(changes.len(), 2);
        let home = SettingsEdit::field(SettingsScope::Profile("HomeLab".into()), "mtu");
        edit.set(home, String::new());
        assert_eq!(edit.changes(Some(&view))[2].value, None);
    }

    #[test]
    fn a_bad_file_shows_its_line_when_nothing_is_pending() {
        let view = Samples::settings("settingsinvalid");
        let edit = SettingsEdit::default();
        let bar = edit.bar(Some(&view), None, None).unwrap();
        assert!(
            bar.text
                .starts_with("splitwire.conf line 4: max_ttl = 10 blocks every profile")
        );
        assert!(bar.text.ends_with("use 300 or type a valid value"));
        assert!(
            edit.problem(Some(&view), &global("max_ttl"))
                .unwrap()
                .starts_with("splitwire.conf line 4")
        );
        let home = SettingsScope::Profile("HomeLab".into());
        let bar = edit.bar(Some(&view), None, Some(&home)).unwrap();
        assert!(
            bar.text
                .contains("must be fixed before HomeLab can connect")
        );
    }
}
