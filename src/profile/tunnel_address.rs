use std::fmt;
use std::net::Ipv4Addr;

use crate::net::Ipv4Net;

/// The profile's `.conf` `Address`: the adapter gets the address as a /32, and the prefix gives
/// the tunnel subnet, which never overlaps another profile's and is never routed by another tunnel.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TunnelAddress {
    pub address: Ipv4Addr,
    pub prefix: u8,
}

impl TunnelAddress {
    pub const MIN_PREFIX: u8 = 8;

    /// `10.88.4.12/24` or `10.88.4.12` (a /32); host bits are expected here.
    pub fn parse(text: &str) -> Result<Self, String> {
        let (address, prefix) = match text.trim().split_once('/') {
            Some((address, prefix)) => (
                address,
                prefix
                    .parse::<u8>()
                    .ok()
                    .filter(|p| *p <= 32)
                    .ok_or_else(|| format!("bad prefix length '{prefix}' (0-32)"))?,
            ),
            None => (text.trim(), 32),
        };
        let address: Ipv4Addr = address
            .parse()
            .map_err(|_| format!("'{address}' is not an IPv4 address"))?;
        if prefix < Self::MIN_PREFIX {
            return Err(format!(
                "the tunnel subnet must be /{} or longer",
                Self::MIN_PREFIX
            ));
        }
        Ok(Self { address, prefix })
    }

    pub fn subnet(&self) -> Ipv4Net {
        Ipv4Net::new(self.address, self.prefix).expect("prefix checked at parse")
    }
}

impl fmt::Display for TunnelAddress {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}/{}", self.address, self.prefix)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_the_address_and_gives_the_subnet() {
        let address = TunnelAddress::parse("10.88.4.12/24").unwrap();
        assert_eq!(address.address, Ipv4Addr::new(10, 88, 4, 12));
        assert_eq!(address.subnet().to_string(), "10.88.4.0/24");
        assert_eq!(address.to_string(), "10.88.4.12/24");
        assert_eq!(TunnelAddress::parse("10.77.0.5").unwrap().prefix, 32);
    }

    #[test]
    fn refuses_wide_subnets_and_bad_text() {
        assert!(TunnelAddress::parse("10.0.0.1/7").is_err());
        assert!(TunnelAddress::parse("10.0.0.1/33").is_err());
        assert!(TunnelAddress::parse("fd00::1/64").is_err());
    }
}
