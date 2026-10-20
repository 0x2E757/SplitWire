//! The demo's sample log: the ring buffer of this service run, and
//! `logs\splitwire.log` with warnings and errors of earlier days.

use std::time::{Duration, SystemTime};

use super::Scenario;

use crate::ipc::LogLine;
use crate::logs::{Level, LogLineText};

const B: &str = "Contoso";
const A: &str = "AcmeCorp";
const H: &str = "HomeLab";
const G: &str = "";

type Row = (
    &'static str,
    Level,
    &'static str,
    &'static str,
    &'static str,
);

const BASE: &[Row] = &[
    (
        "09:12:03.114",
        Level::Info,
        G,
        "service",
        "started, version 0.1.0",
    ),
    (
        "09:12:03.120",
        Level::Info,
        G,
        "service",
        "cleanup: removed 0 NRPT rules, 0 routes",
    ),
    (
        "09:12:03.126",
        Level::Info,
        G,
        "service",
        "3 profiles: Contoso, AcmeCorp, HomeLab",
    ),
    (
        "09:12:03.131",
        Level::Error,
        H,
        "settings",
        "profile.conf line 2: mtu = 9000: must be 1280–1500; HomeLab cannot connect",
    ),
    ("09:12:03.140", Level::Info, B, "list", "loaded: 4 entries"),
    (
        "09:12:03.141",
        Level::Info,
        A,
        "list",
        "loaded: 4 entries; line 4 overrides Contoso line 2 while both are connected",
    ),
    ("09:12:03.142", Level::Info, H, "list", "loaded: 1 entry"),
    (
        "09:14:10.402",
        Level::Info,
        B,
        "connect",
        "started (connect at boot)",
    ),
    (
        "09:14:10.415",
        Level::Info,
        G,
        "dns",
        "proxy listening on 127.0.0.53:53 (UDP, TCP)",
    ),
    (
        "09:14:10.598",
        Level::Info,
        B,
        "wg",
        "driver 1.1 loaded, adapter SplitWire-Contoso created",
    ),
    (
        "09:14:10.611",
        Level::Info,
        B,
        "tunnel",
        "address 10.99.0.46/32, MTU 1420",
    ),
    (
        "09:14:10.744",
        Level::Info,
        B,
        "tunnel",
        "endpoint vpn.contoso.example → 203.0.113.62 (via 192.168.1.1)",
    ),
    (
        "09:14:11.032",
        Level::Info,
        B,
        "wg",
        "Receiving handshake response from peer 1 (203.0.113.62:51820)",
    ),
    (
        "09:14:11.040",
        Level::Info,
        B,
        "routes",
        "added 10.66.0.53/32 (upstream DNS), 10.66.0.0/16, 172.16.40.12/32",
    ),
    (
        "09:14:11.052",
        Level::Info,
        G,
        "nrpt",
        "2 rules set (contoso.example, shop-contoso.example)",
    ),
    (
        "09:14:11.060",
        Level::Warn,
        G,
        "nrpt",
        "Group Policy defines NRPT rules; local rules are ignored",
    ),
    (
        "09:14:11.071",
        Level::Info,
        B,
        "connect",
        "connected in 0.67 s",
    ),
    (
        "09:15:02.311",
        Level::Info,
        B,
        "routes",
        "learned 192.0.2.93/32 ← contoso.example",
    ),
    (
        "09:15:02.540",
        Level::Info,
        B,
        "routes",
        "learned 203.0.113.7/32 ← jira.contoso.example",
    ),
    (
        "09:16:20.902",
        Level::Info,
        B,
        "routes",
        "learned 10.66.4.15/32 ← intranet.contoso.example",
    ),
    (
        "09:21:47.009",
        Level::Warn,
        B,
        "routes",
        "refused 192.168.1.20 ← printer.contoso.example: on-link network of Ethernet 192.168.1.0/24",
    ),
    (
        "09:40:05.882",
        Level::Info,
        B,
        "routes",
        "learned 198.51.100.71/32 ← www.shop-contoso.example",
    ),
    (
        "10:16:11.205",
        Level::Info,
        B,
        "wg",
        "Receiving handshake response from peer 1 (203.0.113.62:51820)",
    ),
    ("10:41:02.118", Level::Info, A, "connect", "started"),
    (
        "10:41:02.130",
        Level::Info,
        A,
        "guard",
        "added Contoso’s endpoint and tunnel; Contoso’s guard gains AcmeCorp’s",
    ),
    (
        "10:41:02.301",
        Level::Info,
        A,
        "wg",
        "adapter SplitWire-AcmeCorp created",
    ),
    (
        "10:41:02.312",
        Level::Info,
        A,
        "tunnel",
        "address 10.88.4.12/32, MTU 1380",
    ),
    (
        "10:41:02.455",
        Level::Info,
        A,
        "tunnel",
        "endpoint vpn.acme.example → 203.0.113.25 (via 192.168.1.1)",
    ),
    (
        "10:41:02.690",
        Level::Info,
        A,
        "wg",
        "Receiving handshake response from peer 1 (203.0.113.25:51820)",
    ),
    (
        "10:41:02.701",
        Level::Info,
        A,
        "routes",
        "added 10.88.0.53/32 (upstream DNS), 172.20.0.0/16",
    ),
    (
        "10:41:02.712",
        Level::Info,
        A,
        "dns",
        "upstream 10.88.0.53 bound to SplitWire-AcmeCorp (10.88.4.12)",
    ),
    (
        "10:41:02.720",
        Level::Info,
        A,
        "matcher",
        "partners.contoso.example now goes to AcmeCorp; re-homed 0 routes",
    ),
    (
        "10:41:02.731",
        Level::Info,
        G,
        "nrpt",
        "4 rules set (+ acme.example, intranet.acme.example)",
    ),
    (
        "10:41:02.740",
        Level::Info,
        A,
        "connect",
        "connected in 0.62 s",
    ),
    (
        "10:43:18.551",
        Level::Info,
        A,
        "routes",
        "learned 198.51.100.14/32 ← acme.example",
    ),
    (
        "10:43:19.007",
        Level::Info,
        A,
        "routes",
        "learned 10.88.12.5/32 ← intranet.acme.example",
    ),
    (
        "10:47:30.214",
        Level::Info,
        A,
        "routes",
        "learned 192.0.2.140/32 ← partners.contoso.example",
    ),
    (
        "10:52:08.663",
        Level::Info,
        B,
        "routes",
        "learned 198.51.100.201/32 ← admin.shop-contoso.example",
    ),
    (
        "11:02:10.330",
        Level::Warn,
        A,
        "tunnel",
        "handshake stale (183 s); re-resolving vpn.acme.example",
    ),
    (
        "11:02:10.512",
        Level::Info,
        A,
        "tunnel",
        "endpoint unchanged (203.0.113.25)",
    ),
    (
        "11:03:01.770",
        Level::Info,
        A,
        "wg",
        "Receiving handshake response from peer 1 (203.0.113.25:51820)",
    ),
    (
        "11:14:10.410",
        Level::Info,
        B,
        "routes",
        "reconcile: 13 learned, 2 static, 1 upstream; nothing to change",
    ),
    (
        "11:20:02.188",
        Level::Info,
        A,
        "routes",
        "learned 10.88.20.4/32 ← jira.acme.example",
    ),
    (
        "11:21:44.902",
        Level::Warn,
        A,
        "routes",
        "192.0.2.93 ← cdn.acme.example is held by Contoso (contoso.example); dropped from the answer",
    ),
    (
        "11:26:40.120",
        Level::Error,
        B,
        "dns",
        "upstream 10.66.0.53 did not answer in 2 s (jira.contoso.example A), SERVFAIL",
    ),
    (
        "11:26:52.447",
        Level::Info,
        B,
        "dns",
        "upstream 10.66.0.53 answers again (21 ms)",
    ),
    (
        "11:27:37.208",
        Level::Info,
        A,
        "wg",
        "Receiving handshake response from peer 1 (203.0.113.25:51820)",
    ),
    (
        "11:28:03.400",
        Level::Info,
        B,
        "routes",
        "learned 198.51.100.46/32 ← shop-contoso.example",
    ),
    (
        "11:28:03.512",
        Level::Info,
        B,
        "routes",
        "learned 203.0.113.140/32 ← api.shop-contoso.example",
    ),
    (
        "11:28:10.064",
        Level::Info,
        A,
        "routes",
        "learned 198.51.100.80/32 ← sso.acme.example",
    ),
    (
        "11:28:13.390",
        Level::Info,
        B,
        "wg",
        "Receiving handshake response from peer 1 (203.0.113.62:51820)",
    ),
    (
        "11:28:20.774",
        Level::Info,
        A,
        "routes",
        "learned 10.88.12.31/32 ← git.intranet.acme.example",
    ),
    (
        "11:28:55.390",
        Level::Info,
        B,
        "tunnel",
        "health: Connected, handshake 42 s ago",
    ),
    (
        "11:28:55.391",
        Level::Info,
        A,
        "tunnel",
        "health: Connected, handshake 1 min 18 s ago",
    ),
];

const CONNECT_FAIL: &[Row] = &[
    (
        "11:27:41.102",
        Level::Info,
        H,
        "connect",
        "started (by the user)",
    ),
    (
        "11:27:41.104",
        Level::Info,
        H,
        "proxy",
        "skipped: Contoso already runs the DNS proxy",
    ),
    (
        "11:27:41.118",
        Level::Error,
        H,
        "guard",
        "home.example.net did not resolve through the fallback DNS 192.168.1.1 (NXDOMAIN)",
    ),
    (
        "11:27:41.120",
        Level::Error,
        H,
        "connect",
        "failed at step 3 guard; undone: profile; Contoso and AcmeCorp unchanged",
    ),
];

const SETTINGS_BAD: &[Row] = &[
    (
        "11:25:01.004",
        Level::Info,
        G,
        "service",
        "stopping (update); disconnecting Contoso, AcmeCorp",
    ),
    (
        "11:25:01.310",
        Level::Info,
        G,
        "nrpt",
        "0 rules (removed 4)",
    ),
    (
        "11:25:02.880",
        Level::Info,
        G,
        "service",
        "started, version 0.1.0",
    ),
    (
        "11:25:02.886",
        Level::Error,
        G,
        "settings",
        "splitwire.conf line 4: max_ttl = 10: must be 30–86400; every profile stays disconnected",
    ),
    (
        "11:25:02.890",
        Level::Warn,
        G,
        "service",
        "not restoring Contoso, AcmeCorp: splitwire.conf is invalid",
    ),
    (
        "11:25:02.893",
        Level::Error,
        H,
        "settings",
        "profile.conf line 2: mtu = 9000: must be 1280–1500; HomeLab cannot connect",
    ),
];

/// Earlier days of `splitwire.log`: `(days ago, time, level, profile, source, message)`.
const FILE_OLD: &[(u64, &str, Level, &str, &str, &str)] = &[
    (
        2,
        "08:55:12.004",
        Level::Warn,
        G,
        "nrpt",
        "Group Policy defines NRPT rules; local rules are ignored",
    ),
    (
        2,
        "13:40:51.330",
        Level::Error,
        B,
        "connect",
        "failed at step 2 proxy: 127.0.0.53:53 is in use by another process (PID 4120); undone: profile",
    ),
    (
        2,
        "13:44:02.716",
        Level::Warn,
        B,
        "routes",
        "refused 192.168.1.20 ← printer.contoso.example: on-link network of Ethernet 192.168.1.0/24",
    ),
    (
        1,
        "21:02:44.118",
        Level::Warn,
        H,
        "tunnel",
        "handshake stale (181 s); re-resolving home.example.net",
    ),
    (
        1,
        "21:03:15.009",
        Level::Error,
        H,
        "tunnel",
        "endpoint home.example.net did not resolve through 192.168.1.1 (timeout)",
    ),
    (
        1,
        "22:10:07.552",
        Level::Warn,
        H,
        "connect",
        "disconnected by the user while Stale",
    ),
];

pub struct DemoLogs;

impl DemoLogs {
    /// `HH:MM:SS.mmm` of the scenario's day, `days` before it.
    fn time(clock: &str, days: u64) -> SystemTime {
        let part = |range: std::ops::Range<usize>| clock[range].parse::<u64>().unwrap_or(0);
        Scenario::at(part(0..2), part(3..5), part(6..8)) + Duration::from_millis(part(9..12))
            - Duration::from_secs(days * 86_400)
    }

    fn rows(scenario: Scenario) -> Vec<Row> {
        let mut rows: Vec<Row> = BASE.to_vec();
        match scenario {
            Scenario::ConnectFail => {
                rows.retain(|r| r.3 != "settings");
                rows.extend_from_slice(CONNECT_FAIL);
                rows.sort_by(|a, b| a.0.cmp(b.0));
            }
            Scenario::SettingsInvalid => {
                rows.retain(|r| r.0 < "11:25");
                rows.extend_from_slice(SETTINGS_BAD);
            }
            Scenario::FirstRun => rows.truncate(2),
            Scenario::Normal | Scenario::NoService => {}
        }
        rows
    }

    /// The ring buffer, oldest first. About 1150 earlier lines of this run are not drawn;
    /// the numbering starts after them.
    pub fn live(scenario: Scenario) -> Vec<LogLine> {
        Self::rows(scenario)
            .into_iter()
            .enumerate()
            .map(
                |(index, (clock, level, profile, source, message))| LogLine {
                    seq: 1151 + index as u64,
                    time: Self::time(clock, 0),
                    level,
                    profile: (!profile.is_empty()).then(|| profile.to_owned()),
                    source: source.into(),
                    message: message.into(),
                },
            )
            .collect()
    }

    /// `logs\splitwire.log`: warnings and errors, oldest first, one line each:
    /// `2026-10-19 09:14:11.060Z WARN  - nrpt: …`.
    pub fn file(scenario: Scenario) -> String {
        let mut lines: Vec<(SystemTime, Level, &str, &str, &str)> = FILE_OLD
            .iter()
            .map(|(days, clock, level, profile, source, message)| {
                (
                    Self::time(clock, *days),
                    *level,
                    *profile,
                    *source,
                    *message,
                )
            })
            .collect();
        lines.extend(
            Self::rows(scenario)
                .into_iter()
                .filter(|r| r.1 != Level::Info)
                .map(|(clock, level, profile, source, message)| {
                    (Self::time(clock, 0), level, profile, source, message)
                }),
        );
        lines
            .into_iter()
            .map(|(at, level, profile, source, message)| {
                let profile = (!profile.is_empty()).then_some(profile);
                format!(
                    "{}\r\n",
                    LogLineText::format(at, level, profile, source, message)
                )
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_live_log_is_numbered_and_the_file_keeps_warnings() {
        let live = DemoLogs::live(Scenario::Normal);
        assert_eq!(live.len(), BASE.len());
        assert_eq!(live[0].seq, 1151);
        assert_eq!(live[3].profile.as_deref(), Some("HomeLab"));
        let file = DemoLogs::file(Scenario::Normal);
        assert!(file.starts_with("2026-10-17 08:55:12.004Z WARN  - nrpt: Group Policy"));
        assert!(!file.contains(" INFO "));
        let failed = DemoLogs::live(Scenario::ConnectFail);
        assert!(
            failed
                .iter()
                .any(|l| l.source == "guard" && l.level == Level::Error)
        );
    }
}
