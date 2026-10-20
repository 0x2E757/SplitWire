use std::net::{Ipv4Addr, SocketAddrV4};
use std::time::Duration;

use super::{KeyValueFile, SettingProblem, SettingValue};
use crate::list::DomainName;

/// `splitwire.conf`: the global settings. A missing key takes its default; an unknown key, a
/// duplicate or a bad value makes the file invalid, and then every profile stays off.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Settings {
    /// Where the DNS proxy listens; read at start only.
    pub listen: SocketAddrV4,
    /// Empty: the DNS servers of the default-route interface.
    pub fallback: Vec<Ipv4Addr>,
    /// Names always resolved through the fallback and never routed.
    pub bypass: Vec<DomainName>,
    pub max_ttl: Duration,
    pub idle_expiry: Duration,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            listen: SocketAddrV4::new(Ipv4Addr::new(127, 0, 0, 53), 53),
            fallback: Vec::new(),
            bypass: Vec::new(),
            max_ttl: Duration::from_secs(u64::from(Self::DEFAULT_MAX_TTL)),
            idle_expiry: Duration::from_secs(u64::from(Self::DEFAULT_IDLE_EXPIRY)),
        }
    }
}

impl Settings {
    pub const FILE: &'static str = "splitwire.conf";
    pub const MAX_FALLBACK: usize = 4;
    pub const DEFAULT_MAX_TTL: u32 = 300;
    pub const DEFAULT_IDLE_EXPIRY: u32 = 86_400;
    /// The keys applied without a reconnect; `listen` applies at the next start.
    pub const LIVE_KEYS: [&'static str; 4] = ["fallback", "bypass", "max_ttl", "idle_expiry"];

    /// `dry_run` allows any loopback port for `listen`.
    pub fn parse(text: &str, dry_run: bool) -> Result<Self, Vec<SettingProblem>> {
        let mut file = KeyValueFile::parse(Self::FILE, text);
        let mut settings = Self::default();
        for entry in file.entries.clone() {
            let value = entry.value.as_str();
            let (result, suggest) = match entry.key.as_str() {
                "listen" => (
                    SettingValue::listen(value, dry_run).map(|v| settings.listen = v),
                    Some("127.0.0.53:53".to_owned()),
                ),
                "fallback" => (
                    SettingValue::addresses(value, Self::MAX_FALLBACK)
                        .map(|v| settings.fallback = v),
                    None,
                ),
                "bypass" => (
                    SettingValue::names(value).map(|v| settings.bypass = v),
                    None,
                ),
                "max_ttl" => (
                    SettingValue::number(value, 30, 86_400)
                        .map(|v| settings.max_ttl = Duration::from_secs(u64::from(v))),
                    Some(Self::DEFAULT_MAX_TTL.to_string()),
                ),
                "idle_expiry" => (
                    SettingValue::number(value, 600, 604_800)
                        .map(|v| settings.idle_expiry = Duration::from_secs(u64::from(v))),
                    Some(Self::DEFAULT_IDLE_EXPIRY.to_string()),
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
    fn an_empty_file_gives_the_defaults() {
        let settings = Settings::parse("# nothing set\n", false).unwrap();
        assert_eq!(settings, Settings::default());
        assert_eq!(settings.listen.to_string(), "127.0.0.53:53");
        assert_eq!(settings.max_ttl, Duration::from_secs(300));
        assert_eq!(settings.idle_expiry, Duration::from_secs(86_400));
    }

    #[test]
    fn reads_every_key() {
        let settings = Settings::parse(
            "listen=127.0.0.1:15353\nfallback=1.1.1.1, 9.9.9.9\nbypass=vpn.example.com\n\
             max_ttl=60\nidle_expiry=3600\n",
            true,
        )
        .unwrap();
        assert_eq!(settings.listen.port(), 15353);
        assert_eq!(settings.fallback.len(), 2);
        assert_eq!(settings.bypass[0].as_str(), "vpn.example.com");
        assert_eq!(settings.max_ttl, Duration::from_secs(60));
        assert_eq!(settings.idle_expiry, Duration::from_secs(3600));
    }

    #[test]
    fn every_problem_has_its_line_and_a_fix_where_one_exists() {
        let problems = Settings::parse(
            "listen=127.0.0.1:15353\nfallback=1.1.1.1,2.2.2.2,3.3.3.3,4.4.4.4,5.5.5.5\n\
             bypass=intranet\nmax_ttl=5\nidle_expiry=60\nmaxttl=300\nmax_ttl=300\n",
            false,
        )
        .unwrap_err();
        let summary: Vec<(usize, &str, &str, Option<&str>)> = problems
            .iter()
            .map(|p| {
                (
                    p.line,
                    p.key.as_str(),
                    p.reason.as_str(),
                    p.suggest.as_deref(),
                )
            })
            .collect();
        assert_eq!(
            summary,
            [
                (7, "max_ttl", "max_ttl is already on line 4", None),
                (1, "listen", "the port must be 53", Some("127.0.0.53:53")),
                (2, "fallback", "at most 4 entries", None),
                (
                    3,
                    "bypass",
                    "intranet: a domain needs at least two labels, e.g. example.com",
                    None
                ),
                (4, "max_ttl", "must be a number 30-86400", Some("300")),
                (
                    5,
                    "idle_expiry",
                    "must be a number 600-604800",
                    Some("86400")
                ),
                (6, "maxttl", "unknown key", None),
            ]
        );
        assert!(problems.iter().all(|p| p.file == "splitwire.conf"));
        assert_eq!(problems[4].value, "5");
    }
}
