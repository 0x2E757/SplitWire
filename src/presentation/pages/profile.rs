//! The profile page: header, page messages, the step table of a failed
//! Connect, the tunnel strip, the traffic chart, a blocked profile's settings, the list card with
//! its routes, and the profile settings.

use super::{InfoBar, RouteGroupView, RouteGroups, SwitchView};
use crate::config::{ProfileSettings, SettingsScope};
use crate::ipc::{
    AlertKind, ListLineKind, ProfileDetail, ProfileState, SettingsView, StatusReport, TrafficSample,
};
use crate::presentation::{
    AlertList, AlertView, ApplyBarView, ChartView, EntryVerdict, Format, LocalTime, Pill,
    SettingsEdit, StateText, StepTableView, TrafficChart,
};
use crate::profile::Pin;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StripView {
    pub endpoint: String,
    /// `→ 203.0.113.62 · 10.99.0.46/32 · MTU 1420`.
    pub route: String,
    pub connected: bool,
    pub handshake: String,
    pub receive: String,
    pub send: String,
    pub upstream: String,
    pub rtt: Option<String>,
}

/// A settings field of the page: its value, placeholder and problem.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProfileField {
    pub value: String,
    pub placeholder: String,
    pub problem: Option<String>,
    pub changed: bool,
    /// "Use 1420": fills the field.
    pub fix: Option<String>,
}

/// A blocked profile's settings cards: the bad line marked in the shown file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BlockedSettings {
    pub boot: bool,
    pub upstream: ProfileField,
    pub mtu: ProfileField,
    pub file: String,
    /// `(line, text, bad, comment)`.
    pub lines: Vec<(usize, String, bool, bool)>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProfilePageView {
    pub name: String,
    pub colour: Pin,
    pub subtitle: String,
    pub pill: Pill,
    pub switch: SwitchView,
    pub global_message: Option<InfoBar>,
    pub failure: Option<StepTableView>,
    pub alerts: Vec<AlertView>,
    pub strip: StripView,
    pub chart_title: String,
    pub chart_note: String,
    pub chart: ChartView,
    pub blocked: Option<BlockedSettings>,
    pub apply_bar: Option<ApplyBarView>,
    pub add_value: String,
    pub add_verdict: EntryVerdict,
    pub groups: Vec<RouteGroupView>,
    pub footer: String,
    /// `boot on · upstream auto (10.66.0.53) · MTU 1420`.
    pub settings_summary: String,
    pub settings_open: bool,
    pub boot: bool,
    pub upstream: ProfileField,
    pub mtu: ProfileField,
    pub imported: String,
}

pub struct ProfileSource<'a> {
    pub status: &'a StatusReport,
    pub detail: &'a ProfileDetail,
    pub settings: Option<&'a SettingsView>,
    pub edit: &'a SettingsEdit,
    pub traffic: Vec<TrafficSample>,
    pub add: &'a str,
    pub verdict: EntryVerdict,
    pub search: &'a str,
    pub settings_open: bool,
    pub time: &'a LocalTime,
}

impl ProfileSource<'_> {
    pub fn view(&self) -> ProfilePageView {
        let detail = self.detail;
        let summary = &detail.summary;
        let name = &summary.name;
        let connected = matches!(summary.state, ProfileState::Connected(_));
        let global_invalid = !self.status.problems.is_empty();
        let others: Vec<&str> = self
            .status
            .profiles
            .iter()
            .filter(|p| p.name != *name && matches!(p.state, ProfileState::Connected(_)))
            .map(|p| p.name.as_str())
            .collect();
        let failure = detail
            .last_attempt
            .as_ref()
            .filter(|_| summary.state == ProfileState::Failed)
            .map(|a| StepTableView::of(a, &others, self.time));
        let alerts = AlertList::new(&self.status.alerts)
            .active(Some(Some(name)))
            .into_iter()
            .filter(|a| !(failure.is_some() && a.kind == AlertKind::ConnectFailed))
            .map(|a| crate::presentation::AlertView::of(a, self.time))
            .collect();
        let scope = SettingsScope::Profile(name.clone());
        let field = |key: &str| SettingsEdit::field(scope.clone(), key);
        let upstream_default = detail
            .upstream
            .iter()
            .map(|s| s.ip().to_string())
            .collect::<Vec<_>>()
            .join(", ");
        let conf_mtu = detail.mtu;
        let make_field = |key: &str, placeholder: String, fix: Option<String>| {
            let f = field(key);
            let problem = self.edit.problem(self.settings, &f);
            ProfileField {
                value: self.edit.value(self.settings, &f),
                placeholder,
                fix: fix.filter(|_| problem.is_some()),
                problem,
                changed: self.edit.changed(self.settings, &f),
            }
        };
        let mtu_fix = SettingsEdit::file_problem(self.settings, &field("mtu"))
            .and_then(|p| p.suggest.clone());
        let upstream = make_field("upstream", format!("Automatic · {upstream_default}"), None);
        let mtu = make_field("mtu", format!("Automatic · {conf_mtu}"), mtu_fix);
        let boot = self.edit.value(self.settings, &field("connect_at_boot")) == "true";
        let blocked = (summary.state == ProfileState::Blocked).then(|| {
            let text = self
                .settings
                .and_then(|v| v.scopes.iter().find(|s| s.scope == scope))
                .map(|s| s.text.clone())
                .unwrap_or_default();
            let bad: Vec<usize> = detail.settings_problems.iter().map(|p| p.line).collect();
            BlockedSettings {
                boot,
                upstream: upstream.clone(),
                mtu: mtu.clone(),
                file: ProfileSettings::file_label(name),
                lines: text
                    .lines()
                    .enumerate()
                    .map(|(i, l)| {
                        (
                            i + 1,
                            l.to_owned(),
                            bad.contains(&(i + 1)),
                            l.trim_start().starts_with('#'),
                        )
                    })
                    .collect(),
            }
        });
        let entries = detail
            .list
            .iter()
            .filter(|l| !matches!(l.kind, ListLineKind::Comment | ListLineKind::Blank))
            .count();
        let endpoint_ip = detail
            .endpoint_addresses
            .first()
            .map_or_else(|| "not resolved yet".to_owned(), |ip| ip.to_string());
        let dash = || "—".to_owned();
        ProfilePageView {
            name: name.clone(),
            colour: summary.colour,
            subtitle: format!("{} · adapter {}", detail.endpoint, detail.adapter),
            pill: StateText::pill(summary, self.time),
            switch: SwitchView::of(summary, global_invalid),
            global_message: global_invalid.then(|| {
                InfoBar::new(
                    "Every profile is disconnected",
                    "splitwire.conf is invalid, so no profile can connect until it is fixed.",
                )
            }),
            failure,
            alerts,
            strip: StripView {
                endpoint: detail.endpoint.to_string(),
                route: format!(
                    "→ {endpoint_ip} · {}/{} · MTU {}",
                    detail.address.address, detail.address.prefix, detail.mtu
                ),
                connected,
                handshake: if connected {
                    summary.handshake_age_secs.map_or_else(dash, Format::ago)
                } else {
                    dash()
                },
                receive: if connected {
                    Format::rate(summary.rx_rate)
                } else {
                    dash()
                },
                send: if connected {
                    Format::rate(summary.tx_rate)
                } else {
                    dash()
                },
                upstream: upstream_default.clone(),
                rtt: summary
                    .dns_rtt_ms
                    .filter(|_| connected)
                    .map(|ms| format!("{ms} ms")),
            },
            chart_title: format!("{name} tunnel traffic"),
            chart_note: if connected {
                format!(
                    "{} received · {} sent · scale 0–{}, grows with peak",
                    Format::bytes(summary.rx_total),
                    Format::bytes(summary.tx_total),
                    TrafficChart::of(&self.traffic, self.time).scale_label
                )
            } else {
                String::new()
            },
            chart: match summary.state {
                ProfileState::Connected(_) => TrafficChart::of(&self.traffic, self.time),
                ProfileState::Connecting(_) => TrafficChart::empty(
                    "Handshaking…",
                    "waiting for the first handshake",
                    self.time,
                ),
                _ => TrafficChart::empty(
                    "Not connected",
                    &StateText::last_connected(summary, self.time)
                        .unwrap_or_else(|| "never connected".into()),
                    self.time,
                ),
            },
            apply_bar: self
                .edit
                .bar(self.settings, Some(self.status), Some(&scope)),
            blocked,
            add_value: self.add.to_owned(),
            add_verdict: self.verdict.clone(),
            groups: RouteGroups {
                profile: name,
                lines: &detail.list,
                groups: &detail.routes,
                connected,
                search: self.search,
                time: self.time,
            }
            .views(false),
            footer: format!(
                "{} in profiles\\{name}\\routes.txt · a listed name goes to the connected profile with the most specific match",
                Format::plural(entries, "line", "lines")
            ),
            settings_summary: format!(
                "boot {} · upstream {} · MTU {}",
                if boot { "on" } else { "off" },
                if upstream.value.is_empty() {
                    format!("auto ({upstream_default})")
                } else {
                    upstream.value.clone()
                },
                if mtu.value.is_empty() {
                    conf_mtu.to_string()
                } else {
                    mtu.value.clone()
                }
            ),
            settings_open: self.settings_open,
            boot,
            upstream,
            mtu,
            imported: self.time.date(detail.imported),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::presentation::samples::Samples;

    fn view(key: &str, name: &str, edit: &SettingsEdit) -> ProfilePageView {
        let status = Samples::status(key);
        let detail = Samples::detail(key, name);
        let settings = Samples::settings(key);
        ProfileSource {
            status: &status,
            detail: &detail,
            settings: Some(&settings),
            edit,
            traffic: Vec::new(),
            add: "",
            verdict: EntryVerdict::Empty,
            search: "",
            settings_open: false,
            time: &Samples::time(),
        }
        .view()
    }

    #[test]
    fn a_connected_profile_shows_its_strip_and_groups() {
        let view = view("normal", "Contoso", &SettingsEdit::default());
        assert_eq!(
            view.subtitle,
            "vpn.contoso.example:51820 · adapter SplitWire-Contoso"
        );
        assert_eq!(view.pill.tail.as_deref(), Some("2 h 14 min"));
        assert_eq!(
            view.strip.route,
            "→ 203.0.113.62 · 10.99.0.46/32 · MTU 1420"
        );
        assert_eq!(view.strip.handshake, "42 s ago");
        assert_eq!(view.strip.receive, "38 KB/s");
        assert_eq!(view.strip.rtt.as_deref(), Some("18 ms"));
        assert_eq!(view.groups.len(), 6);
        assert_eq!(view.alerts.len(), 1);
        assert!(
            view.footer
                .starts_with("4 lines in profiles\\Contoso\\routes.txt")
        );
        assert_eq!(
            view.settings_summary,
            "boot on · upstream auto (10.66.0.53) · MTU 1420"
        );
        assert!(view.blocked.is_none() && view.apply_bar.is_none());
    }

    #[test]
    fn a_blocked_profile_marks_the_bad_line_and_offers_the_fix() {
        let mut edit = SettingsEdit::default();
        let view = view("normal", "HomeLab", &edit);
        let blocked = view.blocked.unwrap();
        assert_eq!(blocked.lines[1], (2, "mtu = 9000".into(), true, false));
        assert!(blocked.lines[2].3);
        assert_eq!(blocked.mtu.fix.as_deref(), Some("1420"));
        assert!(
            blocked
                .mtu
                .problem
                .unwrap()
                .starts_with("profile.conf line 2: mtu = 9000")
        );
        let bar = view.apply_bar.unwrap();
        assert!(
            bar.bad
                && bar
                    .text
                    .contains("must be fixed before HomeLab can connect")
        );
        assert_eq!(
            view.chart.empty.unwrap().1,
            "last connected yesterday 22:10"
        );
        edit.set(
            SettingsEdit::field(SettingsScope::Profile("HomeLab".into()), "mtu"),
            "1420".into(),
        );
        let fixed = super::tests::view("normal", "HomeLab", &edit);
        assert!(fixed.apply_bar.unwrap().can_apply);
        assert!(fixed.blocked.unwrap().mtu.problem.is_none());
    }

    #[test]
    fn a_failed_connect_shows_the_step_table_instead_of_its_alert() {
        let view = view("connectfail", "HomeLab", &SettingsEdit::default());
        let failure = view.failure.unwrap();
        assert_eq!(failure.title, "Connect failed at step 3 of 10 · guard");
        assert!(view.alerts.is_empty());
        let invalid = super::tests::view("settingsinvalid", "AcmeCorp", &SettingsEdit::default());
        assert!(invalid.global_message.is_some());
        assert!(!invalid.switch.enabled);
    }
}
