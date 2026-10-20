use std::collections::HashMap;
use std::net::Ipv4Addr;

use super::{Base64, ConfLine, Endpoint, LineOutcome, SecretKey, TunnelAddress};
use crate::list::DomainName;

/// What SplitWire reads from a WireGuard `.conf`: one IPv4 address, one peer.
#[derive(Clone, Debug)]
pub struct WgConf {
    pub private_key: SecretKey,
    pub address: TunnelAddress,
    pub dns: Vec<Ipv4Addr>,
    pub mtu: Option<u16>,
    pub peer_public_key: [u8; 32],
    pub preshared_key: Option<SecretKey>,
    pub endpoint: Endpoint,
    pub keepalive: u16,
}

/// A parsed `.conf`: every line's outcome, and the values when no line is a problem.
#[derive(Debug)]
pub struct ParsedConf {
    pub lines: Vec<ConfLine>,
    conf: Option<WgConf>,
}

impl ParsedConf {
    pub fn conf(&self) -> Option<&WgConf> {
        self.conf.as_ref()
    }

    pub fn into_conf(self) -> Option<WgConf> {
        self.conf
    }

    pub fn problems(&self) -> impl Iterator<Item = &ConfLine> {
        self.lines
            .iter()
            .filter(|line| line.outcome == LineOutcome::Problem)
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Section {
    None,
    Interface,
    Peer,
    /// An unknown section or a second `[Peer]`: its lines are not read.
    Skipped(&'static str),
}

/// Reads a WireGuard `.conf` line by line. Keys are case-insensitive, `#` starts a comment, and
/// `Address`, `DNS` and `AllowedIPs` may repeat; every other key appears once.
pub struct WgConfParser {
    section: Section,
    interfaces: usize,
    peers: usize,
    first_line: HashMap<&'static str, usize>,
    lines: Vec<ConfLine>,
    private_key: Option<SecretKey>,
    address: Option<TunnelAddress>,
    dns: Vec<Ipv4Addr>,
    mtu: Option<u16>,
    public_key: Option<[u8; 32]>,
    preshared_key: Option<SecretKey>,
    endpoint: Option<Endpoint>,
    keepalive: u16,
}

impl WgConfParser {
    pub const MAX_DNS: usize = 4;
    const MAX_KEY_NAME: usize = 24;
    const SEARCH_DOMAIN: &'static str =
        "Search domains are not used; add the domain to the list if needed";
    const NO_IPV6: &'static str = "IPv6 is not supported";
    const KEYS: [(&'static str, &'static str); 16] = [
        ("privatekey", "PrivateKey"),
        ("address", "Address"),
        ("dns", "DNS"),
        ("mtu", "MTU"),
        ("listenport", "ListenPort"),
        ("table", "Table"),
        ("preup", "PreUp"),
        ("postup", "PostUp"),
        ("predown", "PreDown"),
        ("postdown", "PostDown"),
        ("saveconfig", "SaveConfig"),
        ("fwmark", "FwMark"),
        ("publickey", "PublicKey"),
        ("presharedkey", "PresharedKey"),
        ("endpoint", "Endpoint"),
        ("persistentkeepalive", "PersistentKeepalive"),
    ];

    pub fn parse(text: &str) -> ParsedConf {
        let mut parser = Self {
            section: Section::None,
            interfaces: 0,
            peers: 0,
            first_line: HashMap::new(),
            lines: Vec::new(),
            private_key: None,
            address: None,
            dns: Vec::new(),
            mtu: None,
            public_key: None,
            preshared_key: None,
            endpoint: None,
            keepalive: 0,
        };
        for (index, raw) in text.lines().enumerate() {
            let content = raw.split('#').next().unwrap_or_default().trim();
            if !content.is_empty() {
                parser.line(index + 1, content);
            }
        }
        parser.finish()
    }

    fn line(&mut self, line: usize, content: &str) {
        if content.starts_with('[') && content.ends_with(']') {
            return self.header(line, &content[1..content.len() - 1]);
        }
        let Some((key, value)) = content.split_once('=') else {
            return self
                .lines
                .push(ConfLine::problem(line, "", "expected Key = value"));
        };
        let (key, value) = (key.trim(), value.trim());
        // A stray key line (`<base64>=`) splits at its padding: never show such a "key".
        if !Self::is_key_name(key) {
            return self
                .lines
                .push(ConfLine::problem(line, "", "expected Key = value"));
        }
        let known = Self::KEYS
            .iter()
            .find(|(lower, _)| key.eq_ignore_ascii_case(lower))
            .map(|(_, canonical)| *canonical);
        match (self.section, known) {
            (Section::None, _) => self.lines.push(ConfLine::problem(
                line,
                known.unwrap_or_default(),
                "outside [Interface] and [Peer]",
            )),
            (Section::Skipped(reason), _) => self.lines.push(ConfLine::ignored(
                line,
                known.unwrap_or(key),
                format!("{} = {}", known.unwrap_or(key), ConfLine::ELIDED),
                reason,
            )),
            (Section::Interface, Some(key)) if Self::is_interface_key(key) => {
                self.value(line, key, value)
            }
            (Section::Peer, Some(key)) if !Self::is_interface_key(key) => {
                self.value(line, key, value)
            }
            (Section::Peer, None) if key.eq_ignore_ascii_case("allowedips") => {
                self.lines.push(ConfLine::ignored(
                    line,
                    "AllowedIPs",
                    format!("AllowedIPs = {value}"),
                    "SplitWire routes only the names on the profile's list",
                ))
            }
            _ => self.lines.push(ConfLine::ignored(
                line,
                known.unwrap_or(key),
                format!("{} = {}", known.unwrap_or(key), ConfLine::ELIDED),
                "unknown key",
            )),
        }
    }

    fn header(&mut self, line: usize, name: &str) {
        let name = name.trim();
        if name.eq_ignore_ascii_case("interface") {
            self.interfaces += 1;
            if self.interfaces == 1 {
                self.section = Section::Interface;
            } else {
                self.section = Section::Skipped("in a second [Interface]");
                self.header_problem(line, "Interface", "more than one [Interface]");
            }
        } else if name.eq_ignore_ascii_case("peer") {
            self.peers += 1;
            if self.peers == 1 {
                self.section = Section::Peer;
            } else {
                self.section = Section::Skipped("in a second [Peer]");
                self.header_problem(line, "Peer", "only one [Peer] is supported");
            }
        } else {
            self.section = Section::Skipped("in an unknown section");
            self.header_problem(line, name, "unknown section");
        }
    }

    fn header_problem(&mut self, line: usize, name: &str, reason: &str) {
        let mut problem = ConfLine::problem(line, "", reason);
        let shown = if Self::is_key_name(name) {
            name
        } else {
            ConfLine::ELIDED
        };
        problem.shown = format!("[{shown}]");
        self.lines.push(problem);
    }

    /// A key name as WireGuard and its forks write them: short, a letter then letters and digits.
    fn is_key_name(key: &str) -> bool {
        key.len() <= Self::MAX_KEY_NAME
            && key.starts_with(|c: char| c.is_ascii_alphabetic())
            && key.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
    }

    fn is_interface_key(key: &str) -> bool {
        !matches!(
            key,
            "PublicKey" | "PresharedKey" | "Endpoint" | "PersistentKeepalive"
        )
    }

    fn value(&mut self, line: usize, key: &'static str, value: &str) {
        if let Some(reason) = Self::ignored_reason(key) {
            let shown = if key.starts_with("Pre") || key.starts_with("Post") {
                ConfLine::ELIDED
            } else {
                value
            };
            return self.lines.push(ConfLine::ignored(
                line,
                key,
                format!("{key} = {shown}"),
                reason,
            ));
        }
        if value.is_empty() {
            return self.lines.push(ConfLine::problem(line, key, "no value"));
        }
        match key {
            "Address" => return self.address(line, value),
            "DNS" => return self.dns(line, value),
            _ => {}
        }
        if let Some(first) = self.first_line.get(key) {
            let reason = format!("already line {first}");
            return self.lines.push(ConfLine::problem(line, key, reason));
        }
        self.first_line.insert(key, line);
        let result = match key {
            "PrivateKey" => Base64::decode_secret(value)
                .map(|k| self.private_key = Some(k))
                .map_err(String::from),
            "PresharedKey" => Base64::decode_secret(value)
                .map(|k| self.preshared_key = Some(k))
                .map_err(String::from),
            "PublicKey" => Base64::decode_key(value)
                .map(|k| self.public_key = Some(k))
                .map_err(String::from),
            "Endpoint" => Endpoint::parse(value).map(|e| self.endpoint = Some(e)),
            "MTU" => Self::number(value, 1280, 1500).map(|m| self.mtu = Some(m)),
            _ => Self::keepalive(value).map(|k| self.keepalive = k),
        };
        self.lines.push(match result {
            Ok(()) => ConfLine::read(line, key),
            Err(reason) => ConfLine::problem(line, key, reason),
        });
    }

    fn ignored_reason(key: &str) -> Option<&'static str> {
        Some(match key {
            "ListenPort" => "A random port is used",
            "Table" => "SplitWire manages its own routes",
            "PreUp" | "PostUp" | "PreDown" | "PostDown" => "Scripts are never run",
            "SaveConfig" => "Nothing is written back to the file",
            "FwMark" => "Not used on Windows",
            _ => return None,
        })
    }

    fn address(&mut self, line: usize, value: &str) {
        let mut ignored = Vec::new();
        let mut read = false;
        for item in Self::items(value) {
            if item.contains(':') {
                ignored.push((Self::NO_IPV6, item));
                continue;
            }
            if self.address.is_some() {
                let problem =
                    ConfLine::problem(line, "Address", "only one IPv4 address is supported");
                return self.lines.push(problem);
            }
            match TunnelAddress::parse(item) {
                Ok(address) => {
                    self.address = Some(address);
                    read = true;
                }
                Err(reason) => return self.lines.push(ConfLine::problem(line, "Address", reason)),
            }
        }
        self.list_line(line, "Address", read, &ignored);
    }

    fn dns(&mut self, line: usize, value: &str) {
        let mut ignored = Vec::new();
        let mut read = false;
        for item in Self::items(value) {
            if item.contains(':') {
                ignored.push((Self::NO_IPV6, item));
            } else if let Ok(server) = item.parse::<Ipv4Addr>() {
                if self.dns.len() == Self::MAX_DNS {
                    let reason = format!("at most {} DNS servers", Self::MAX_DNS);
                    return self.lines.push(ConfLine::problem(line, "DNS", reason));
                }
                self.dns.push(server);
                read = true;
            } else if Self::is_search_domain(item) {
                ignored.push((Self::SEARCH_DOMAIN, item));
            } else {
                let reason = "not an IPv4 address or a search domain";
                return self.lines.push(ConfLine::problem(line, "DNS", reason));
            }
        }
        self.list_line(line, "DNS", read, &ignored);
    }

    /// A list line: read when anything was read, and one ignored entry per reason for the rest,
    /// shown as `DNS = …, partner.local`.
    fn list_line(&mut self, line: usize, key: &str, read: bool, ignored: &[(&'static str, &str)]) {
        if read {
            self.lines.push(ConfLine::read(line, key));
        }
        let mut reasons: Vec<&'static str> = ignored.iter().map(|(reason, _)| *reason).collect();
        reasons.dedup();
        for reason in reasons {
            let items: Vec<&str> = ignored
                .iter()
                .filter(|(r, _)| *r == reason)
                .map(|(_, item)| *item)
                .collect();
            let prefix = if read {
                format!("{}, ", ConfLine::ELIDED)
            } else {
                String::new()
            };
            let shown = format!("{key} = {prefix}{}", items.join(", "));
            self.lines.push(ConfLine::ignored(line, key, shown, reason));
        }
    }

    /// `corp.example.com`, or a single label such as `corp`.
    fn is_search_domain(item: &str) -> bool {
        DomainName::parse(item).is_ok()
            || (!item.contains('.') && DomainName::parse(&format!("{item}.x")).is_ok())
    }

    fn items(value: &str) -> impl Iterator<Item = &str> {
        value
            .split(',')
            .map(str::trim)
            .filter(|item| !item.is_empty())
    }

    fn number(value: &str, min: u16, max: u16) -> Result<u16, String> {
        value
            .parse::<u16>()
            .ok()
            .filter(|n| (min..=max).contains(n))
            .ok_or_else(|| format!("must be a number {min}-{max}"))
    }

    fn keepalive(value: &str) -> Result<u16, String> {
        if value.eq_ignore_ascii_case("off") {
            return Ok(0);
        }
        value
            .parse()
            .map_err(|_| "must be off or a number of seconds 0-65535".into())
    }

    fn finish(mut self) -> ParsedConf {
        for (key, section, missing) in [
            ("PrivateKey", "[Interface]", self.private_key.is_none()),
            ("Address", "[Interface]", self.address.is_none()),
            ("PublicKey", "[Peer]", self.public_key.is_none()),
            ("Endpoint", "[Peer]", self.endpoint.is_none()),
        ] {
            let has_problem = self
                .lines
                .iter()
                .any(|l| l.key == key && l.outcome == LineOutcome::Problem);
            if missing && !has_problem {
                self.lines.push(ConfLine::missing(key, section));
            }
        }
        let valid = !self.lines.iter().any(|l| l.outcome == LineOutcome::Problem);
        let conf = match (
            self.private_key,
            self.address,
            self.public_key,
            self.endpoint,
        ) {
            (Some(private_key), Some(address), Some(peer_public_key), Some(endpoint)) if valid => {
                Some(WgConf {
                    private_key,
                    address,
                    dns: self.dns,
                    mtu: self.mtu,
                    peer_public_key,
                    preshared_key: self.preshared_key,
                    endpoint,
                    keepalive: self.keepalive,
                })
            }
            _ => None,
        };
        ParsedConf {
            lines: self.lines,
            conf,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profile::fixtures::{PRIVATE, PUBLIC, full_conf};

    fn outcomes(parsed: &ParsedConf) -> Vec<(usize, &str, LineOutcome, &str)> {
        parsed
            .lines
            .iter()
            .map(|l| (l.line, l.shown.as_str(), l.outcome, l.reason.as_str()))
            .collect()
    }

    #[test]
    fn reads_a_full_conf_and_reports_every_line() {
        let parsed = WgConfParser::parse(&full_conf());
        let conf = parsed.conf().expect("valid");
        assert_eq!(conf.address.to_string(), "10.88.4.12/24");
        assert_eq!(
            conf.dns,
            [Ipv4Addr::new(10, 66, 0, 53), Ipv4Addr::new(10, 66, 0, 54)]
        );
        assert_eq!(conf.mtu, Some(1380));
        assert_eq!(conf.endpoint.to_string(), "vpn.example.com:51820");
        assert_eq!(conf.keepalive, 25);
        assert_eq!(Base64::encode(&conf.peer_public_key), PUBLIC);
        assert_eq!(Base64::encode(conf.private_key.expose()), PRIVATE);
        assert!(conf.preshared_key.is_some());
        use LineOutcome::{Ignored, Read};
        assert_eq!(
            outcomes(&parsed),
            [
                (2, "PrivateKey = …", Read, ""),
                (3, "Address = …", Read, ""),
                (
                    3,
                    "Address = …, fd86:ea04:1115::46/64",
                    Ignored,
                    "IPv6 is not supported"
                ),
                (4, "DNS = …", Read, ""),
                (
                    4,
                    "DNS = …, corp.example.com",
                    Ignored,
                    WgConfParser::SEARCH_DOMAIN
                ),
                (5, "MTU = …", Read, ""),
                (6, "ListenPort = 51820", Ignored, "A random port is used"),
                (
                    7,
                    "Table = off",
                    Ignored,
                    "SplitWire manages its own routes"
                ),
                (8, "PostUp = …", Ignored, "Scripts are never run"),
                (
                    9,
                    "SaveConfig = true",
                    Ignored,
                    "Nothing is written back to the file"
                ),
                (10, "Jc = …", Ignored, "unknown key"),
                (14, "PublicKey = …", Read, ""),
                (15, "PresharedKey = …", Read, ""),
                (
                    16,
                    "AllowedIPs = 0.0.0.0/0, ::/0",
                    Ignored,
                    "SplitWire routes only the names on the profile's list"
                ),
                (17, "Endpoint = …", Read, ""),
                (18, "PersistentKeepalive = …", Read, ""),
            ]
        );
    }

    #[test]
    fn missing_keys_are_problems_of_the_file() {
        let parsed = WgConfParser::parse("[Interface]\nDNS = 10.0.0.1\n");
        assert!(parsed.conf().is_none());
        let missing: Vec<(usize, &str)> = parsed
            .problems()
            .map(|l| (l.line, l.reason.as_str()))
            .collect();
        assert_eq!(
            missing,
            [
                (0, "PrivateKey is missing from [Interface]"),
                (0, "Address is missing from [Interface]"),
                (0, "PublicKey is missing from [Peer]"),
                (0, "Endpoint is missing from [Peer]"),
            ]
        );
    }

    #[test]
    fn bad_values_are_problems_that_never_show_the_value() {
        let short_key = &PRIVATE[..40];
        let text = format!(
            "Stray = 1\n[Interface]\nPrivateKey = {short_key}\nAddress = 10.0.0.2/32, 10.0.0.3/32\n\
             MTU = 9000\nDNS = 10.0.0.1, not_a host\n{PRIVATE}\n[Peer]\nPublicKey = {PUBLIC}\n\
             PublicKey = {PUBLIC}\nPresharedKey = {PRIVATE}!\nEndpoint = [2001:db8::1]:51820\n\
             PersistentKeepalive = soon\n[Peer]\nPublicKey = {PUBLIC}\n[Extra]\nX = 1\n[{PRIVATE}]\n"
        );
        let parsed = WgConfParser::parse(&text);
        assert!(parsed.conf().is_none());
        let problems: Vec<(usize, &str, &str)> = parsed
            .problems()
            .map(|l| (l.line, l.shown.as_str(), l.reason.as_str()))
            .collect();
        assert_eq!(
            problems,
            [
                (1, "…", "outside [Interface] and [Peer]"),
                (3, "PrivateKey = …", Base64::WRONG_LENGTH),
                (4, "Address = …", "only one IPv4 address is supported"),
                (5, "MTU = …", "must be a number 1280-1500"),
                (6, "DNS = …", "not an IPv4 address or a search domain"),
                (7, "…", "expected Key = value"),
                (10, "PublicKey = …", "already line 9"),
                (11, "PresharedKey = …", Base64::NOT_BASE64),
                (12, "Endpoint = …", "IPv6 is not supported"),
                (
                    13,
                    "PersistentKeepalive = …",
                    "must be off or a number of seconds 0-65535"
                ),
                (14, "[Peer]", "only one [Peer] is supported"),
                (16, "[Extra]", "unknown section"),
                (18, "[…]", "unknown section"),
            ]
        );
        let shown: String = parsed.lines.iter().map(|l| format!("{l:?}")).collect();
        assert!(!shown.contains(&PRIVATE[..12]));
        assert!(!shown.contains(&short_key[..12]));
        let skipped: Vec<(usize, &str)> = parsed
            .lines
            .iter()
            .filter(|l| l.outcome == LineOutcome::Ignored)
            .map(|l| (l.line, l.reason.as_str()))
            .collect();
        assert_eq!(
            skipped,
            [(15, "in a second [Peer]"), (17, "in an unknown section")]
        );
    }

    #[test]
    fn keys_are_case_insensitive_and_keepalive_may_be_off() {
        let text = format!(
            "[interface]\nprivatekey={PRIVATE}\naddress=10.9.0.2\n[PEER]\npublickey={PUBLIC}\n\
             endpoint=198.51.100.7:51820\npersistentkeepalive=off\n"
        );
        let conf = WgConfParser::parse(&text).into_conf().unwrap();
        assert_eq!(conf.keepalive, 0);
        assert_eq!(conf.address.prefix, 32);
        assert!(conf.dns.is_empty());
        assert!(conf.preshared_key.is_none());
    }

    #[test]
    fn a_line_of_only_ignored_items_is_ignored() {
        let text = format!("[Interface]\nPrivateKey = {PRIVATE}\nAddress = fd00::2/64\n");
        let parsed = WgConfParser::parse(&text);
        assert_eq!(
            outcomes(&parsed)[1],
            (
                3,
                "Address = fd00::2/64",
                LineOutcome::Ignored,
                "IPv6 is not supported"
            )
        );
        assert!(
            parsed
                .problems()
                .any(|l| l.reason == "Address is missing from [Interface]")
        );
    }
}
