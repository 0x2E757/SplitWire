use crate::alerts::{self, AlertKind, Severity, TrayState};
use crate::config::SettingsScope;
use crate::ipc;

/// Alerts as the pipe carries them: the engine's alert with its action target and
/// one-click fix parsed into the window's types. The fingerprint stays in the service.
pub struct WireAlerts;

impl WireAlerts {
    pub fn alert(alert: &alerts::Alert) -> ipc::Alert {
        let raised = &alert.raised;
        ipc::Alert {
            id: alert.id.clone(),
            kind: Self::kind(alert.kind()),
            severity: Self::severity(alert.severity()),
            profile: raised.profile.clone(),
            since: alert.since,
            cleared: alert.cleared,
            title: raised.title.clone(),
            message: raised.message.clone(),
            action: raised.action.as_ref().and_then(|action| {
                Some(ipc::AlertAction {
                    label: action.label.clone(),
                    target: Self::target(&action.target)?,
                })
            }),
            suggest: raised.suggest.as_ref().and_then(|suggest| {
                Some(ipc::Suggestion {
                    field: ipc::SettingField {
                        scope: SettingsScope::parse(&suggest.scope)?,
                        key: suggest.key.clone(),
                    },
                    value: suggest.value.clone(),
                })
            }),
            blocking: alert.blocking(),
            dismissed: alert.is_dismissed(),
        }
    }

    pub fn tray(state: TrayState) -> ipc::TrayState {
        match state {
            TrayState::Service => ipc::TrayState::Service,
            TrayState::Error => ipc::TrayState::Error,
            TrayState::Warning => ipc::TrayState::Warning,
            TrayState::Connected => ipc::TrayState::Connected,
            TrayState::Off => ipc::TrayState::Off,
        }
    }

    /// `status`, `profile:<name>[#field]`, `settings[#<scope>.<key>]` or `logs[:profile]`. A
    /// profile name may hold dots, a key never does.
    pub fn target(text: &str) -> Option<ipc::ActionTarget> {
        if text == "status" {
            return Some(ipc::ActionTarget::Status);
        }
        if text == "logs" {
            return Some(ipc::ActionTarget::Logs { profile: None });
        }
        if let Some(profile) = text.strip_prefix("logs:") {
            return Some(ipc::ActionTarget::Logs {
                profile: Some(profile.to_owned()),
            });
        }
        if text == "settings" {
            return Some(ipc::ActionTarget::Settings(None));
        }
        if let Some(field) = text.strip_prefix("settings#") {
            let (scope, key) = field.rsplit_once('.')?;
            return Some(ipc::ActionTarget::Settings(Some(ipc::SettingField {
                scope: SettingsScope::parse(scope)?,
                key: key.to_owned(),
            })));
        }
        let profile = text.strip_prefix("profile:")?;
        let (name, field) = match profile.split_once('#') {
            Some((name, field)) => (name, Some(field.to_owned())),
            None => (profile, None),
        };
        Some(ipc::ActionTarget::Profile {
            name: name.to_owned(),
            field,
        })
    }

    fn kind(kind: AlertKind) -> ipc::AlertKind {
        match kind {
            AlertKind::SettingsInvalid => ipc::AlertKind::SettingsInvalid,
            AlertKind::ProfileSettingsInvalid => ipc::AlertKind::ProfileSettingsInvalid,
            AlertKind::ConnectFailed => ipc::AlertKind::ConnectFailed,
            AlertKind::RecoverFailed => ipc::AlertKind::RecoverFailed,
            AlertKind::UpstreamUnreachable => ipc::AlertKind::UpstreamUnreachable,
            AlertKind::NrptWriteFailed => ipc::AlertKind::NrptWriteFailed,
            AlertKind::HandshakeStale => ipc::AlertKind::HandshakeStale,
            AlertKind::AddressHeld => ipc::AlertKind::AddressHeld,
            AlertKind::ListProblem => ipc::AlertKind::ListProblem,
            AlertKind::GpNrpt => ipc::AlertKind::GpNrpt,
            AlertKind::RouteRefused => ipc::AlertKind::RouteRefused,
        }
    }

    fn severity(severity: Severity) -> ipc::Severity {
        match severity {
            Severity::Error => ipc::Severity::Error,
            Severity::Warning => ipc::Severity::Warning,
            Severity::Info => ipc::Severity::Info,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::alerts::AlertTexts;
    use crate::config::SettingProblem;
    use std::time::UNIX_EPOCH;

    fn wire(raised: alerts::AlertRaise) -> ipc::Alert {
        WireAlerts::alert(&alerts::Alert {
            id: raised.id(),
            raised,
            since: UNIX_EPOCH,
            cleared: None,
            dismissed: Some(UNIX_EPOCH),
        })
    }

    #[test]
    fn parses_every_action_target() {
        use ipc::ActionTarget::*;
        let field = |scope, key: &str| {
            Some(ipc::SettingField {
                scope,
                key: key.into(),
            })
        };
        let cases = [
            ("status", Some(Status)),
            ("logs", Some(Logs { profile: None })),
            (
                "logs:My.VPN",
                Some(Logs {
                    profile: Some("My.VPN".into()),
                }),
            ),
            ("settings", Some(Settings(None))),
            (
                "settings#global.max_ttl",
                Some(Settings(field(SettingsScope::Global, "max_ttl"))),
            ),
            (
                "settings#profile:My.VPN.mtu",
                Some(Settings(field(
                    SettingsScope::Profile("My.VPN".into()),
                    "mtu",
                ))),
            ),
            (
                "profile:AcmeCorp#list",
                Some(Profile {
                    name: "AcmeCorp".into(),
                    field: Some("list".into()),
                }),
            ),
            (
                "profile:AcmeCorp",
                Some(Profile {
                    name: "AcmeCorp".into(),
                    field: None,
                }),
            ),
            ("elsewhere", None),
            ("settings#nonsense", None),
        ];
        for (text, expected) in cases {
            assert_eq!(WireAlerts::target(text), expected, "{text}");
        }
    }

    #[test]
    fn carries_the_kind_severity_action_and_fix() {
        let problem = SettingProblem {
            file: "profile.conf".into(),
            line: 3,
            key: "mtu".into(),
            value: "9000".into(),
            reason: "must be 1280-1500".into(),
            suggest: Some("1420".into()),
        };
        let alert = wire(AlertTexts::profile_settings_invalid("AcmeCorp", &[problem]));
        assert_eq!(alert.kind, ipc::AlertKind::ProfileSettingsInvalid);
        assert_eq!(alert.severity, ipc::Severity::Error);
        assert!(alert.blocking && alert.dismissed);
        assert_eq!(alert.id, "profile_settings_invalid:AcmeCorp:");
        assert_eq!(
            alert.action.unwrap().target,
            ipc::ActionTarget::Profile {
                name: "AcmeCorp".into(),
                field: Some("mtu".into())
            }
        );
        let suggest = alert.suggest.unwrap();
        assert_eq!(
            suggest.field.scope,
            SettingsScope::Profile("AcmeCorp".into())
        );
        assert_eq!(
            (suggest.field.key.as_str(), suggest.value.as_str()),
            ("mtu", "1420")
        );
        let info = wire(AlertTexts::route_refused(
            "A",
            "10.0.0.0/8",
            "static",
            "covers",
        ));
        assert_eq!(info.severity, ipc::Severity::Info);
        assert_eq!(
            WireAlerts::tray(TrayState::Warning),
            ipc::TrayState::Warning
        );
    }
}
