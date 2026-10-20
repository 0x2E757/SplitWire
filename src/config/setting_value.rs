use std::net::{Ipv4Addr, SocketAddrV4};

use crate::list::DomainName;

/// Parsers for setting values; each error is the reason a problem shows. An empty list value is
/// an empty list.
pub struct SettingValue;

impl SettingValue {
    pub fn boolean(value: &str) -> Result<bool, String> {
        match value {
            "true" => Ok(true),
            "false" => Ok(false),
            _ => Err("must be true or false".into()),
        }
    }

    pub fn number(value: &str, min: u32, max: u32) -> Result<u32, String> {
        value
            .parse::<u32>()
            .ok()
            .filter(|n| (min..=max).contains(n))
            .ok_or_else(|| format!("must be a number {min}-{max}"))
    }

    /// Up to `max` IPv4 addresses, comma-separated, each once.
    pub fn addresses(value: &str, max: usize) -> Result<Vec<Ipv4Addr>, String> {
        Self::list(value, max, |item| {
            item.parse()
                .map_err(|_| format!("{item} is not an IPv4 address"))
        })
    }

    /// `min` to `max` servers, `IPv4[:port]`, port 53 by default.
    pub fn servers(value: &str, min: usize, max: usize) -> Result<Vec<SocketAddrV4>, String> {
        let servers = Self::list(value, max, |item| {
            item.parse()
                .or_else(|_| item.parse().map(|ip| SocketAddrV4::new(ip, 53)))
                .map_err(|_| format!("{item} is not an IPv4 address (optionally with :port)"))
        })?;
        if servers.len() < min {
            return Err(format!("needs at least {min} server"));
        }
        Ok(servers)
    }

    /// Names with at least 2 labels, comma-separated.
    pub fn names(value: &str) -> Result<Vec<DomainName>, String> {
        Self::list(value, usize::MAX, |item| {
            DomainName::parse(item).map_err(|reason| format!("{item}: {reason}"))
        })
    }

    /// A loopback IPv4 address with its port; port 53 unless `any_port` (a dry run).
    pub fn listen(value: &str, any_port: bool) -> Result<SocketAddrV4, String> {
        let address: SocketAddrV4 = value
            .parse()
            .map_err(|_| "must be a loopback IPv4 address with its port, such as 127.0.0.53:53")?;
        if !address.ip().is_loopback() {
            return Err("must be a loopback address (127.x.x.x)".into());
        }
        if !any_port && address.port() != 53 {
            return Err("the port must be 53".into());
        }
        Ok(address)
    }

    fn list<T: PartialEq>(
        value: &str,
        max: usize,
        parse: impl Fn(&str) -> Result<T, String>,
    ) -> Result<Vec<T>, String> {
        let mut items = Vec::new();
        for item in value.split(',').map(str::trim).filter(|i| !i.is_empty()) {
            let parsed = parse(item)?;
            if items.contains(&parsed) {
                return Err(format!("{item} is listed twice"));
            }
            items.push(parsed);
        }
        if items.len() > max {
            return Err(format!("at most {max} entries"));
        }
        Ok(items)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_scalars() {
        assert_eq!(SettingValue::boolean("true"), Ok(true));
        assert!(SettingValue::boolean("yes").is_err());
        assert_eq!(SettingValue::number("300", 30, 86_400), Ok(300));
        assert_eq!(
            SettingValue::number("5", 30, 86_400).unwrap_err(),
            "must be a number 30-86400"
        );
        assert!(SettingValue::number("-1", 0, 9).is_err());
    }

    #[test]
    fn parses_lists() {
        assert!(SettingValue::addresses("", 4).unwrap().is_empty());
        assert_eq!(
            SettingValue::addresses("1.1.1.1, 9.9.9.9", 4).unwrap(),
            [Ipv4Addr::new(1, 1, 1, 1), Ipv4Addr::new(9, 9, 9, 9)]
        );
        assert!(SettingValue::addresses("1.1.1.1,1.1.1.1", 4).is_err());
        assert!(SettingValue::addresses("1.1.1.1,2.2.2.2,3.3.3.3,4.4.4.4,5.5.5.5", 4).is_err());
        assert_eq!(
            SettingValue::servers("10.0.0.2, 10.0.0.3:5353", 1, 4).unwrap(),
            [
                "10.0.0.2:53".parse().unwrap(),
                "10.0.0.3:5353".parse().unwrap()
            ]
        );
        assert!(SettingValue::servers("", 1, 4).is_err());
        assert!(SettingValue::servers("dns.google", 1, 4).is_err());
        assert_eq!(SettingValue::names("a.example.com").unwrap().len(), 1);
        assert!(SettingValue::names("intranet").is_err());
    }

    #[test]
    fn listen_is_loopback_and_port_53_unless_dry_run() {
        assert!(SettingValue::listen("127.0.0.53:53", false).is_ok());
        assert!(SettingValue::listen("127.0.0.1:15353", false).is_err());
        assert!(SettingValue::listen("127.0.0.1:15353", true).is_ok());
        assert!(SettingValue::listen("192.168.1.2:53", true).is_err());
        assert!(SettingValue::listen("127.0.0.53", true).is_err());
    }
}
