use std::net::{Ipv4Addr, SocketAddr};
use std::sync::Arc;
use std::time::SystemTime;

use super::wire_alerts::WireAlerts;
use super::wire_attempts::WireAttempts;
use super::{Coordinator, StoredProfile};
use crate::alerts::AlertKind;
use crate::ipc::{
    Counters, NrptStatus, ProfileSummary, ProfileTraffic, Revisions, StatusReport, TrafficSample,
};
use crate::profile::ProfileState;

/// What `Status` reports, read without the coordinator's lock so the window keeps
/// polling while a Connect runs: the revisions, the counters, NRPT, the alerts with the tray state,
/// and every profile's summary in the user's order. Also the traffic samples.
pub struct StatusReporter {
    coordinator: Arc<Coordinator>,
    started: SystemTime,
}

impl StatusReporter {
    pub fn new(coordinator: Arc<Coordinator>, started: SystemTime) -> Self {
        Self {
            coordinator,
            started,
        }
    }

    pub fn status(&self) -> StatusReport {
        let parts = self.coordinator.parts();
        let catalog = self.coordinator.catalog();
        let states: Vec<(String, ProfileState)> = catalog
            .iter()
            .map(|p| (p.name.to_string(), parts.states.state(p.name.as_str())))
            .collect();
        StatusReport {
            version: env!("CARGO_PKG_VERSION").into(),
            wgnt_version: parts.tunnel.driver_version(),
            since: self.started,
            tray_state: WireAlerts::tray(parts.alerts.tray_state(&states)),
            revs: self.revisions(),
            counters: self.counters(),
            nrpt: self.nrpt(),
            fallback: parts
                .guards
                .fallback_servers()
                .iter()
                .filter_map(|server| match server {
                    SocketAddr::V4(v4) => Some(*v4.ip()),
                    SocketAddr::V6(_) => None,
                })
                .collect::<Vec<Ipv4Addr>>(),
            problems: match parts.settings_files.global() {
                Ok(Err(problems)) => problems,
                _ => Vec::new(),
            },
            alerts: parts
                .alerts
                .active()
                .iter()
                .map(WireAlerts::alert)
                .collect(),
            profiles: catalog.iter().map(|p| self.summary(p)).collect(),
        }
    }

    pub fn revisions(&self) -> Revisions {
        Revisions {
            profiles: self.coordinator.profiles_rev(),
            lists: self.coordinator.lists_rev().unwrap_or_default(),
            settings: self.coordinator.settings_rev(),
            alerts: self.coordinator.parts().alerts.rev(),
        }
    }

    /// The summary of the stored profile `name`, if there is one.
    pub fn summary_of(&self, name: &str) -> Option<ProfileSummary> {
        self.coordinator
            .catalog()
            .iter()
            .find(|p| p.name.as_str().eq_ignore_ascii_case(name))
            .map(|p| self.summary(p))
    }

    pub fn summary(&self, stored: &StoredProfile) -> ProfileSummary {
        let parts = self.coordinator.parts();
        let name = stored.name.as_str();
        let now = parts.clock.now();
        let record = parts.states.get(name);
        let active = parts.active.get(name);
        let health = active.as_ref().map(|a| *a.health.lock().unwrap());
        let mut summary = ProfileSummary {
            name: name.to_owned(),
            colour: stored.colour,
            state: WireAttempts::state(record.state, health),
            since: None,
            uptime_secs: None,
            last_connected: record.last_connected,
            handshake_age_secs: None,
            rx_rate: 0,
            tx_rate: 0,
            rx_total: 0,
            tx_total: 0,
            learned: 0,
            dns_rtt_ms: None,
            upstream: None,
            keepalive: stored.facts.as_ref().map_or(0, |facts| facts.keepalive),
            last_attempt: record
                .last_attempt
                .as_ref()
                .map(|a| WireAttempts::attempt(a).summary()),
            problem: self.problem(stored, record.state),
        };
        let Some(active) = active else {
            return summary;
        };
        let secs_since = |at: SystemTime| now.duration_since(at).unwrap_or_default().as_secs();
        summary.since = Some(active.since);
        summary.uptime_secs = Some(secs_since(active.since));
        summary.handshake_age_secs = parts
            .tunnel
            .health(name)
            .ok()
            .and_then(|h| h.last_handshake)
            .map(secs_since);
        {
            let traffic = active.traffic.lock().unwrap();
            if let Some(latest) = traffic.latest() {
                (summary.rx_rate, summary.tx_rate) = (latest.rx_bps, latest.tx_bps);
            }
            (summary.rx_total, summary.tx_total) = traffic.totals();
        }
        summary.upstream = active.upstream.first().copied();
        summary.learned = active.routes.lock().unwrap().learned_ips().len() as u64;
        summary.dns_rtt_ms = parts
            .resolvers
            .get(name)
            .and_then(|r| r.health().rtt)
            .map(|rtt| rtt.as_millis() as u64);
        summary
    }

    /// Every connected profile's samples after `after_seq`, at most the 5-minute chart's 300.
    pub fn traffic(&self, after_seq: u64) -> Vec<ProfileTraffic> {
        let parts = self.coordinator.parts();
        self.coordinator
            .catalog()
            .iter()
            .filter_map(|stored| {
                let active = parts.active.get(stored.name.as_str())?;
                let samples = active.traffic.lock().unwrap().after(after_seq);
                let skip = samples.len().saturating_sub(ProfileTraffic::MAX_SAMPLES);
                Some(ProfileTraffic {
                    name: stored.name.to_string(),
                    samples: samples[skip..]
                        .iter()
                        .map(|s| TrafficSample {
                            seq: s.seq,
                            rx_bps: s.rx_bps,
                            tx_bps: s.tx_bps,
                        })
                        .collect(),
                })
            })
            .collect()
    }

    fn counters(&self) -> Counters {
        let parts = self.coordinator.parts();
        let mut counters = Counters {
            queries: parts.stats.snapshot(),
            ..Counters::default()
        };
        for active in parts.active.all() {
            counters.learned_routes += active.routes.lock().unwrap().learned_ips().len() as u64;
            if let Some(resolver) = parts.resolvers.get(&active.name) {
                counters.cache_entries += resolver.cache_len() as u64;
            }
        }
        counters
    }

    /// One rule per domain of the connected profiles; a note when Windows ignores them or the
    /// last write failed.
    fn nrpt(&self) -> NrptStatus {
        let parts = self.coordinator.parts();
        let active = parts.alerts.active();
        let has = |kind: AlertKind| active.iter().any(|a| a.kind() == kind);
        let note = if has(AlertKind::GpNrpt) {
            Some("Group Policy defines NRPT rules: Windows ignores SplitWire's".to_owned())
        } else if has(AlertKind::NrptWriteFailed) {
            Some("the last write of the rules failed".to_owned())
        } else {
            None
        };
        NrptStatus {
            rules: parts.matcher.current().domains().len() as u64,
            note,
        }
    }

    /// Why the profile cannot connect: its blob is unreadable, its `profile.conf` is invalid, or
    /// `splitwire.conf` is.
    fn problem(&self, stored: &StoredProfile, state: ProfileState) -> Option<String> {
        if let Err(problem) = &stored.facts {
            return Some(problem.clone());
        }
        let parts = self.coordinator.parts();
        match state {
            ProfileState::HeldOff => Some("splitwire.conf is invalid".into()),
            ProfileState::Blocked => parts
                .alerts
                .active()
                .into_iter()
                .find(|a| {
                    a.kind() == AlertKind::ProfileSettingsInvalid
                        && a.profile() == Some(stored.name.as_str())
                })
                .map(|a| a.raised.message),
            _ => None,
        }
    }
}
