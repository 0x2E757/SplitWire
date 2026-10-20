//! The demo engine's state: what a service would hold, in IPC terms.

use std::net::{Ipv4Addr, SocketAddrV4};
use std::time::{Instant, SystemTime};

use crate::config::{ProfileSettings, SettingProblem, Settings};
use crate::ipc::{Alert, Attempt, LogLine, ProfileState, RouteRow};
use crate::profile::{Endpoint, Pin, TunnelAddress};

/// A route row and the list line it belongs to (`None`: the Upstream DNS or Refused group).
#[derive(Clone, Debug)]
pub struct DemoRoute {
    pub line: Option<usize>,
    pub row: RouteRow,
}

/// The demo traffic of a connected profile: a random walk around `level` KB/s.
#[derive(Clone, Copy, Debug)]
pub struct DemoChart {
    pub level: f64,
    pub seed: u64,
}

#[derive(Clone, Debug)]
pub struct DemoProfile {
    pub name: String,
    pub colour: Pin,
    pub state: ProfileState,
    pub since: Option<SystemTime>,
    pub uptime_secs: Option<u64>,
    pub last_connected: Option<SystemTime>,
    pub handshake_age_secs: Option<u64>,
    pub rx_rate: u64,
    pub tx_rate: u64,
    pub rx_total: u64,
    pub tx_total: u64,
    pub dns_rtt_ms: Option<u64>,
    pub address: TunnelAddress,
    pub mtu: u16,
    pub endpoint: Endpoint,
    pub endpoint_addresses: Vec<Ipv4Addr>,
    pub interface_key: String,
    pub peer_key: String,
    pub dns: Vec<SocketAddrV4>,
    pub keepalive: u16,
    pub imported: SystemTime,
    /// `routes.txt`.
    pub list: String,
    pub routes: Vec<DemoRoute>,
    /// `profile.conf`.
    pub settings: String,
    pub attempt: Option<Attempt>,
    pub chart: DemoChart,
    /// A Connect in progress; it completes after a moment.
    pub connecting_since: Option<Instant>,
    /// The first traffic sample of this connection.
    pub traffic_from: u64,
}

impl DemoProfile {
    pub fn adapter(&self) -> String {
        format!("SplitWire-{}", self.name)
    }

    pub fn settings_problems(&self) -> Vec<SettingProblem> {
        ProfileSettings::parse(&self.name, &self.settings)
            .err()
            .unwrap_or_default()
    }

    pub fn parsed_settings(&self) -> ProfileSettings {
        ProfileSettings::parse(&self.name, &self.settings).unwrap_or_default()
    }

    /// The DNS servers of the tunnel: `profile.conf`'s `upstream`, else the `.conf`'s `DNS=`.
    pub fn upstream(&self) -> Vec<SocketAddrV4> {
        let settings = self.parsed_settings();
        if settings.upstream.is_empty() {
            self.dns.clone()
        } else {
            settings.upstream
        }
    }

    pub fn connected(&self) -> bool {
        matches!(self.state, ProfileState::Connected(_))
    }

    pub fn learned(&self) -> u64 {
        self.routes
            .iter()
            .filter(|r| matches!(r.row, RouteRow::Learned { .. }))
            .count() as u64
    }

    /// Goes off; `at` becomes the last connection.
    pub fn disconnect(&mut self, at: SystemTime) {
        if self.connected() {
            self.last_connected = Some(at);
        }
        self.state = ProfileState::Off;
        self.since = None;
        self.uptime_secs = None;
        self.handshake_age_secs = None;
        self.rx_rate = 0;
        self.tx_rate = 0;
        self.dns_rtt_ms = None;
        self.connecting_since = None;
        self.routes.clear();
    }
}

/// Everything the demo service holds.
#[derive(Clone, Debug)]
pub struct DemoModel {
    pub service: bool,
    pub since: SystemTime,
    pub queries: u64,
    pub listed: u64,
    pub cache_entries: u64,
    pub nrpt_rules: u64,
    pub nrpt_note: Option<String>,
    pub fallback: Vec<Ipv4Addr>,
    /// `splitwire.conf`.
    pub global: String,
    pub profiles: Vec<DemoProfile>,
    pub alerts: Vec<Alert>,
    pub history: Vec<Alert>,
    pub logs: Vec<LogLine>,
    /// `logs\splitwire.log`.
    pub log_file: String,
}

impl DemoModel {
    pub fn global_problems(&self) -> Vec<SettingProblem> {
        Settings::parse(&self.global, false)
            .err()
            .unwrap_or_default()
    }

    pub fn profile(&self, name: &str) -> Option<&DemoProfile> {
        self.profiles
            .iter()
            .find(|p| p.name.eq_ignore_ascii_case(name))
    }

    pub fn profile_mut(&mut self, name: &str) -> Option<&mut DemoProfile> {
        self.profiles
            .iter_mut()
            .find(|p| p.name.eq_ignore_ascii_case(name))
    }

    /// The profiles' list files, `(name, text)`, in order.
    pub fn lists(&self) -> Vec<(&str, &str)> {
        self.profiles
            .iter()
            .map(|p| (p.name.as_str(), p.list.as_str()))
            .collect()
    }
}
