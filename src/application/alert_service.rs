use std::sync::{Arc, Mutex};
use std::time::SystemTime;

use crate::alerts::{
    Alert, AlertKind, AlertRaise, AlertStore, DismissRefused, Severity, TrayState,
};
use crate::clock::Clock;
use crate::logs::Log;
use crate::profile::ProfileState;

/// The engine's alerts: conditions raise and clear them, the user dismisses and restores them.
/// A new alert is also logged, so the log tells the same story.
pub struct AlertService {
    store: Mutex<AlertStore>,
    clock: Arc<dyn Clock>,
    log: Log,
}

impl AlertService {
    /// The conditions that hold only while a profile is connected; they end with it.
    const WHILE_CONNECTED: [AlertKind; 5] = [
        AlertKind::RecoverFailed,
        AlertKind::UpstreamUnreachable,
        AlertKind::HandshakeStale,
        AlertKind::AddressHeld,
        AlertKind::RouteRefused,
    ];

    pub fn new(clock: Arc<dyn Clock>, log: Log) -> Self {
        Self {
            store: Mutex::new(AlertStore::default()),
            clock,
            log: log.with_source("alerts"),
        }
    }

    pub fn raise(&self, raised: AlertRaise) {
        let id = raised.id();
        let line = format!("{}: {}", raised.title, raised.message);
        let profile = raised.profile.clone();
        let severity = raised.kind.severity();
        let new = {
            let mut store = self.store.lock().unwrap();
            let existed = store.get(&id).is_some();
            store.raise(raised, self.clock.now()) && !existed
        };
        if new {
            let log = match &profile {
                Some(name) => self.log.with_profile(name),
                None => self.log.clone(),
            };
            match severity {
                Severity::Error => log.error(line),
                Severity::Warning => log.warn(line),
                Severity::Info => log.info(line),
            }
        }
    }

    /// Clears the condition `kind` of `profile` (or the global one) with an empty subject.
    pub fn clear(&self, kind: AlertKind, profile: Option<&str>) {
        let id = Alert::id_of(kind, profile, "");
        self.store.lock().unwrap().clear(&id, self.clock.now());
    }

    /// Clears the alerts of `kind` (of `profile`, when given) whose subject `keep` refuses.
    pub fn clear_kind(&self, kind: AlertKind, profile: Option<&str>, keep: impl Fn(&str) -> bool) {
        self.store
            .lock()
            .unwrap()
            .clear_kind(kind, profile, keep, self.clock.now());
    }

    /// The profile disconnected: what held only while it was connected ends.
    pub fn clear_connected_conditions(&self, profile: &str) {
        for kind in Self::WHILE_CONNECTED {
            self.clear_kind(kind, Some(profile), |_| false);
        }
    }

    pub fn dismiss(&self, id: &str) -> Result<(), DismissRefused> {
        self.store.lock().unwrap().dismiss(id, self.clock.now())
    }

    pub fn dismiss_all(&self) -> usize {
        self.store.lock().unwrap().dismiss_all(self.clock.now())
    }

    pub fn restore(&self, id: Option<&str>) -> usize {
        self.store.lock().unwrap().restore(id)
    }

    pub fn active(&self) -> Vec<Alert> {
        self.store.lock().unwrap().active().to_vec()
    }

    pub fn history(&self, since: SystemTime) -> Vec<Alert> {
        self.store.lock().unwrap().history(since)
    }

    pub fn rev(&self) -> u64 {
        self.store.lock().unwrap().rev()
    }

    pub fn tray_state(&self, profiles: &[(String, ProfileState)]) -> TrayState {
        TrayState::of(self.store.lock().unwrap().active(), profiles)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::alerts::AlertTexts;
    use crate::clock::ManualClock;
    use std::net::Ipv4Addr;
    use std::time::UNIX_EPOCH;

    #[test]
    fn logs_new_alerts_and_ends_the_connected_conditions_together() {
        let clock = Arc::new(ManualClock::new(UNIX_EPOCH));
        let log = Log::new(1 << 16, clock.clone(), false);
        let alerts = AlertService::new(clock, log.clone());
        alerts.raise(AlertTexts::handshake_stale("AcmeCorp"));
        alerts.raise(AlertTexts::address_held(
            "AcmeCorp",
            Ipv4Addr::new(192, 0, 2, 93),
            "cdn.acme.example",
            "Contoso",
        ));
        alerts.raise(AlertTexts::gp_nrpt());
        assert!(log.contains("Stale handshake"));
        assert_eq!(alerts.active().len(), 3);
        alerts.clear_connected_conditions("AcmeCorp");
        assert_eq!(alerts.active().len(), 1);
        alerts.clear(AlertKind::GpNrpt, None);
        assert!(alerts.active().is_empty());
        assert_eq!(alerts.history(UNIX_EPOCH).len(), 3);
        assert_eq!(alerts.tray_state(&[]), TrayState::Off);
    }
}
