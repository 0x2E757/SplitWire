//! One sample of every message, with every optional field set in one sample and unset in another,
//! and every variant of every enum somewhere. Profiles come from the made-up fixture keys.

use std::net::{Ipv4Addr, SocketAddrV4};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use super::*;
use crate::config::{KeyValueEntry, SettingChange, SettingProblem, SettingsScope};
use crate::list::{ListSet, TextProblem};
use crate::logs::Level;
use crate::profile::fixtures::{
    OTHER_PRIVATE, PRESHARED, PRIVATE, conf, full_conf, sample_profile,
};
use crate::profile::{
    Base64, ImportContext, ImportPreview, ImportRequest, KnownProfile, Pin, Profile, ProfileName,
};
use crate::stats::{QueryCounters, UpstreamHealthState};
use crate::tunnel::{DriverVersion, HandshakeHealth};

/// A time with milliseconds, which the wire keeps.
pub fn at(secs: u64) -> SystemTime {
    UNIX_EPOCH + Duration::from_millis(1_792_400_000_000 + secs * 1000 + 250)
}

fn pin(number: u8) -> Pin {
    Pin::new(number).unwrap()
}

pub fn requests() -> Vec<Request> {
    let name = || "AcmeCorp".to_owned();
    vec![
        Request::Status,
        Request::ProfileStatus { name: name() },
        Request::Routes,
        Request::Traffic { after_seq: 1234 },
        Request::Logs { after: 0 },
        Request::AlertHistory,
        Request::DismissAlert {
            id: "address_held:AcmeCorp:192.0.2.93".into(),
        },
        Request::DismissAll,
        Request::RestoreAlerts {
            id: Some("gp_nrpt::".into()),
        },
        Request::RestoreAlerts { id: None },
        Request::GetAllLists,
        Request::EditList {
            base_rev: 7,
            name: name(),
            add: Some("corp.example.com".into()),
            remove: None,
        },
        Request::EditList {
            base_rev: 7,
            name: name(),
            add: None,
            remove: Some(LineRef {
                line: 4,
                text: "10.0.0.0/8 # office".into(),
            }),
        },
        Request::SaveList {
            base_rev: 8,
            name: name(),
            text: "corp.example.com\n# comment\n10.1.0.0/16\n".into(),
        },
        Request::SaveAllLists {
            base_rev: 9,
            text: "[profile AcmeCorp]\ncorp.example.com\n\n[profile Partner]\n".into(),
        },
        Request::GetSettings,
        Request::SaveSettings {
            base_rev: 3,
            changes: vec![
                SettingChange {
                    scope: SettingsScope::Global,
                    key: "max_ttl".into(),
                    value: Some("600".into()),
                },
                SettingChange {
                    scope: SettingsScope::Profile(name()),
                    key: "mtu".into(),
                    value: None,
                },
            ],
        },
        Request::ResetCache { name: Some(name()) },
        Request::ResetCache { name: None },
        Request::Connect { name: name() },
        Request::Disconnect {
            target: ProfileTarget::One(name()),
        },
        Request::Disconnect {
            target: ProfileTarget::All,
        },
        Request::PreviewImport {
            name: "Partner".into(),
            file_name: "Partner.conf".into(),
            conf_text: ConfText::new(full_conf()),
        },
        Request::Import {
            name: "Partner".into(),
            conf_text: ConfText::new(full_conf()),
            colour: pin(4),
            replace: true,
        },
        Request::RemoveProfile { name: name() },
        Request::SetProfileColor {
            name: name(),
            colour: pin(6),
        },
        Request::MoveProfile {
            name: name(),
            index: 2,
        },
    ]
}

pub fn responses() -> Vec<Response> {
    let mut responses = vec![
        Response::Done,
        Response::Status(Box::new(status())),
        Response::ProfileStatus(Box::new(detail())),
        Response::Routes(vec![
            ProfileRoutes {
                name: "AcmeCorp".into(),
                groups: route_groups(),
            },
            ProfileRoutes {
                name: "Partner".into(),
                groups: Vec::new(),
            },
        ]),
        Response::Traffic(vec![ProfileTraffic {
            name: "AcmeCorp".into(),
            samples: (1..=3)
                .map(|seq| TrafficSample {
                    seq,
                    rx_bps: seq * 1_000_000,
                    tx_bps: seq * 300,
                })
                .collect(),
        }]),
        Response::Logs(logs()),
        Response::AlertHistory(
            alerts()
                .into_iter()
                .filter(|a| a.cleared.is_some())
                .collect(),
        ),
        Response::Lists(Box::new(lists())),
        Response::Settings(Box::new(settings())),
        Response::SettingsSaved(Box::new(SettingsSaved {
            settings: settings(),
            reconnects: vec![
                Reconnect {
                    name: "AcmeCorp".into(),
                    outcome: ReconnectOutcome::Done,
                },
                Reconnect {
                    name: "Partner".into(),
                    outcome: ReconnectOutcome::Failed(attempt()),
                },
            ],
        })),
        Response::CacheReset {
            entries: 412,
            routes: 37,
        },
        Response::ConnectFailed {
            name: "AcmeCorp".into(),
            attempt: attempt(),
        },
        Response::Imported(Box::new(summaries().remove(0))),
    ];
    responses.extend(errors().into_iter().map(Response::Error));
    responses.extend(
        previews()
            .into_iter()
            .map(|preview| Response::ImportPreview(Box::new(preview))),
    );
    responses
}

fn errors() -> Vec<ErrorReply> {
    let reply = |code, message: &str| ErrorReply {
        code,
        message: message.into(),
    };
    vec![
        reply(ErrorCode::BadRequest, "the message is cut short"),
        reply(ErrorCode::UnknownProfile, "no profile named Acme"),
        reply(ErrorCode::StaleRev, "the lists changed on disk; reloaded"),
        reply(
            ErrorCode::Refused(vec![
                RefusedProblem::List(text_problem()),
                RefusedProblem::Setting(setting_problem()),
                RefusedProblem::Text("Overlaps AcmeCorp's tunnel 10.88.4.0/24".into()),
            ]),
            "nothing was written",
        ),
        reply(ErrorCode::Refused(Vec::new()), ""),
    ]
}

/// Previews of the fixture `.conf`s: a new profile, a replace with changes and a reconnect
/// warning, and a broken file whose lines hold keys.
pub fn previews() -> Vec<ImportPreview> {
    let stored = sample_profile();
    let profiles = [KnownProfile {
        name: ProfileName::parse("AcmeCorp").unwrap(),
        colour: pin(1),
        facts: Some(stored.facts.clone()),
        connected: true,
    }];
    let lists = ListSet::load([("AcmeCorp", "corp.example.com\n")]);
    let answers = [("AcmeCorp".to_owned(), Ipv4Addr::new(198, 51, 100, 7))];
    let build = |name: &str, text: &str, replaced: Option<&Profile>| {
        let file_name = format!("{name}.conf");
        let request = ImportRequest {
            name,
            file_name: &file_name,
            conf_text: text,
        };
        ImportPreview::build(
            &request,
            &ImportContext {
                profiles: &profiles,
                lists: &lists,
                replaced,
                endpoint_addresses: &[Ipv4Addr::new(198, 51, 100, 7)],
                answers: &answers,
            },
        )
    };
    let replacement = conf(
        OTHER_PRIVATE,
        "10.88.4.12/24",
        "10.66.0.53",
        "vpn.example.com:51820",
    );
    let broken = format!(
        "{PRIVATE}\n[Interface]\nPrivateKey = {PRIVATE}x\n[Peer]\nPresharedKey={PRESHARED}\n"
    );
    vec![
        build("Partner", &full_conf(), None),
        build("AcmeCorp", &replacement, Some(&stored)),
        build("Broken", &broken, None),
    ]
}

fn status() -> StatusReport {
    StatusReport {
        version: "0.1.0".into(),
        wgnt_version: Some(DriverVersion { major: 1, minor: 1 }),
        since: at(0),
        tray_state: TrayState::Warning,
        revs: Revisions {
            profiles: 4,
            lists: 12,
            settings: 2,
            alerts: 31,
        },
        counters: Counters {
            queries: QueryCounters {
                total: 9000,
                listed: 1200,
                bypassed: 30,
                other: 7700,
                failed: 70,
                cache_hits: 5000,
            },
            cache_entries: 412,
            learned_routes: 37,
        },
        nrpt: NrptStatus {
            rules: 14,
            note: Some("Group Policy rules exist".into()),
        },
        fallback: vec![Ipv4Addr::new(192, 168, 1, 1), Ipv4Addr::new(1, 1, 1, 1)],
        problems: vec![setting_problem()],
        alerts: alerts(),
        profiles: summaries(),
    }
}

/// One profile per state: the first has every optional field, the second none.
pub fn summaries() -> Vec<ProfileSummary> {
    let full = ProfileSummary {
        name: "AcmeCorp".into(),
        colour: pin(1),
        state: ProfileState::Connected(HandshakeHealth::Ok),
        since: Some(at(10)),
        uptime_secs: Some(3590),
        last_connected: Some(at(10)),
        handshake_age_secs: Some(42),
        rx_rate: 1_250_000,
        tx_rate: 64_000,
        rx_total: u64::MAX,
        tx_total: 1 << 40,
        learned: 37,
        dns_rtt_ms: Some(18),
        upstream: Some(SocketAddrV4::new(Ipv4Addr::new(10, 66, 0, 53), 5353)),
        keepalive: 25,
        last_attempt: Some(attempt().summary()),
        problem: Some("profile.conf line 3: mtu must be a number 1280-1500".into()),
    };
    let empty = ProfileSummary {
        name: "Partner".into(),
        colour: pin(2),
        state: ProfileState::Off,
        since: None,
        uptime_secs: None,
        last_connected: None,
        handshake_age_secs: None,
        rx_rate: 0,
        tx_rate: 0,
        rx_total: 0,
        tx_total: 0,
        learned: 0,
        dns_rtt_ms: None,
        upstream: None,
        keepalive: 0,
        last_attempt: None,
        problem: None,
    };
    let others = [
        ProfileState::Connecting(ConnectStep::Peer),
        ProfileState::Connected(HandshakeHealth::Handshaking),
        ProfileState::Connected(HandshakeHealth::Stale),
        ProfileState::Disconnecting,
        ProfileState::Failed,
        ProfileState::Blocked,
        ProfileState::HeldOff,
    ];
    let mut summaries = vec![full.clone(), empty];
    summaries.extend(
        others
            .iter()
            .enumerate()
            .map(|(index, state)| ProfileSummary {
                name: format!("P{index}"),
                colour: pin(index as u8 % 6 + 1),
                state: *state,
                ..full.clone()
            }),
    );
    summaries
}

pub fn attempt() -> Attempt {
    let results = [
        StepResult::Undone,
        StepResult::Skipped,
        StepResult::Undone,
        StepResult::Failed,
        StepResult::NotRun,
    ];
    let mut attempt = Attempt {
        at: at(5),
        kind: AttemptKind::Connect,
        result: AttemptResult::Failed,
        step: Some(ConnectStep::Adapter),
        error: Some("WireGuardCreateAdapter failed: access denied (5)".into()),
        steps: ConnectStep::ALL
            .iter()
            .enumerate()
            .map(|(index, step)| StepOutcome {
                step: *step,
                result: results[index.min(4)],
                detail: if index < 4 {
                    format!("step {}", step.id())
                } else {
                    String::new()
                },
            })
            .collect(),
    };
    attempt.steps[9].result = StepResult::Done;
    attempt
}

/// One alert of every kind, with every action target, every severity, cleared and dismissed ones.
fn alerts() -> Vec<Alert> {
    let targets = [
        Some(ActionTarget::Status),
        Some(ActionTarget::Profile {
            name: "AcmeCorp".into(),
            field: Some("mtu".into()),
        }),
        Some(ActionTarget::Profile {
            name: "AcmeCorp".into(),
            field: None,
        }),
        Some(ActionTarget::Settings(Some(SettingField {
            scope: SettingsScope::Global,
            key: "max_ttl".into(),
        }))),
        Some(ActionTarget::Settings(None)),
        Some(ActionTarget::Logs {
            profile: Some("AcmeCorp".into()),
        }),
        Some(ActionTarget::Logs { profile: None }),
        None,
    ];
    AlertKind::ALL
        .iter()
        .enumerate()
        .map(|(index, kind)| Alert {
            id: format!("{index}:AcmeCorp:192.0.2.93"),
            kind: *kind,
            severity: Severity::ALL[index % 3],
            profile: (index % 2 == 0).then(|| "AcmeCorp".to_owned()),
            since: at(index as u64),
            cleared: (index % 3 == 0).then(|| at(100 + index as u64)),
            title: "Address held by another profile".into(),
            message: "192.0.2.93 is held by Partner".into(),
            action: targets[index % targets.len()]
                .clone()
                .map(|target| AlertAction {
                    label: "Open settings".into(),
                    target,
                }),
            suggest: (index % 2 == 1).then(|| Suggestion {
                field: SettingField {
                    scope: SettingsScope::Profile("AcmeCorp".into()),
                    key: "mtu".into(),
                },
                value: "1420".into(),
            }),
            blocking: index < 2,
            dismissed: index % 4 == 3,
        })
        .collect()
}

/// The profile page of the fixture profile, mapped from its facts as the engine would.
fn detail() -> ProfileDetail {
    let facts = sample_profile().facts;
    ProfileDetail {
        summary: summaries().remove(0),
        adapter: "SplitWire-AcmeCorp".into(),
        address: facts.address,
        mtu: facts.mtu_or_default(),
        mtu_override: Some(1380),
        endpoint: facts.endpoint.clone(),
        endpoint_addresses: vec![Ipv4Addr::new(198, 51, 100, 7)],
        interface_public_key: Some(Base64::encode(&[7; 32])),
        peer_public_key: Base64::encode(&facts.peer_public_key),
        upstream: vec![
            "10.66.0.53:53".parse().unwrap(),
            "10.66.0.54:5353".parse().unwrap(),
        ],
        upstream_health: UpstreamHealthState {
            last_success: Some(at(50)),
            last_failure: Some(at(40)),
            last_error: Some("timed out".into()),
            consecutive_failures: 2,
            rtt: Some(Duration::from_millis(18)),
        },
        keepalive: facts.keepalive,
        imported: UNIX_EPOCH + Duration::from_secs(facts.imported),
        list: list_lines(),
        routes: route_groups(),
        settings_problems: vec![setting_problem()],
        last_attempt: Some(attempt()),
    }
}

fn list_lines() -> Vec<ListLine> {
    let lines = [
        (
            ListLineKind::Domain,
            "corp.example.com",
            Some("inside example.com (line 2)"),
        ),
        (ListLineKind::Address, "10.1.2.3", None),
        (
            ListLineKind::Network,
            "10.1.0.0/16",
            Some("overrides Partner line 4 while both are connected"),
        ),
        (ListLineKind::Comment, "# office", None),
        (ListLineKind::Blank, "", None),
        (
            ListLineKind::Problem,
            "intranet",
            Some("a domain needs at least 2 labels"),
        ),
    ];
    lines
        .iter()
        .enumerate()
        .map(|(index, (kind, text, note))| ListLine {
            line: index + 1,
            text: (*text).into(),
            kind: *kind,
            note: note.map(str::to_owned),
            routes: index as u64 * 3,
        })
        .collect()
}

fn route_groups() -> Vec<RouteGroup> {
    vec![
        RouteGroup {
            key: RouteGroupKey::Line {
                line: 1,
                text: "corp.example.com".into(),
            },
            rows: vec![
                RouteRow::Learned {
                    ip: Ipv4Addr::new(10, 1, 2, 3),
                    name: "git.corp.example.com".into(),
                    last_seen: at(60),
                    expires: at(86_460),
                },
                RouteRow::Held {
                    ip: Ipv4Addr::new(192, 0, 2, 93),
                    name: "cdn.corp.example.com".into(),
                    by: "Partner".into(),
                    by_name: "cdn.partner.example".into(),
                },
            ],
        },
        RouteGroup {
            key: RouteGroupKey::Line {
                line: 3,
                text: "10.1.0.0/16".into(),
            },
            rows: vec![RouteRow::Static {
                net: "10.1.0.0/16".parse().unwrap(),
            }],
        },
        RouteGroup {
            key: RouteGroupKey::Upstream,
            rows: vec![RouteRow::Upstream {
                ip: Ipv4Addr::new(10, 66, 0, 53),
            }],
        },
        RouteGroup {
            key: RouteGroupKey::Refused,
            rows: vec![
                RouteRow::Refused {
                    net: "192.168.1.0/24".parse().unwrap(),
                    name: None,
                    reason: "on-link network of Ethernet".into(),
                },
                RouteRow::Refused {
                    net: "198.51.100.7/32".parse().unwrap(),
                    name: Some("vpn.corp.example.com".into()),
                    reason: "covers Partner's endpoint".into(),
                },
            ],
        },
    ]
}

fn logs() -> LogsPage {
    LogsPage {
        entries: [Level::Info, Level::Warn, Level::Error]
            .iter()
            .enumerate()
            .map(|(index, level)| LogLine {
                seq: 100 + index as u64,
                time: at(index as u64),
                level: *level,
                profile: (index > 0).then(|| "AcmeCorp".to_owned()),
                source: "routes".into(),
                message: "added 10.1.2.3/32 for git.corp.example.com".into(),
            })
            .collect(),
        last_seq: 102,
    }
}

fn lists() -> ListsView {
    ListsView {
        rev: 12,
        text: "[profile AcmeCorp]\ncorp.example.com\n".into(),
        files: vec![
            ListFile {
                name: "AcmeCorp".into(),
                lines: list_lines(),
            },
            ListFile {
                name: "Partner".into(),
                lines: Vec::new(),
            },
        ],
        problems: vec![
            text_problem(),
            TextProblem {
                text_line: Some(1),
                profile: None,
                file_line: None,
                text: "intranet".into(),
                reason: "lines before the first header".into(),
                other: None,
                blocking: true,
            },
        ],
    }
}

fn settings() -> SettingsView {
    SettingsView {
        rev: 2,
        scopes: vec![
            ScopeSettings {
                scope: SettingsScope::Global,
                text: "# global\nmax_ttl = 30000000\n".into(),
                values: vec![KeyValueEntry {
                    line: 2,
                    key: "max_ttl".into(),
                    value: "30000000".into(),
                }],
            },
            ScopeSettings {
                scope: SettingsScope::Profile("AcmeCorp".into()),
                text: String::new(),
                values: Vec::new(),
            },
        ],
        problems: vec![setting_problem()],
    }
}

fn text_problem() -> TextProblem {
    TextProblem {
        text_line: Some(14),
        profile: Some("AcmeCorp".into()),
        file_line: Some(4),
        text: "corp.example.com".into(),
        reason: "also in Partner line 2: a name belongs to one profile".into(),
        other: Some(("Partner".into(), 2)),
        blocking: true,
    }
}

fn setting_problem() -> SettingProblem {
    SettingProblem {
        file: "splitwire.conf".into(),
        line: 2,
        key: "max_ttl".into(),
        value: "30000000".into(),
        reason: "must be a number 30-86400".into(),
        suggest: Some("300".into()),
    }
}
