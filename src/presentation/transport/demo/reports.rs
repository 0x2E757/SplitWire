//! The demo service's answers built from its state: status, a profile page, the routes and the
//! settings.

use std::time::Duration;

use super::Scenario;
use super::lists::DemoLists;
use super::model::{DemoModel, DemoProfile};
use crate::config::{KeyValueFile, ProfileSettings, SettingsScope};
use crate::ipc::{
    Alert, AlertKind, Counters, NrptStatus, ProfileDetail, ProfileRoutes, ProfileState,
    ProfileSummary, Revisions, RouteGroup, RouteGroupKey, RouteRow, ScopeSettings, SettingsView,
    Severity, StatusReport, TrayState,
};
use crate::list::{ListConflicts, ListSet};
use crate::profile::{Base64, KnownProfile, ProfileFacts, ProfileName};
use crate::stats::{QueryCounters, UpstreamHealthState};
use crate::tunnel::DriverVersion;

pub struct DemoReports;

impl DemoReports {
    pub fn status(model: &DemoModel, revs: Revisions) -> StatusReport {
        StatusReport {
            version: env!("CARGO_PKG_VERSION").into(),
            wgnt_version: Some(DriverVersion { major: 1, minor: 1 }),
            since: model.since,
            tray_state: Self::tray_state(model),
            revs,
            counters: Counters {
                queries: QueryCounters {
                    total: model.queries,
                    listed: model.listed,
                    ..QueryCounters::default()
                },
                cache_entries: model.cache_entries,
                learned_routes: model.profiles.iter().map(DemoProfile::learned).sum(),
            },
            nrpt: NrptStatus {
                rules: model.nrpt_rules,
                note: model.nrpt_note.clone(),
            },
            fallback: model.fallback.clone(),
            problems: model.global_problems(),
            alerts: model.alerts.clone(),
            profiles: model.profiles.iter().map(Self::summary).collect(),
        }
    }

    /// First match over the alerts not dismissed.
    pub fn tray_state(model: &DemoModel) -> TrayState {
        let active: Vec<&Alert> = model.alerts.iter().filter(|a| !a.dismissed).collect();
        let connected = |name: &Option<String>| {
            name.as_deref()
                .and_then(|n| model.profile(n))
                .is_some_and(DemoProfile::connected)
        };
        if active.iter().any(|a| {
            (a.severity == Severity::Error && connected(&a.profile))
                || matches!(
                    a.kind,
                    AlertKind::ConnectFailed | AlertKind::SettingsInvalid
                )
        }) {
            TrayState::Error
        } else if active.iter().any(|a| a.severity != Severity::Info)
            || model
                .profiles
                .iter()
                .any(|p| matches!(p.state, ProfileState::Connecting(_)))
        {
            TrayState::Warning
        } else if model.profiles.iter().any(DemoProfile::connected) {
            TrayState::Connected
        } else {
            TrayState::Off
        }
    }

    pub fn summary(profile: &DemoProfile) -> ProfileSummary {
        ProfileSummary {
            name: profile.name.clone(),
            colour: profile.colour,
            state: profile.state,
            since: profile.since,
            uptime_secs: profile.uptime_secs,
            last_connected: profile.last_connected,
            handshake_age_secs: profile.handshake_age_secs,
            rx_rate: profile.rx_rate,
            tx_rate: profile.tx_rate,
            rx_total: profile.rx_total,
            tx_total: profile.tx_total,
            learned: profile.learned(),
            dns_rtt_ms: profile.dns_rtt_ms,
            upstream: profile
                .connected()
                .then(|| profile.upstream().first().copied())
                .flatten(),
            keepalive: profile.keepalive,
            last_attempt: profile.attempt.as_ref().map(|a| a.summary()),
            problem: profile.settings_problems().first().map(|p| {
                format!(
                    "{} line {}: {} = {} {}",
                    ProfileSettings::FILE,
                    p.line,
                    p.key,
                    p.value,
                    p.reason
                )
            }),
        }
    }

    pub fn detail(model: &DemoModel, profile: &DemoProfile) -> ProfileDetail {
        let set = ListSet::load(model.lists());
        let footprints = DemoLists::footprints(model);
        let conflicts = ListConflicts::new(&set, &footprints);
        let settings = profile.parsed_settings();
        let connected = profile.connected();
        ProfileDetail {
            summary: Self::summary(profile),
            adapter: profile.adapter(),
            address: profile.address,
            mtu: settings.mtu.unwrap_or(profile.mtu),
            mtu_override: settings.mtu,
            endpoint: profile.endpoint.clone(),
            endpoint_addresses: profile.endpoint_addresses.clone(),
            interface_public_key: connected.then(|| profile.interface_key.clone()),
            peer_public_key: profile.peer_key.clone(),
            upstream: profile.upstream(),
            // The rest (failure count, RTT) keeps its default, so the struct may grow.
            upstream_health: UpstreamHealthState {
                last_success: connected.then(|| Scenario::now() - Duration::from_secs(4)),
                ..UpstreamHealthState::default()
            },
            keepalive: profile.keepalive,
            imported: profile.imported,
            list: DemoLists::file(profile, &set, &conflicts).lines,
            routes: Self::groups(profile),
            settings_problems: profile.settings_problems(),
            last_attempt: profile.attempt.clone(),
        }
    }

    /// Routes grouped by list line, then the Upstream DNS and Refused groups.
    pub fn groups(profile: &DemoProfile) -> Vec<RouteGroup> {
        let mut groups: Vec<RouteGroup> = Vec::new();
        for (index, text) in profile.list.lines().enumerate() {
            let line = index + 1;
            let rows: Vec<RouteRow> = profile
                .routes
                .iter()
                .filter(|r| r.line == Some(line))
                .map(|r| r.row.clone())
                .collect();
            if !rows.is_empty() {
                groups.push(RouteGroup {
                    key: RouteGroupKey::Line {
                        line,
                        text: text.trim().to_owned(),
                    },
                    rows,
                });
            }
        }
        for key in [RouteGroupKey::Upstream, RouteGroupKey::Refused] {
            let rows: Vec<RouteRow> = profile
                .routes
                .iter()
                .filter(|r| r.line.is_none())
                .filter(|r| {
                    matches!(
                        (&key, &r.row),
                        (RouteGroupKey::Upstream, RouteRow::Upstream { .. })
                            | (RouteGroupKey::Refused, RouteRow::Refused { .. })
                    )
                })
                .map(|r| r.row.clone())
                .collect();
            if !rows.is_empty() {
                groups.push(RouteGroup { key, rows });
            }
        }
        groups
    }

    pub fn routes(model: &DemoModel) -> Vec<ProfileRoutes> {
        model
            .profiles
            .iter()
            .map(|p| ProfileRoutes {
                name: p.name.clone(),
                groups: Self::groups(p),
            })
            .collect()
    }

    pub fn settings(model: &DemoModel, rev: u64) -> SettingsView {
        let scope = |scope: SettingsScope, file: &str, text: &str| ScopeSettings {
            scope,
            text: text.to_owned(),
            values: KeyValueFile::parse(file, text).entries,
        };
        let mut scopes = vec![scope(
            SettingsScope::Global,
            "splitwire.conf",
            &model.global,
        )];
        let mut problems = model.global_problems();
        for profile in &model.profiles {
            scopes.push(scope(
                SettingsScope::Profile(profile.name.clone()),
                &ProfileSettings::file_label(&profile.name),
                &profile.settings,
            ));
            problems.extend(profile.settings_problems());
        }
        SettingsView {
            rev,
            scopes,
            problems,
        }
    }

    /// The stored profiles as an import preview checks against them.
    pub fn known(model: &DemoModel) -> Vec<KnownProfile> {
        model
            .profiles
            .iter()
            .filter_map(|p| {
                Some(KnownProfile {
                    name: ProfileName::parse(&p.name).ok()?,
                    colour: p.colour,
                    facts: Some(ProfileFacts {
                        guid: 0,
                        address: p.address,
                        mtu: Some(p.mtu),
                        dns: p.dns.iter().map(|s| *s.ip()).collect(),
                        peer_public_key: Base64::decode_key(&p.peer_key).unwrap_or([0; 32]),
                        has_preshared_key: true,
                        endpoint: p.endpoint.clone(),
                        keepalive: p.keepalive,
                        imported: crate::clock::unix_secs(p.imported),
                    }),
                    connected: p.connected(),
                })
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::presentation::transport::demo::scenarios::DemoScenarios;

    #[test]
    fn tray_state_follows_spec_section_9() {
        let state = |s| DemoReports::tray_state(&DemoScenarios::model(s));
        assert_eq!(state(Scenario::Normal), TrayState::Warning);
        assert_eq!(state(Scenario::FirstRun), TrayState::Off);
        assert_eq!(state(Scenario::ConnectFail), TrayState::Error);
        assert_eq!(state(Scenario::SettingsInvalid), TrayState::Error);
    }

    #[test]
    fn the_profile_page_groups_routes_by_line() {
        let model = DemoScenarios::model(Scenario::Normal);
        let detail = DemoReports::detail(&model, &model.profiles[0]);
        assert_eq!(detail.adapter, "SplitWire-Contoso");
        let keys: Vec<&RouteGroupKey> = detail.routes.iter().map(|g| &g.key).collect();
        assert_eq!(keys.len(), 6);
        assert_eq!(keys[4], &RouteGroupKey::Upstream);
        assert_eq!(keys[5], &RouteGroupKey::Refused);
        let home = DemoReports::summary(&model.profiles[2]);
        assert!(
            home.problem
                .unwrap()
                .starts_with("profile.conf line 2: mtu = 9000")
        );
        let settings = DemoReports::settings(&model, 1);
        assert_eq!(settings.scopes.len(), 4);
        assert_eq!(settings.problems.len(), 1);
    }
}
