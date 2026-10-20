//! The demo's sample alerts, as the service would raise them.

use std::time::SystemTime;

use super::Scenario;
use crate::config::SettingsScope;
use crate::ipc::{ActionTarget, Alert, AlertAction, AlertKind, SettingField, Severity, Suggestion};

pub struct DemoAlerts;

impl DemoAlerts {
    #[allow(clippy::too_many_arguments)]
    fn alert(
        kind: AlertKind,
        severity: Severity,
        profile: Option<&str>,
        subject: &str,
        since: SystemTime,
        title: &str,
        message: &str,
        action: (&str, ActionTarget),
    ) -> Alert {
        Alert {
            id: format!(
                "{}:{}:{subject}",
                Self::kind_key(kind),
                profile.unwrap_or_default()
            ),
            kind,
            severity,
            profile: profile.map(str::to_owned),
            since,
            cleared: None,
            title: title.into(),
            message: message.into(),
            action: Some(AlertAction {
                label: action.0.into(),
                target: action.1,
            }),
            suggest: None,
            blocking: false,
            dismissed: false,
        }
    }

    fn kind_key(kind: AlertKind) -> &'static str {
        match kind {
            AlertKind::SettingsInvalid => "settings_invalid",
            AlertKind::ProfileSettingsInvalid => "profile_settings_invalid",
            AlertKind::ConnectFailed => "connect_failed",
            AlertKind::RecoverFailed => "recover_failed",
            AlertKind::UpstreamUnreachable => "upstream_unreachable",
            AlertKind::NrptWriteFailed => "nrpt_write_failed",
            AlertKind::HandshakeStale => "handshake_stale",
            AlertKind::AddressHeld => "address_held",
            AlertKind::ListProblem => "list_problem",
            AlertKind::GpNrpt => "gp_nrpt",
            AlertKind::RouteRefused => "route_refused",
        }
    }

    fn profile_target(name: &str) -> ActionTarget {
        ActionTarget::Profile {
            name: name.into(),
            field: None,
        }
    }

    pub fn home_lab_blocked(message: &str) -> Alert {
        let mut alert = Self::alert(
            AlertKind::ProfileSettingsInvalid,
            Severity::Error,
            Some("HomeLab"),
            "mtu",
            Scenario::at(9, 12, 3),
            "HomeLab cannot connect",
            message,
            (
                "Open settings",
                ActionTarget::Profile {
                    name: "HomeLab".into(),
                    field: Some("mtu".into()),
                },
            ),
        );
        alert.blocking = true;
        alert.suggest = Some(Suggestion {
            field: SettingField {
                scope: SettingsScope::Profile("HomeLab".into()),
                key: "mtu".into(),
            },
            value: "1420".into(),
        });
        alert
    }

    pub fn normal() -> Vec<Alert> {
        let mut stale = Self::alert(
            AlertKind::HandshakeStale,
            Severity::Warning,
            Some("AcmeCorp"),
            "",
            Scenario::at(11, 2, 10),
            "Stale handshake",
            "No handshake for 3 min while traffic was sent; the endpoint was re-resolved.",
            ("Open profile", Self::profile_target("AcmeCorp")),
        );
        stale.dismissed = true;
        vec![
            Self::home_lab_blocked(
                "profile.conf line 2: mtu = 9000 must be 1280–1500. Only HomeLab is blocked.",
            ),
            Self::alert(
                AlertKind::GpNrpt,
                Severity::Warning,
                None,
                "",
                Scenario::at(9, 14, 11),
                "NRPT rules from Group Policy override SplitWire",
                "Listed names are not routed while a policy defines its own rules.",
                ("Show in log", ActionTarget::Logs { profile: None }),
            ),
            Self::alert(
                AlertKind::AddressHeld,
                Severity::Warning,
                Some("AcmeCorp"),
                "192.0.2.93",
                Scenario::at(11, 21, 44),
                "192.0.2.93 is held by Contoso",
                "Learned by Contoso (contoso.example) and also answered for AcmeCorp \
                 (cdn.acme.example): Contoso holds it, so that traffic goes through Contoso.",
                ("Open profile", Self::profile_target("AcmeCorp")),
            ),
            Self::alert(
                AlertKind::RouteRefused,
                Severity::Info,
                Some("Contoso"),
                "192.168.1.20",
                Scenario::at(9, 21, 47),
                "Route refused: 192.168.1.20",
                "printer.contoso.example answered with an address on the on-link network of \
                 Ethernet (192.168.1.0/24).",
                ("Open profile", Self::profile_target("Contoso")),
            ),
            stale,
        ]
    }

    pub fn connect_failed() -> Alert {
        Self::alert(
            AlertKind::ConnectFailed,
            Severity::Error,
            Some("HomeLab"),
            "",
            Scenario::at(11, 27, 41),
            "Connect HomeLab failed at step 3 (guard)",
            "home.example.net did not resolve through the fallback DNS 192.168.1.1. Undone: \
             profile. Nothing else changed.",
            ("Open profile", Self::profile_target("HomeLab")),
        )
    }

    pub fn settings_invalid() -> Alert {
        let mut alert = Self::alert(
            AlertKind::SettingsInvalid,
            Severity::Error,
            None,
            "",
            Scenario::at(11, 25, 2),
            "splitwire.conf is invalid",
            "Line 4: max_ttl = 10 must be 30–86400. Every profile stays disconnected until it \
             is fixed.",
            (
                "Open settings",
                ActionTarget::Settings(Some(SettingField {
                    scope: SettingsScope::Global,
                    key: "max_ttl".into(),
                })),
            ),
        );
        alert.blocking = true;
        alert.suggest = Some(Suggestion {
            field: SettingField {
                scope: SettingsScope::Global,
                key: "max_ttl".into(),
            },
            value: "300".into(),
        });
        alert
    }

    /// Today's cleared alerts, for the bell's history.
    pub fn history() -> Vec<Alert> {
        let mut cleared = Self::alert(
            AlertKind::UpstreamUnreachable,
            Severity::Error,
            Some("Contoso"),
            "",
            Scenario::at(11, 26, 40),
            "Contoso’s DNS 10.66.0.53 did not answer",
            "3 queries failed in a row; the next answer cleared it.",
            (
                "Show in log",
                ActionTarget::Logs {
                    profile: Some("Contoso".into()),
                },
            ),
        );
        cleared.cleared = Some(Scenario::at(11, 26, 52));
        vec![cleared]
    }
}
