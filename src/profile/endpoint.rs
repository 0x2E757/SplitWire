use std::fmt;
use std::net::Ipv4Addr;

use crate::list::DomainName;

/// Where a peer is reached: an IPv4 address, used as is, or a name resolved through the fallback DNS.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EndpointHost {
    Address(Ipv4Addr),
    Name(DomainName),
}

impl fmt::Display for EndpointHost {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EndpointHost::Address(address) => address.fmt(f),
            EndpointHost::Name(name) => name.fmt(f),
        }
    }
}

/// A peer endpoint, `host:port`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Endpoint {
    pub host: EndpointHost,
    pub port: u16,
}

impl Endpoint {
    pub fn parse(text: &str) -> Result<Self, String> {
        let text = text.trim();
        if text.starts_with('[') || text.matches(':').count() > 1 {
            return Err("IPv6 is not supported".into());
        }
        let (host, port) = text
            .rsplit_once(':')
            .ok_or("expected host:port, such as vpn.example.com:51820")?;
        let port = port
            .parse::<u16>()
            .ok()
            .filter(|port| *port != 0)
            .ok_or("the port must be 1-65535")?;
        let host = if host.bytes().all(|b| b.is_ascii_digit() || b == b'.') {
            EndpointHost::Address(
                host.parse()
                    .map_err(|_| format!("'{host}' is not an IPv4 address"))?,
            )
        } else {
            EndpointHost::Name(
                DomainName::parse(host).map_err(|reason| format!("'{host}': {reason}"))?,
            )
        };
        Ok(Self { host, port })
    }

    /// The address when the host is an IP literal.
    pub fn literal(&self) -> Option<Ipv4Addr> {
        match self.host {
            EndpointHost::Address(address) => Some(address),
            EndpointHost::Name(_) => None,
        }
    }
}

impl fmt::Display for Endpoint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.host, self.port)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_names_and_addresses() {
        let named = Endpoint::parse("VPN.Example.com:51820").unwrap();
        assert_eq!(named.to_string(), "vpn.example.com:51820");
        assert_eq!(named.literal(), None);
        let literal = Endpoint::parse(" 198.51.100.7:51820 ").unwrap();
        assert_eq!(literal.literal(), Some(Ipv4Addr::new(198, 51, 100, 7)));
        assert_eq!(literal.port, 51820);
    }

    #[test]
    fn refuses_ipv6_bad_ports_and_bad_hosts() {
        for (text, reason) in [
            ("[2001:db8::1]:51820", "IPv6 is not supported"),
            ("2001:db8::1", "IPv6 is not supported"),
            ("vpn.example.com", "expected host:port"),
            ("vpn.example.com:0", "the port must be 1-65535"),
            ("vpn.example.com:70000", "the port must be 1-65535"),
            ("300.1.2.3:51820", "is not an IPv4 address"),
            ("vpn:51820", "at least two labels"),
        ] {
            let error = Endpoint::parse(text).unwrap_err();
            assert!(error.contains(reason), "{text}: {error}");
        }
    }
}
