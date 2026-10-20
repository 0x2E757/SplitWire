use std::fmt;
use std::net::Ipv4Addr;
use std::str::FromStr;

/// An IPv4 network: an address with its host bits cleared and a prefix length.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Ipv4Net {
    addr: Ipv4Addr,
    prefix: u8,
}

impl Ipv4Net {
    /// The network holding `addr` with `prefix` bits; `None` when `prefix` > 32.
    pub fn new(addr: Ipv4Addr, prefix: u8) -> Option<Self> {
        (prefix <= 32).then(|| Self {
            addr: Ipv4Addr::from(u32::from(addr) & Self::mask(prefix)),
            prefix,
        })
    }

    pub fn host(addr: Ipv4Addr) -> Self {
        Self { addr, prefix: 32 }
    }

    pub fn addr(&self) -> Ipv4Addr {
        self.addr
    }

    pub fn prefix(&self) -> u8 {
        self.prefix
    }

    pub fn contains(&self, ip: Ipv4Addr) -> bool {
        u32::from(ip) & Self::mask(self.prefix) == u32::from(self.addr)
    }

    /// True when every address of `other` is in `self`.
    pub fn covers(&self, other: &Ipv4Net) -> bool {
        self.prefix <= other.prefix && self.contains(other.addr)
    }

    pub fn overlaps(&self, other: &Ipv4Net) -> bool {
        self.covers(other) || other.covers(self)
    }

    fn mask(prefix: u8) -> u32 {
        match prefix {
            0 => 0,
            p => u32::MAX << (32 - u32::from(p)),
        }
    }
}

impl fmt::Display for Ipv4Net {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}/{}", self.addr, self.prefix)
    }
}

impl FromStr for Ipv4Net {
    type Err = String;

    /// `a.b.c.d` (a /32) or `a.b.c.d/n`; host bits must be zero so typos are not silently widened.
    fn from_str(text: &str) -> Result<Self, String> {
        let (addr_text, prefix) = match text.split_once('/') {
            None => (text, 32),
            Some((addr, len)) => (
                addr,
                len.parse::<u8>()
                    .ok()
                    .filter(|p| *p <= 32)
                    .ok_or_else(|| format!("bad prefix length '{len}' (0-32)"))?,
            ),
        };
        let addr = Ipv4Addr::from_str(addr_text)
            .map_err(|_| format!("'{addr_text}' is not an IPv4 address"))?;
        let net = Self::new(addr, prefix).expect("prefix checked above");
        if net.addr != addr {
            return Err(format!("host bits are set; the network is {net}"));
        }
        Ok(net)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn net(text: &str) -> Ipv4Net {
        text.parse().unwrap()
    }

    #[test]
    fn parses_hosts_and_networks() {
        assert_eq!(
            net("192.0.2.93"),
            Ipv4Net::host(Ipv4Addr::new(192, 0, 2, 93))
        );
        assert_eq!(net("10.0.0.0/8").prefix(), 8);
        assert_eq!(net("0.0.0.0/0").to_string(), "0.0.0.0/0");
    }

    #[test]
    fn rejects_bad_text() {
        assert!(
            "10.0.0.1/8"
                .parse::<Ipv4Net>()
                .unwrap_err()
                .contains("10.0.0.0/8")
        );
        assert!("10.0.0.0/33".parse::<Ipv4Net>().is_err());
        assert!("10.0.0/8".parse::<Ipv4Net>().is_err());
        assert!("300.1.1.1".parse::<Ipv4Net>().is_err());
    }

    #[test]
    fn containment_and_overlap() {
        let ten = net("10.0.0.0/8");
        assert!(ten.contains(Ipv4Addr::new(10, 9, 8, 7)));
        assert!(!ten.contains(Ipv4Addr::new(11, 0, 0, 1)));
        assert!(ten.covers(&net("10.66.0.0/16")));
        assert!(!net("10.66.0.0/16").covers(&ten));
        assert!(net("10.66.0.0/16").overlaps(&ten));
        assert!(!net("192.168.1.0/24").overlaps(&ten));
        assert!(net("0.0.0.0/0").covers(&ten));
    }
}
