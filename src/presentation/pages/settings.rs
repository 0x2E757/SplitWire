//! Settings: expanders with summaries and Changed / Problem badges. Profiles (one per
//! profile), All profiles (`splitwire.conf`: DNS, Routes and cache), This app (window and tray, at
//! once) and About. Service settings wait for Apply.

use super::InfoBar;
use crate::config::{SettingValue, SettingsScope};
use crate::ipc::{ProfileState, SettingField, SettingsView, StatusReport};
use crate::presentation::{
    ApplyBarView, Format, LocalTime, SettingsEdit, StateText, ThemeChoice, WindowSettings,
};
use crate::profile::{ImportPreview, Pin};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Badge {
    Changed,
    Problem,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExpanderView {
    pub id: String,
    pub title: String,
    pub description: String,
    pub summary: String,
    pub badge: Option<Badge>,
    pub open: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FieldView {
    pub field: SettingField,
    pub value: String,
    pub placeholder: String,
    pub problem: Option<String>,
    pub changed: bool,
    /// "Use 300": fills the field with the suggested value.
    pub fix: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProfileExpander {
    pub name: String,
    pub colour: Pin,
    pub expander: ExpanderView,
    pub boot: (bool, bool),
    pub upstream: FieldView,
    pub mtu: FieldView,
    /// `vpn.acme.example:51820 · imported 16.10.2026 · stored encrypted for this machine`.
    pub about: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SettingsPageView {
    pub message: Option<InfoBar>,
    pub profiles_caption: String,
    pub profiles: Vec<ProfileExpander>,
    pub dns: ExpanderView,
    pub listen: String,
    pub fallback: FieldView,
    pub bypass: Vec<String>,
    pub bypass_changed: bool,
    pub bypass_new: String,
    pub bypass_problem: Option<String>,
    pub cache: ExpanderView,
    pub max_ttl: FieldView,
    pub idle_expiry: FieldView,
    pub window: ExpanderView,
    pub start_at_sign_in: bool,
    pub open_window_at_sign_in: bool,
    pub theme: ThemeChoice,
    pub about: ExpanderView,
    pub wgnt: String,
    pub service_since: String,
    pub data_folder: String,
    pub apply_bar: Option<ApplyBarView>,
}

pub struct SettingsSource<'a> {
    pub status: &'a StatusReport,
    pub settings: Option<&'a SettingsView>,
    pub edit: &'a SettingsEdit,
    pub window: &'a WindowSettings,
    pub start_at_sign_in: bool,
    pub bypass_new: &'a str,
    /// The data folder's path, shown as text.
    pub data_folder: &'a str,
    /// Whether an expander is open, given its default.
    pub open: &'a dyn Fn(&str, bool) -> bool,
    pub time: &'a LocalTime,
}

impl SettingsSource<'_> {
    fn field(&self, scope: SettingsScope, key: &str, placeholder: String) -> FieldView {
        let field = SettingsEdit::field(scope, key);
        let problem = self.edit.problem(self.settings, &field);
        let fix = SettingsEdit::file_problem(self.settings, &field)
            .and_then(|p| p.suggest.clone())
            .filter(|_| problem.is_some());
        FieldView {
            value: self.edit.value(self.settings, &field),
            changed: self.edit.changed(self.settings, &field),
            placeholder,
            problem,
            fix,
            field,
        }
    }

    fn badge(fields: &[&FieldView], extra_changed: bool) -> Option<Badge> {
        if fields.iter().any(|f| f.problem.is_some()) {
            Some(Badge::Problem)
        } else if extra_changed || fields.iter().any(|f| f.changed) {
            Some(Badge::Changed)
        } else {
            None
        }
    }

    fn expander(
        &self,
        id: &str,
        title: &str,
        description: &str,
        summary: String,
        badge: Option<Badge>,
        default: bool,
    ) -> ExpanderView {
        ExpanderView {
            id: id.into(),
            title: title.into(),
            description: description.into(),
            summary: if badge == Some(Badge::Problem) {
                String::new()
            } else {
                summary
            },
            badge,
            open: (self.open)(id, default),
        }
    }

    pub fn view(&self) -> SettingsPageView {
        let global_invalid = self.status.problems.first();
        let global = SettingsScope::Global;
        let profiles = self
            .status
            .profiles
            .iter()
            .map(|p| {
                let scope = SettingsScope::Profile(p.name.clone());
                let boot_field = SettingsEdit::field(scope.clone(), "connect_at_boot");
                let boot = (
                    self.edit.value(self.settings, &boot_field) == "true",
                    self.edit.changed(self.settings, &boot_field),
                );
                let upstream = self.field(
                    scope.clone(),
                    "upstream",
                    "Automatic · the profile’s DNS".into(),
                );
                let mtu = self.field(scope, "mtu", "Automatic · the profile’s MTU".into());
                let badge = Self::badge(&[&upstream, &mtu], boot.1);
                let state = match &p.state {
                    _ if global_invalid.is_some() && p.state != ProfileState::Blocked => {
                        "Off · splitwire.conf is invalid".to_owned()
                    }
                    ProfileState::Connected(_) => format!(
                        "Connected · {}",
                        p.uptime_secs.map(Format::span).unwrap_or_default()
                    ),
                    ProfileState::Blocked => "Can’t connect · profile.conf".to_owned(),
                    ProfileState::Failed => StateText::pill(p, self.time).tail.map_or_else(
                        || "Connect failed".into(),
                        |t| format!("Connect failed · {t}"),
                    ),
                    state => StateText::pill(p, self.time).tail.map_or_else(
                        || StateText::label(state).into(),
                        |t| format!("{} · {t}", StateText::label(state)),
                    ),
                };
                let default_open = global_invalid.is_none()
                    && matches!(p.state, ProfileState::Blocked | ProfileState::Failed);
                ProfileExpander {
                    name: p.name.clone(),
                    colour: p.colour,
                    expander: self.expander(
                        &format!("settings:profile:{}", p.name),
                        &p.name,
                        &state,
                        format!(
                            "boot {} · DNS {} · MTU {}",
                            if boot.0 { "on" } else { "off" },
                            if upstream.value.is_empty() {
                                "auto"
                            } else {
                                &upstream.value
                            },
                            if mtu.value.is_empty() {
                                "auto"
                            } else {
                                &mtu.value
                            }
                        ),
                        badge,
                        default_open,
                    ),
                    boot,
                    upstream,
                    mtu,
                    about: "stored encrypted for this machine".into(),
                }
            })
            .collect();
        let listen = self
            .settings
            .and_then(|v| v.scopes.iter().find(|s| s.scope == global))
            .and_then(|s| s.values.iter().find(|e| e.key == "listen"))
            .map_or_else(|| "127.0.0.53:53".to_owned(), |e| e.value.clone());
        let fallback_auto = self
            .status
            .fallback
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(", ");
        let fallback = self.field(
            global.clone(),
            "fallback",
            format!("Automatic · {fallback_auto}"),
        );
        let bypass_field = SettingsEdit::field(global.clone(), "bypass");
        let bypass: Vec<String> = self
            .edit
            .value(self.settings, &bypass_field)
            .split(',')
            .map(str::trim)
            .filter(|n| !n.is_empty())
            .map(str::to_owned)
            .collect();
        let bypass_changed = self.edit.changed(self.settings, &bypass_field);
        let max_ttl = self.field(global.clone(), "max_ttl", "300".into());
        let idle_expiry = self.field(global.clone(), "idle_expiry", "86400".into());
        let dns_badge = Self::badge(&[&fallback], bypass_changed);
        let cache_badge = Self::badge(&[&max_ttl, &idle_expiry], false);
        let idle_text = if idle_expiry.value == "86400" {
            "24 h".to_owned()
        } else {
            format!("{} s", idle_expiry.value)
        };
        SettingsPageView {
            message: global_invalid.map(|g| {
                InfoBar::new(
                    "splitwire.conf is invalid, so every profile stays disconnected",
                    &format!(
                        "Line {}: {} = {} {}. Fix it under Routes and cache and Apply; the service writes the file only when every value is valid.",
                        g.line, g.key, g.value, g.reason
                    ),
                )
            }),
            profiles_caption: format!(
                "{} of {} · overrides saved to each profile’s profile.conf",
                self.status.profiles.len(),
                ImportPreview::LIMIT
            ),
            profiles,
            dns: self.expander(
                "settings:dns",
                "DNS",
                "The proxy, the fallback DNS and the bypass names",
                format!(
                    "{listen} · fallback {} · {}",
                    if fallback.value.is_empty() { "auto" } else { &fallback.value },
                    Format::plural(bypass.len(), "bypass name", "bypass names")
                ),
                dns_badge,
                false,
            ),
            listen,
            fallback,
            bypass,
            bypass_changed,
            bypass_new: self.bypass_new.to_owned(),
            bypass_problem: (!self.bypass_new.trim().is_empty())
                .then(|| SettingValue::names(self.bypass_new.trim()).err())
                .flatten(),
            cache: self.expander(
                "settings:cache",
                "Routes and cache",
                "How long answers are cached and learned routes kept",
                format!("max TTL {} s · idle expiry {idle_text}", max_ttl.value),
                cache_badge,
                global_invalid.is_some(),
            ),
            max_ttl,
            idle_expiry,
            window: self.expander(
                "settings:window",
                "Window and tray",
                "Applies at once; stored for your account, not in splitwire.conf",
                format!(
                    "tray at sign-in {} · window {} · {}",
                    if self.start_at_sign_in { "on" } else { "off" },
                    if self.window.open_window_at_sign_in { "opens" } else { "on demand" },
                    self.window.theme.label()
                ),
                None,
                false,
            ),
            start_at_sign_in: self.start_at_sign_in,
            open_window_at_sign_in: self.window.open_window_at_sign_in,
            theme: self.window.theme,
            about: self.expander(
                "settings:about",
                &format!("SplitWire v{}", self.status.version),
                "Routes the names on each profile’s list through that profile’s WireGuard tunnel; everything else stays on your network",
                format!(
                    "wireguard-nt {} · service since {}",
                    self.wgnt(),
                    self.time.clock(self.status.since)
                ),
                None,
                false,
            ),
            wgnt: format!("wireguard-nt {}", self.wgnt()),
            service_since: format!("running since {}", self.time.clock(self.status.since)),
            data_folder: self.data_folder.to_owned(),
            apply_bar: self.edit.bar(self.settings, Some(self.status), None),
        }
    }

    fn wgnt(&self) -> String {
        self.status
            .wgnt_version
            .map_or_else(|| "not loaded".into(), |v| v.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::presentation::samples::Samples;

    fn view(key: &str, edit: &SettingsEdit) -> SettingsPageView {
        let status = Samples::status(key);
        let settings = Samples::settings(key);
        let open = |_: &str, default: bool| default;
        SettingsSource {
            status: &status,
            settings: Some(&settings),
            edit,
            window: &WindowSettings::default(),
            start_at_sign_in: true,
            bypass_new: "",
            data_folder: "C:\\ProgramData\\SplitWire",
            open: &open,
            time: &Samples::time(),
        }
        .view()
    }

    #[test]
    fn normal_settings_summarise_every_group() {
        let view = view("normal", &SettingsEdit::default());
        assert_eq!(
            view.profiles_caption,
            "3 of 16 · overrides saved to each profile’s profile.conf"
        );
        let acme = &view.profiles[1].expander;
        assert_eq!(acme.description, "Connected · 47 min");
        assert_eq!(acme.summary, "boot off · DNS auto · MTU auto");
        let home = &view.profiles[2];
        assert_eq!(home.expander.badge, Some(Badge::Problem));
        assert!(home.expander.open);
        assert_eq!(home.mtu.fix.as_deref(), Some("1420"));
        assert_eq!(
            view.dns.summary,
            "127.0.0.53:53 · fallback auto · 1 bypass name"
        );
        assert_eq!(view.bypass, ["mail.contoso.example"]);
        assert_eq!(view.cache.summary, "max TTL 300 s · idle expiry 24 h");
        assert_eq!(
            view.about.title,
            format!("SplitWire v{}", env!("CARGO_PKG_VERSION"))
        );
        assert_eq!(view.about.summary, "wireguard-nt 1.1 · service since 09:12");
        assert!(view.apply_bar.is_none());
    }

    #[test]
    fn an_invalid_file_opens_routes_and_cache_with_the_fix() {
        let view = view("settingsinvalid", &SettingsEdit::default());
        assert!(view.message.is_some());
        assert!(view.cache.open);
        assert_eq!(view.cache.badge, Some(Badge::Problem));
        assert_eq!(view.max_ttl.fix.as_deref(), Some("300"));
        assert!(view.apply_bar.unwrap().bad);
        assert_eq!(
            view.profiles[0].expander.description,
            "Off · splitwire.conf is invalid"
        );
    }

    #[test]
    fn edits_mark_their_group_changed() {
        let mut edit = SettingsEdit::default();
        edit.set(
            SettingsEdit::field(SettingsScope::Global, "bypass"),
            "a.example.com".into(),
        );
        let view = view("normal", &edit);
        assert_eq!(view.dns.badge, Some(Badge::Changed));
        assert!(view.bypass_changed);
        assert_eq!(
            view.apply_bar.unwrap().text,
            "1 change not applied · profile changes take effect at their next Connect"
        );
    }
}
