use super::{Alert, AlertKind, Severity};
use crate::profile::ProfileState;

/// The tray icon's state, computed by the service so the window, tray and CLI agree.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TrayState {
    /// The pipe is absent (computed by the client).
    Service,
    Error,
    Warning,
    Connected,
    Off,
}

impl TrayState {
    pub fn id(self) -> &'static str {
        match self {
            TrayState::Service => "service",
            TrayState::Error => "error",
            TrayState::Warning => "warning",
            TrayState::Connected => "connected",
            TrayState::Off => "off",
        }
    }

    /// The first rule that matches, over the alerts not dismissed.
    pub fn of(alerts: &[Alert], profiles: &[(String, ProfileState)]) -> TrayState {
        let connected = |name: Option<&str>| {
            profiles
                .iter()
                .any(|(n, state)| state.is_connected() && Some(n.as_str()) == name)
        };
        let shown: Vec<&Alert> = alerts.iter().filter(|a| !a.is_dismissed()).collect();
        let error = shown.iter().any(|a| {
            a.severity() == Severity::Error
                && (connected(a.profile())
                    || matches!(
                        a.kind(),
                        AlertKind::ConnectFailed
                            | AlertKind::RecoverFailed
                            | AlertKind::SettingsInvalid
                    ))
        });
        if error {
            return TrayState::Error;
        }
        let warning = shown.iter().any(|a| a.severity() >= Severity::Warning)
            || profiles
                .iter()
                .any(|(_, s)| matches!(s, ProfileState::Connecting { .. }));
        if warning {
            TrayState::Warning
        } else if profiles.iter().any(|(_, s)| s.is_connected()) {
            TrayState::Connected
        } else {
            TrayState::Off
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::alerts::AlertRaise;
    use crate::tunnel::{ConnectStep, HandshakeHealth};
    use std::time::UNIX_EPOCH;

    fn alert(kind: AlertKind, profile: Option<&str>, dismissed: bool) -> Alert {
        let raised = AlertRaise {
            kind,
            profile: profile.map(Into::into),
            subject: String::new(),
            title: String::new(),
            message: String::new(),
            action: None,
            suggest: None,
            fingerprint: String::new(),
        };
        Alert {
            id: raised.id(),
            raised,
            since: UNIX_EPOCH,
            cleared: None,
            dismissed: dismissed.then_some(UNIX_EPOCH),
        }
    }

    fn profiles(states: &[(&str, ProfileState)]) -> Vec<(String, ProfileState)> {
        states.iter().map(|(n, s)| ((*n).to_owned(), *s)).collect()
    }

    const UP: ProfileState = ProfileState::Connected {
        health: HandshakeHealth::Ok,
    };

    #[test]
    fn follows_the_table_first_match_wins() {
        let both = profiles(&[("A", UP), ("B", ProfileState::Off)]);
        let error_of_connected = [alert(AlertKind::UpstreamUnreachable, Some("A"), false)];
        assert_eq!(TrayState::of(&error_of_connected, &both), TrayState::Error);
        let blocked_off = [alert(AlertKind::ProfileSettingsInvalid, Some("B"), false)];
        assert_eq!(TrayState::of(&blocked_off, &both), TrayState::Warning);
        let failed = [alert(AlertKind::ConnectFailed, Some("B"), false)];
        assert_eq!(TrayState::of(&failed, &both), TrayState::Error);
        let global = [alert(AlertKind::SettingsInvalid, None, false)];
        assert_eq!(TrayState::of(&global, &profiles(&[])), TrayState::Error);
        let info = [alert(AlertKind::RouteRefused, Some("A"), false)];
        assert_eq!(TrayState::of(&info, &both), TrayState::Connected);
        let dismissed = [alert(AlertKind::GpNrpt, None, true)];
        assert_eq!(TrayState::of(&dismissed, &both), TrayState::Connected);
        let gp = [alert(AlertKind::GpNrpt, None, false)];
        assert_eq!(TrayState::of(&gp, &both), TrayState::Warning);
        let connecting = profiles(&[(
            "A",
            ProfileState::Connecting {
                step: ConnectStep::Peer,
            },
        )]);
        assert_eq!(TrayState::of(&[], &connecting), TrayState::Warning);
        assert_eq!(
            TrayState::of(&[], &profiles(&[("A", ProfileState::Off)])),
            TrayState::Off
        );
        assert_eq!(TrayState::Service.id(), "service");
    }
}
