//! Alerts as the window shows them: one row rendering for the bell flyout, Status
//! and the profile page; the bell's badge in the worst severity; the flyout's dismissed and cleared
//! lines.

use super::LocalTime;
use crate::ipc::{Alert, Severity};

/// One alert row.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AlertView {
    pub id: String,
    pub severity: Severity,
    pub title: String,
    pub message: String,
    /// `AcmeCorp · 11:21`, or `Global · 09:14`.
    pub meta: String,
    pub action: Option<String>,
    /// Blocking alerts cannot be dismissed.
    pub dismissible: bool,
}

impl AlertView {
    pub fn of(alert: &Alert, time: &LocalTime) -> Self {
        Self {
            id: alert.id.clone(),
            severity: alert.severity,
            title: alert.title.clone(),
            message: alert.message.clone(),
            meta: format!(
                "{} · {}",
                alert.profile.as_deref().unwrap_or("Global"),
                time.clock(alert.since)
            ),
            action: alert.action.as_ref().map(|a| a.label.clone()),
            dismissible: !alert.blocking,
        }
    }
}

/// The bell flyout's content.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BellFlyout {
    /// `1 error · 2 warnings · 1 info`.
    pub counts: String,
    pub active: Vec<AlertView>,
    pub can_dismiss_all: bool,
    /// `1 dismissed: Stale handshake`.
    pub dismissed: Option<String>,
    /// Titles of the alerts cleared today.
    pub cleared: Vec<String>,
    /// No service: alerts come from it.
    pub service_down: bool,
}

/// The alerts of one `Status`, in severity order.
pub struct AlertList<'a> {
    alerts: &'a [Alert],
}

impl<'a> AlertList<'a> {
    pub fn new(alerts: &'a [Alert]) -> Self {
        Self { alerts }
    }

    /// Active (not dismissed) alerts, errors first; `profile` filters by scope when given
    /// (`Some(None)`: global ones only).
    pub fn active(&self, profile: Option<Option<&str>>) -> Vec<&'a Alert> {
        let mut active: Vec<&Alert> = self
            .alerts
            .iter()
            .filter(|a| !a.dismissed && a.cleared.is_none())
            .filter(|a| profile.is_none_or(|p| a.profile.as_deref() == p))
            .collect();
        active.sort_by_key(|a| a.severity);
        active
    }

    pub fn views(&self, profile: Option<Option<&str>>, time: &LocalTime) -> Vec<AlertView> {
        self.active(profile)
            .into_iter()
            .map(|a| AlertView::of(a, time))
            .collect()
    }

    /// The bell's badge: the count of active alerts and the worst severity.
    pub fn badge(&self) -> Option<(usize, Severity)> {
        let active = self.active(None);
        let worst = active.iter().map(|a| a.severity).min()?;
        Some((active.len(), worst))
    }

    pub fn flyout(&self, history: &[Alert], time: &LocalTime, service_down: bool) -> BellFlyout {
        let active = self.active(None);
        let counts = [
            (Severity::Error, "error", "errors"),
            (Severity::Warning, "warning", "warnings"),
            (Severity::Info, "info", "info"),
        ]
        .into_iter()
        .filter_map(|(severity, one, many)| {
            let n = active.iter().filter(|a| a.severity == severity).count();
            (n > 0).then(|| format!("{n} {}", if n == 1 { one } else { many }))
        })
        .collect::<Vec<_>>()
        .join(" · ");
        let dismissed: Vec<&str> = self
            .alerts
            .iter()
            .filter(|a| a.dismissed)
            .map(|a| a.title.as_str())
            .collect();
        BellFlyout {
            counts,
            can_dismiss_all: active.iter().any(|a| !a.blocking),
            active: active.iter().map(|a| AlertView::of(a, time)).collect(),
            dismissed: (!dismissed.is_empty())
                .then(|| format!("{} dismissed: {}", dismissed.len(), dismissed.join(", "))),
            cleared: history
                .iter()
                .filter(|a| a.cleared.is_some())
                .map(|a| format!("{} · {}", a.title, time.clock(a.cleared.unwrap_or(a.since))))
                .collect(),
            service_down,
        }
    }

    pub fn find(&self, id: &str) -> Option<&'a Alert> {
        self.alerts.iter().find(|a| a.id == id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::presentation::samples::Samples;

    #[test]
    fn active_alerts_sort_errors_first_and_skip_dismissed() {
        let status = Samples::status("normal");
        let list = AlertList::new(&status.alerts);
        let titles: Vec<&str> = list.active(None).iter().map(|a| a.title.as_str()).collect();
        assert_eq!(titles[0], "HomeLab cannot connect");
        assert_eq!(titles.len(), 4);
        assert!(!titles.contains(&"Stale handshake"));
        assert_eq!(list.badge(), Some((4, Severity::Error)));
        let global = list.active(Some(None));
        assert_eq!(global.len(), 1);
        assert_eq!(list.active(Some(Some("AcmeCorp"))).len(), 1);
    }

    #[test]
    fn the_flyout_counts_by_severity_and_lists_the_dismissed() {
        let status = Samples::status("normal");
        let flyout = AlertList::new(&status.alerts).flyout(&[], &Samples::time(), false);
        assert_eq!(flyout.counts, "1 error · 2 warnings · 1 info");
        assert_eq!(flyout.dismissed.unwrap(), "1 dismissed: Stale handshake");
        assert!(flyout.can_dismiss_all);
        assert_eq!(flyout.active[0].meta, "HomeLab · 09:12");
    }

    #[test]
    fn blocking_alerts_cannot_be_dismissed() {
        let status = Samples::status("settingsinvalid");
        let list = AlertList::new(&status.alerts);
        let views = list.views(Some(None), &Samples::time());
        assert_eq!(views[0].title, "splitwire.conf is invalid");
        assert!(!views[0].dismissible);
    }
}
