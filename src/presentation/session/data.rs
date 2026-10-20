//! What the engine last answered, kept for the views.

use std::collections::VecDeque;
use std::time::SystemTime;

use crate::ipc::{
    Alert, ListsView, LogLine, ProfileDetail, ProfileRoutes, ProfileTraffic, SettingsView,
    StatusReport, TrafficSample,
};
use crate::presentation::TrafficChart;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ServiceState {
    /// No answer yet.
    Waiting,
    Running,
    /// The pipe is absent.
    Down,
}

/// The last five minutes of every connected profile's traffic.
#[derive(Default)]
pub struct TrafficStore {
    profiles: Vec<(String, VecDeque<TrafficSample>)>,
    last_seq: u64,
}

impl TrafficStore {
    pub fn last_seq(&self) -> u64 {
        self.last_seq
    }

    pub fn add(&mut self, traffic: Vec<ProfileTraffic>) {
        for profile in traffic {
            if let Some(last) = profile.samples.last() {
                self.last_seq = self.last_seq.max(last.seq);
            }
            let entry = match self.profiles.iter_mut().find(|(n, _)| *n == profile.name) {
                Some((_, samples)) => samples,
                None => {
                    self.profiles.push((profile.name.clone(), VecDeque::new()));
                    &mut self.profiles.last_mut().expect("just pushed").1
                }
            };
            entry.extend(profile.samples);
            while entry.len() > TrafficChart::WINDOW {
                entry.pop_front();
            }
        }
    }

    /// Forgets the profiles that are no longer connected.
    pub fn retain(&mut self, connected: &[&str]) {
        self.profiles
            .retain(|(n, _)| connected.contains(&n.as_str()));
    }

    pub fn samples(&self, name: &str) -> Vec<TrafficSample> {
        self.profiles
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, s)| s.iter().copied().collect())
            .unwrap_or_default()
    }
}

/// The ring buffer's lines the window has seen, oldest first.
#[derive(Default)]
pub struct LogStore {
    pub lines: Vec<LogLine>,
    pub last_seq: u64,
}

impl LogStore {
    /// The window keeps this many lines of the service's ring buffer.
    pub const KEEP: usize = 5000;

    pub fn add(&mut self, entries: Vec<LogLine>, last_seq: u64) {
        self.lines.extend(entries);
        if self.lines.len() > Self::KEEP {
            self.lines.drain(..self.lines.len() - Self::KEEP);
        }
        self.last_seq = self.last_seq.max(last_seq);
    }
}

#[derive(Default)]
pub struct EngineData {
    pub status: Option<StatusReport>,
    pub detail: Option<ProfileDetail>,
    pub routes: Option<Vec<ProfileRoutes>>,
    pub traffic: TrafficStore,
    pub lists: Option<ListsView>,
    pub settings: Option<SettingsView>,
    pub logs: LogStore,
    pub history: Vec<Alert>,
    /// `logs\splitwire.log` as parsed lines, and its size in bytes.
    pub log_file: Option<(Vec<LogLine>, usize)>,
    pub last_answer: Option<SystemTime>,
}

impl EngineData {
    /// The service went away: nothing it said still holds.
    pub fn forget(&mut self) {
        let last_answer = self.last_answer;
        *self = Self::default();
        self.last_answer = last_answer;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn traffic_keeps_five_minutes_per_profile() {
        let mut store = TrafficStore::default();
        let samples = |from: u64, to: u64| -> Vec<TrafficSample> {
            (from..=to)
                .map(|seq| TrafficSample {
                    seq,
                    rx_bps: seq,
                    tx_bps: 0,
                })
                .collect()
        };
        store.add(vec![ProfileTraffic {
            name: "A".into(),
            samples: samples(1, 250),
        }]);
        store.add(vec![ProfileTraffic {
            name: "A".into(),
            samples: samples(251, 400),
        }]);
        assert_eq!(store.last_seq(), 400);
        let kept = store.samples("A");
        assert_eq!(kept.len(), 300);
        assert_eq!(kept[0].seq, 101);
        store.retain(&[]);
        assert!(store.samples("A").is_empty());
    }
}
