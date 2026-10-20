use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// The demo scenarios, named as `--demo` takes them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scenario {
    /// Two of three profiles connected, four alerts.
    Normal,
    /// No profile yet.
    FirstRun,
    /// The service is not running.
    NoService,
    /// HomeLab's last Connect failed at step 3.
    ConnectFail,
    /// `splitwire.conf` is invalid; every profile stays off.
    SettingsInvalid,
}

impl Scenario {
    pub const ALL: [Self; 5] = [
        Self::Normal,
        Self::FirstRun,
        Self::NoService,
        Self::ConnectFail,
        Self::SettingsInvalid,
    ];

    pub fn key(self) -> &'static str {
        match self {
            Self::Normal => "normal",
            Self::FirstRun => "firstrun",
            Self::NoService => "noservice",
            Self::ConnectFail => "connectfail",
            Self::SettingsInvalid => "settingsinvalid",
        }
    }

    pub fn parse(text: &str) -> Option<Self> {
        let text = text.to_ascii_lowercase().replace(['-', '_'], "");
        Self::ALL.into_iter().find(|s| s.key() == text)
    }

    /// "Now" in every scenario: 19.10.2026 11:28:55, shown in UTC so the texts are the same on every machine.
    pub fn now() -> SystemTime {
        Self::at(11, 28, 55)
    }

    /// A time of the scenario's day.
    pub fn at(hour: u64, minute: u64, second: u64) -> SystemTime {
        // 2026-10-19 00:00:00 UTC.
        UNIX_EPOCH + Duration::from_secs(1_792_368_000 + hour * 3600 + minute * 60 + second)
    }

    /// The local offset the scenario's texts are written in.
    pub const OFFSET_SECS: i64 = 0;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::clock::{format_utc, unix_secs};

    #[test]
    fn scenarios_parse_by_key_and_the_day_is_fixed() {
        for scenario in Scenario::ALL {
            assert_eq!(Scenario::parse(scenario.key()), Some(scenario));
        }
        assert_eq!(Scenario::parse("connect-fail"), Some(Scenario::ConnectFail));
        assert_eq!(Scenario::parse("other"), None);
        assert_eq!(
            format_utc(unix_secs(Scenario::now())),
            "2026-10-19 11:28:55Z"
        );
    }
}
