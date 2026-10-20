use std::fmt;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    Info,
    Warning,
    Error,
}

impl Severity {
    pub fn id(self) -> &'static str {
        match self {
            Severity::Info => "info",
            Severity::Warning => "warning",
            Severity::Error => "error",
        }
    }
}

/// The conditions that raise an alert, with their fixed severity and scope.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum AlertKind {
    SettingsInvalid,
    ProfileSettingsInvalid,
    ConnectFailed,
    RecoverFailed,
    UpstreamUnreachable,
    NrptWriteFailed,
    HandshakeStale,
    AddressHeld,
    ListProblem,
    GpNrpt,
    RouteRefused,
}

impl AlertKind {
    pub fn id(self) -> &'static str {
        match self {
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

    pub fn severity(self) -> Severity {
        match self {
            AlertKind::SettingsInvalid
            | AlertKind::ProfileSettingsInvalid
            | AlertKind::ConnectFailed
            | AlertKind::RecoverFailed
            | AlertKind::UpstreamUnreachable
            | AlertKind::NrptWriteFailed => Severity::Error,
            AlertKind::HandshakeStale
            | AlertKind::AddressHeld
            | AlertKind::ListProblem
            | AlertKind::GpNrpt => Severity::Warning,
            AlertKind::RouteRefused => Severity::Info,
        }
    }

    /// A blocking alert keeps a profile (or every profile) off; it cannot be dismissed.
    pub fn blocking(self) -> bool {
        matches!(
            self,
            AlertKind::SettingsInvalid | AlertKind::ProfileSettingsInvalid
        )
    }

    /// A global alert belongs to no profile.
    pub fn global(self) -> bool {
        matches!(
            self,
            AlertKind::SettingsInvalid | AlertKind::NrptWriteFailed | AlertKind::GpNrpt
        )
    }
}

impl fmt::Display for AlertKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.id())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn follows_the_table_of_spec_9() {
        assert_eq!(AlertKind::AddressHeld.id(), "address_held");
        assert_eq!(AlertKind::RouteRefused.severity(), Severity::Info);
        assert_eq!(AlertKind::GpNrpt.severity(), Severity::Warning);
        assert_eq!(AlertKind::NrptWriteFailed.severity(), Severity::Error);
        assert!(AlertKind::SettingsInvalid.blocking() && AlertKind::SettingsInvalid.global());
        assert!(AlertKind::ProfileSettingsInvalid.blocking());
        assert!(!AlertKind::ProfileSettingsInvalid.global());
        assert!(!AlertKind::ConnectFailed.blocking());
    }
}
