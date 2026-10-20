use std::collections::VecDeque;
use std::fmt;
use std::time::SystemTime;

use super::{Alert, AlertKind, AlertRaise};

/// Why a dismiss did nothing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DismissRefused {
    Unknown,
    /// A blocking alert keeps a profile off until its file is fixed.
    Blocking,
}

impl fmt::Display for DismissRefused {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            DismissRefused::Unknown => "no such alert",
            DismissRefused::Blocking => "a blocking alert cannot be dismissed",
        })
    }
}

/// The alerts: raised and cleared by their conditions, dismissed and restored by the user.
/// A cleared alert moves to the history.
#[derive(Debug, Default)]
pub struct AlertStore {
    active: Vec<Alert>,
    history: VecDeque<Alert>,
    rev: u64,
}

impl AlertStore {
    pub const HISTORY: usize = 200;

    /// Raises the condition's alert, or updates it. A dismissed alert raised with another
    /// fingerprint shows again. True when anything changed.
    pub fn raise(&mut self, raised: AlertRaise, now: SystemTime) -> bool {
        let id = raised.id();
        if let Some(alert) = self.active.iter_mut().find(|a| a.id == id) {
            if alert.raised == raised {
                return false;
            }
            if alert.raised.fingerprint != raised.fingerprint {
                alert.dismissed = None;
            }
            alert.raised = raised;
        } else {
            self.active.push(Alert {
                id,
                raised,
                since: now,
                cleared: None,
                dismissed: None,
            });
        }
        self.rev += 1;
        true
    }

    /// The condition ended: the alert moves to the history.
    pub fn clear(&mut self, id: &str, now: SystemTime) -> bool {
        self.clear_where(|alert| alert.id == id, now) > 0
    }

    /// Clears the alerts of `kind` (of `profile`, when given) whose subject `keep` refuses.
    pub fn clear_kind(
        &mut self,
        kind: AlertKind,
        profile: Option<&str>,
        keep: impl Fn(&str) -> bool,
        now: SystemTime,
    ) -> usize {
        self.clear_where(
            |alert| {
                alert.kind() == kind
                    && (profile.is_none() || alert.profile() == profile)
                    && !keep(&alert.raised.subject)
            },
            now,
        )
    }

    pub fn dismiss(&mut self, id: &str, now: SystemTime) -> Result<(), DismissRefused> {
        let alert = self
            .active
            .iter_mut()
            .find(|a| a.id == id)
            .ok_or(DismissRefused::Unknown)?;
        if alert.blocking() {
            return Err(DismissRefused::Blocking);
        }
        if alert.dismissed.is_none() {
            alert.dismissed = Some(now);
            self.rev += 1;
        }
        Ok(())
    }

    /// Dismisses every alert that may be; returns how many.
    pub fn dismiss_all(&mut self, now: SystemTime) -> usize {
        let mut count = 0;
        for alert in self
            .active
            .iter_mut()
            .filter(|a| !a.blocking() && a.dismissed.is_none())
        {
            alert.dismissed = Some(now);
            count += 1;
        }
        self.bump(count)
    }

    /// Shows one dismissed alert again, or all of them.
    pub fn restore(&mut self, id: Option<&str>) -> usize {
        let mut count = 0;
        for alert in self
            .active
            .iter_mut()
            .filter(|a| id.is_none_or(|id| a.id == id) && a.dismissed.is_some())
        {
            alert.dismissed = None;
            count += 1;
        }
        self.bump(count)
    }

    /// The active alerts, dismissed ones included, oldest first.
    pub fn active(&self) -> &[Alert] {
        &self.active
    }

    pub fn get(&self, id: &str) -> Option<&Alert> {
        self.active.iter().find(|a| a.id == id)
    }

    /// The alerts cleared or dismissed since `since` (today's local midnight), newest first, at
    /// most 200.
    pub fn history(&self, since: SystemTime) -> Vec<Alert> {
        let mut found: Vec<(SystemTime, &Alert)> = self
            .history
            .iter()
            .filter_map(|a| a.cleared.map(|at| (at, a)))
            .chain(
                self.active
                    .iter()
                    .filter_map(|a| a.dismissed.map(|at| (at, a))),
            )
            .filter(|(at, _)| *at >= since)
            .collect();
        found.sort_by_key(|(at, _)| std::cmp::Reverse(*at));
        found
            .into_iter()
            .take(Self::HISTORY)
            .map(|(_, a)| a.clone())
            .collect()
    }

    /// Changes whenever the alerts change (`revs.alerts`).
    pub fn rev(&self) -> u64 {
        self.rev
    }

    fn clear_where(&mut self, ends: impl Fn(&Alert) -> bool, now: SystemTime) -> usize {
        let (ended, kept): (Vec<Alert>, Vec<Alert>) =
            std::mem::take(&mut self.active).into_iter().partition(ends);
        self.active = kept;
        let count = ended.len();
        for mut alert in ended {
            alert.cleared = Some(now);
            if self.history.len() == Self::HISTORY {
                self.history.pop_front();
            }
            self.history.push_back(alert);
        }
        self.bump(count)
    }

    fn bump(&mut self, count: usize) -> usize {
        if count > 0 {
            self.rev += 1;
        }
        count
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, UNIX_EPOCH};

    fn at(secs: u64) -> SystemTime {
        UNIX_EPOCH + Duration::from_secs(secs)
    }

    fn raise(
        kind: AlertKind,
        profile: Option<&str>,
        subject: &str,
        fingerprint: &str,
    ) -> AlertRaise {
        AlertRaise {
            kind,
            profile: profile.map(Into::into),
            subject: subject.into(),
            title: format!("{kind} {subject}"),
            message: fingerprint.into(),
            action: None,
            suggest: None,
            fingerprint: fingerprint.into(),
        }
    }

    #[test]
    fn raise_keeps_one_alert_per_condition_and_clear_moves_it_to_history() {
        let mut store = AlertStore::default();
        let held = raise(
            AlertKind::AddressHeld,
            Some("AcmeCorp"),
            "192.0.2.93",
            "Contoso",
        );
        assert!(store.raise(held.clone(), at(10)));
        assert!(!store.raise(held, at(20)));
        assert_eq!(store.active().len(), 1);
        assert_eq!(store.active()[0].id, "address_held:AcmeCorp:192.0.2.93");
        assert_eq!(store.active()[0].since, at(10));
        let rev = store.rev();
        assert!(store.clear("address_held:AcmeCorp:192.0.2.93", at(30)));
        assert!(store.active().is_empty());
        assert!(store.rev() > rev);
        let history = store.history(at(0));
        assert_eq!(history[0].cleared, Some(at(30)));
        assert!(store.history(at(31)).is_empty());
    }

    #[test]
    fn a_dismissed_alert_shows_again_only_when_its_condition_changes() {
        let mut store = AlertStore::default();
        store.raise(
            raise(AlertKind::HandshakeStale, Some("A"), "", "stale"),
            at(1),
        );
        store.dismiss("handshake_stale:A:", at(2)).unwrap();
        assert!(store.active()[0].is_dismissed());
        store.raise(
            raise(AlertKind::HandshakeStale, Some("A"), "", "stale"),
            at(3),
        );
        assert!(store.active()[0].is_dismissed());
        assert_eq!(store.history(at(0)).len(), 1);
        store.raise(
            raise(AlertKind::HandshakeStale, Some("A"), "", "stale again"),
            at(4),
        );
        assert!(!store.active()[0].is_dismissed());
    }

    #[test]
    fn blocking_alerts_cannot_be_dismissed() {
        let mut store = AlertStore::default();
        store.raise(raise(AlertKind::SettingsInvalid, None, "", "line 4"), at(1));
        store.raise(raise(AlertKind::GpNrpt, None, "", "gp"), at(1));
        assert_eq!(
            store.dismiss("settings_invalid::", at(2)),
            Err(DismissRefused::Blocking)
        );
        assert_eq!(store.dismiss("nope", at(2)), Err(DismissRefused::Unknown));
        assert_eq!(store.dismiss_all(at(2)), 1);
        assert_eq!(store.restore(None), 1);
        store.dismiss("gp_nrpt::", at(3)).unwrap();
        assert_eq!(store.restore(Some("gp_nrpt::")), 1);
        assert!(store.active().iter().all(|a| !a.is_dismissed()));
    }

    #[test]
    fn clear_kind_keeps_the_subjects_still_true() {
        let mut store = AlertStore::default();
        for subject in ["1.1.1.1", "2.2.2.2"] {
            store.raise(
                raise(AlertKind::RouteRefused, Some("A"), subject, ""),
                at(1),
            );
        }
        store.raise(
            raise(AlertKind::RouteRefused, Some("B"), "3.3.3.3", ""),
            at(1),
        );
        let cleared = store.clear_kind(
            AlertKind::RouteRefused,
            Some("A"),
            |s| s == "2.2.2.2",
            at(2),
        );
        assert_eq!(cleared, 1);
        let left: Vec<&str> = store
            .active()
            .iter()
            .map(|a| a.raised.subject.as_str())
            .collect();
        assert_eq!(left, ["2.2.2.2", "3.3.3.3"]);
        assert_eq!(
            store.clear_kind(AlertKind::RouteRefused, None, |_| false, at(3)),
            2
        );
    }

    #[test]
    fn the_history_is_bounded() {
        let mut store = AlertStore::default();
        for n in 0..250u64 {
            store.raise(
                raise(AlertKind::RouteRefused, Some("A"), &n.to_string(), ""),
                at(n),
            );
            store.clear(&format!("route_refused:A:{n}"), at(n));
        }
        let history = store.history(at(0));
        assert_eq!(history.len(), AlertStore::HISTORY);
        assert_eq!(history[0].raised.subject, "249");
    }
}
