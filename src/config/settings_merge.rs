use super::{ProfileSettings, SettingProblem, Settings};

/// Which file a setting lives in.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum SettingsScope {
    /// `splitwire.conf`.
    Global,
    /// `profiles\<name>\profile.conf`.
    Profile(String),
}

impl SettingsScope {
    /// `global` or `profile:<name>`, as `SaveSettings` names them.
    pub fn parse(text: &str) -> Option<Self> {
        match text.split_once(':') {
            None if text == "global" => Some(Self::Global),
            Some(("profile", name)) if !name.is_empty() => Some(Self::Profile(name.to_owned())),
            _ => None,
        }
    }
}

/// One change of `SaveSettings`: a value, or `None` to delete the key's line.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SettingChange {
    pub scope: SettingsScope,
    pub key: String,
    pub value: Option<String>,
}

/// A file's new text, valid, ready to be written with the others.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MergedFile {
    pub scope: SettingsScope,
    pub text: String,
}

/// Merges setting changes into the files' texts, keeping comments, order and line endings, and
/// validates every affected file: all of them are valid, or none is returned.
pub struct SettingsMerge;

impl SettingsMerge {
    /// `current` gives a scope's text (empty for a missing file), or `None` for an unknown profile.
    pub fn plan(
        changes: &[SettingChange],
        mut current: impl FnMut(&SettingsScope) -> Option<String>,
        dry_run: bool,
    ) -> Result<Vec<MergedFile>, Vec<SettingProblem>> {
        let mut scopes: Vec<&SettingsScope> = changes.iter().map(|c| &c.scope).collect();
        scopes.sort();
        scopes.dedup();
        let mut merged = Vec::new();
        let mut problems = Vec::new();
        for scope in scopes {
            let Some(text) = current(scope) else {
                problems.push(Self::unknown(scope));
                continue;
            };
            let edits: Vec<(&str, Option<&str>)> = changes
                .iter()
                .filter(|c| &c.scope == scope)
                .map(|c| (c.key.as_str(), c.value.as_deref()))
                .collect();
            let text = Self::merge(&text, &edits);
            let check = match scope {
                SettingsScope::Global => Settings::parse(&text, dry_run).map(drop),
                SettingsScope::Profile(name) => ProfileSettings::parse(name, &text).map(drop),
            };
            match check {
                Ok(()) => merged.push(MergedFile {
                    scope: scope.clone(),
                    text,
                }),
                Err(found) => problems.extend(found),
            }
        }
        if problems.is_empty() {
            Ok(merged)
        } else {
            Err(problems)
        }
    }

    /// Sets each key on its first line (later repeats are dropped), deletes it for `None`, and
    /// appends a key that is not there.
    pub fn merge(text: &str, edits: &[(&str, Option<&str>)]) -> String {
        let newline = if text.contains("\r\n") { "\r\n" } else { "\n" };
        let mut lines: Vec<String> = text.lines().map(str::to_owned).collect();
        for (key, value) in edits {
            let mut found = false;
            lines.retain_mut(|line| {
                if Self::key_of(line) != Some(*key) {
                    return true;
                }
                match value {
                    Some(value) if !found => {
                        found = true;
                        *line = Self::with_value(line, value);
                        true
                    }
                    _ => false,
                }
            });
            if let (Some(value), false) = (value, found) {
                lines.push(format!("{key}={value}"));
            }
        }
        let mut out = lines.join(newline);
        if !out.is_empty() {
            out.push_str(newline);
        }
        out
    }

    fn key_of(line: &str) -> Option<&str> {
        let code = line.split('#').next().unwrap_or_default();
        code.split_once('=').map(|(key, _)| key.trim())
    }

    /// The line with its value replaced, keeping the spacing and the trailing comment.
    fn with_value(line: &str, value: &str) -> String {
        let (code, comment) = line.split_at(line.find('#').unwrap_or(line.len()));
        let equals = code.find('=').expect("a key line has '='") + 1;
        let (head, rest) = code.split_at(equals);
        let (lead, old) = rest.split_at(rest.len() - rest.trim_start().len());
        let trail = &old[old.trim_end().len()..];
        let trail = if !comment.is_empty() && trail.is_empty() {
            " "
        } else {
            trail
        };
        format!("{head}{lead}{value}{trail}{comment}")
    }

    fn unknown(scope: &SettingsScope) -> SettingProblem {
        let name = match scope {
            SettingsScope::Global => Settings::FILE.to_owned(),
            SettingsScope::Profile(name) => name.clone(),
        };
        SettingProblem {
            file: name.clone(),
            line: 0,
            key: String::new(),
            value: String::new(),
            reason: format!("unknown profile {name}"),
            suggest: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn change(scope: SettingsScope, key: &str, value: Option<&str>) -> SettingChange {
        SettingChange {
            scope,
            key: key.to_owned(),
            value: value.map(str::to_owned),
        }
    }

    #[test]
    fn keeps_comments_order_and_spacing() {
        let text = "# DNS\nmax_ttl = 300   # five minutes\nbypass=a.example.com\n\n# end\n";
        let merged = SettingsMerge::merge(
            text,
            &[
                ("max_ttl", Some("60")),
                ("bypass", None),
                ("fallback", Some("1.1.1.1")),
            ],
        );
        assert_eq!(
            merged,
            "# DNS\nmax_ttl = 60   # five minutes\n\n# end\nfallback=1.1.1.1\n"
        );
        assert_eq!(
            SettingsMerge::merge("mtu=1380#x\r\nupstream=\r\n", &[("mtu", Some("1420"))]),
            "mtu=1420 #x\r\nupstream=\r\n"
        );
        assert_eq!(
            SettingsMerge::merge("", &[("mtu", Some("1400"))]),
            "mtu=1400\n"
        );
        assert_eq!(
            SettingsMerge::merge("mtu=1\nmtu=2\n", &[("mtu", Some("3"))]),
            "mtu=3\n"
        );
    }

    #[test]
    fn plans_every_file_or_none() {
        let current = |scope: &SettingsScope| match scope {
            SettingsScope::Global => Some("max_ttl=300\n".to_owned()),
            SettingsScope::Profile(name) if name == "AcmeCorp" => {
                Some("connect_at_boot=false\n".to_owned())
            }
            SettingsScope::Profile(_) => None,
        };
        let acme = SettingsScope::Profile("AcmeCorp".into());
        let good = [
            change(acme.clone(), "mtu", Some("1380")),
            change(SettingsScope::Global, "max_ttl", Some("60")),
        ];
        let planned = SettingsMerge::plan(&good, current, false).unwrap();
        assert_eq!(
            planned,
            [
                MergedFile {
                    scope: SettingsScope::Global,
                    text: "max_ttl=60\n".into()
                },
                MergedFile {
                    scope: acme.clone(),
                    text: "connect_at_boot=false\nmtu=1380\n".into()
                },
            ]
        );
        let bad = [
            change(acme, "mtu", Some("9000")),
            change(SettingsScope::Global, "max_ttl", Some("60")),
            change(SettingsScope::Profile("Gone".into()), "mtu", None),
        ];
        let problems = SettingsMerge::plan(&bad, current, false).unwrap_err();
        let reasons: Vec<String> = problems.iter().map(ToString::to_string).collect();
        assert_eq!(
            reasons,
            [
                "profiles\\AcmeCorp\\profile.conf line 2: must be a number 1280-1500",
                "Gone line 0: unknown profile Gone",
            ]
        );
    }

    #[test]
    fn parses_scopes() {
        assert_eq!(SettingsScope::parse("global"), Some(SettingsScope::Global));
        assert_eq!(
            SettingsScope::parse("profile:AcmeCorp"),
            Some(SettingsScope::Profile("AcmeCorp".into()))
        );
        assert_eq!(SettingsScope::parse("profile:"), None);
        assert_eq!(SettingsScope::parse("user"), None);
    }
}
