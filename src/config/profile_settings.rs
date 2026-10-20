use std::net::SocketAddrV4;

use super::{KeyValueFile, SettingProblem, SettingValue};
use crate::tunnel::TunnelSpec;

/// `profiles\<name>\profile.conf`. An invalid file blocks only its profile.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ProfileSettings {
    /// Applies at the next start: whether `connected=` restores the profile.
    pub connect_at_boot: bool,
    /// Empty: the profile's `.conf` `DNS=`.
    pub upstream: Vec<SocketAddrV4>,
    /// Empty: the profile's MTU, else 1420.
    pub mtu: Option<u16>,
}

impl ProfileSettings {
    pub const FILE: &'static str = "profile.conf";
    pub const MAX_UPSTREAM: usize = 4;
    /// The keys whose change reconnects a connected profile.
    pub const RECONNECT_KEYS: [&'static str; 2] = ["upstream", "mtu"];
    /// What a new profile's file holds.
    pub const NEW_FILE: &'static str = "connect_at_boot=false\n";

    /// The path the problems name, such as `profiles\AcmeCorp\profile.conf`.
    pub fn file_label(profile: &str) -> String {
        format!("profiles\\{profile}\\{}", Self::FILE)
    }

    pub fn parse(profile: &str, text: &str) -> Result<Self, Vec<SettingProblem>> {
        let mut file = KeyValueFile::parse(&Self::file_label(profile), text);
        let mut settings = Self::default();
        for entry in file.entries.clone() {
            let value = entry.value.as_str();
            let (result, suggest) = match entry.key.as_str() {
                "connect_at_boot" => (
                    SettingValue::boolean(value).map(|v| settings.connect_at_boot = v),
                    None,
                ),
                "upstream" if value.is_empty() => (Ok(()), None),
                "upstream" => (
                    SettingValue::servers(value, 1, Self::MAX_UPSTREAM)
                        .map(|v| settings.upstream = v),
                    None,
                ),
                "mtu" if value.is_empty() => (Ok(()), None),
                "mtu" => (
                    SettingValue::number(value, 1280, 1500).map(|v| settings.mtu = Some(v as u16)),
                    Some(TunnelSpec::DEFAULT_MTU.to_string()),
                ),
                _ => (Err("unknown key".into()), None),
            };
            file.check(&entry, result, suggest.as_deref());
        }
        file.into_result(settings)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_and_values() {
        assert_eq!(
            ProfileSettings::parse("AcmeCorp", ProfileSettings::NEW_FILE).unwrap(),
            ProfileSettings::default()
        );
        let settings = ProfileSettings::parse(
            "AcmeCorp",
            "connect_at_boot=true\nupstream=10.66.0.53, 10.66.0.54:5353\nmtu=1380\n",
        )
        .unwrap();
        assert!(settings.connect_at_boot);
        assert_eq!(settings.upstream.len(), 2);
        assert_eq!(settings.mtu, Some(1380));
        let empty = ProfileSettings::parse("AcmeCorp", "upstream=\nmtu=\n").unwrap();
        assert!(empty.upstream.is_empty() && empty.mtu.is_none());
    }

    #[test]
    fn problems_name_the_profile_file_and_suggest_a_fix() {
        let problems = ProfileSettings::parse(
            "AcmeCorp",
            "connect_at_boot=yes\nupstream=dns.example.com\nmtu=9000\ncolour=2\n",
        )
        .unwrap_err();
        let summary: Vec<(usize, &str, Option<&str>)> = problems
            .iter()
            .map(|p| (p.line, p.reason.as_str(), p.suggest.as_deref()))
            .collect();
        assert_eq!(
            summary,
            [
                (1, "must be true or false", None),
                (
                    2,
                    "dns.example.com is not an IPv4 address (optionally with :port)",
                    None
                ),
                (3, "must be a number 1280-1500", Some("1420")),
                (4, "unknown key", None),
            ]
        );
        assert_eq!(problems[0].file, "profiles\\AcmeCorp\\profile.conf");
    }
}
