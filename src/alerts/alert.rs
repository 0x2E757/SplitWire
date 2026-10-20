use std::time::SystemTime;

use super::{AlertKind, Severity};

/// Where the alert's button leads: `status`, `profile:<name>[#field]`, `settings[#scope.key]` or
/// `logs[:profile]`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AlertAction {
    pub label: String,
    pub target: String,
}

/// A one-click fix: the window fills the field and waits for Apply; nothing is written by it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Suggestion {
    pub scope: String,
    pub key: String,
    pub value: String,
}

/// What a condition reports when it raises its alert.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AlertRaise {
    pub kind: AlertKind,
    pub profile: Option<String>,
    /// What the condition is about (an address, a step); part of the id.
    pub subject: String,
    pub title: String,
    pub message: String,
    pub action: Option<AlertAction>,
    pub suggest: Option<Suggestion>,
    /// The condition's details: a dismissed alert raised with another fingerprint shows again.
    pub fingerprint: String,
}

impl AlertRaise {
    /// `kind:profile:subject`, stable for the condition.
    pub fn id(&self) -> String {
        Alert::id_of(self.kind, self.profile.as_deref(), &self.subject)
    }
}

/// One alert, active or in today's history.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Alert {
    pub id: String,
    pub raised: AlertRaise,
    pub since: SystemTime,
    /// When the condition ended; the alert then moved to the history.
    pub cleared: Option<SystemTime>,
    /// When the user dismissed it; hidden until the condition changes.
    pub dismissed: Option<SystemTime>,
}

impl Alert {
    pub fn id_of(kind: AlertKind, profile: Option<&str>, subject: &str) -> String {
        format!("{kind}:{}:{subject}", profile.unwrap_or(""))
    }

    pub fn kind(&self) -> AlertKind {
        self.raised.kind
    }

    pub fn profile(&self) -> Option<&str> {
        self.raised.profile.as_deref()
    }

    pub fn severity(&self) -> Severity {
        self.raised.kind.severity()
    }

    pub fn blocking(&self) -> bool {
        self.raised.kind.blocking()
    }

    pub fn is_dismissed(&self) -> bool {
        self.dismissed.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_are_kind_profile_and_subject() {
        assert_eq!(
            Alert::id_of(AlertKind::AddressHeld, Some("AcmeCorp"), "192.0.2.93"),
            "address_held:AcmeCorp:192.0.2.93"
        );
        assert_eq!(Alert::id_of(AlertKind::GpNrpt, None, ""), "gp_nrpt::");
    }
}
