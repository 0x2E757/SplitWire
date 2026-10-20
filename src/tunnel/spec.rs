use std::net::{Ipv4Addr, SocketAddrV4};

use crate::profile::SecretKey;

/// Everything a tunnel kind needs to bring one profile's adapter up. It holds the decrypted keys,
/// so it lives only while the profile is connected.
#[derive(Clone, Debug)]
pub struct TunnelSpec {
    /// The profile name; the adapter is `SplitWire-<name>`.
    pub name: String,
    /// Made at import and kept, so Windows keeps one network for the adapter.
    pub adapter_guid: u128,
    /// The tunnel address; the adapter gets it as a /32.
    pub address: Ipv4Addr,
    /// The effective MTU: the `mtu` override, else the profile's, else 1420.
    pub mtu: u16,
    pub private_key: SecretKey,
    pub peer_public_key: [u8; 32],
    pub preshared_key: Option<SecretKey>,
    /// The `Endpoint=` host as written: a name, re-resolved through the fallback DNS when the
    /// handshake goes stale, or an IPv4 literal.
    pub endpoint_host: String,
    /// The endpoint as resolved through the fallback DNS.
    pub endpoint: SocketAddrV4,
    /// Persistent keepalive in seconds; 0 is off.
    pub keepalive: u16,
}

impl TunnelSpec {
    pub const ADAPTER_PREFIX: &'static str = "SplitWire-";
    /// The tunnel type every SplitWire adapter is created with.
    pub const TUNNEL_TYPE: &'static str = "SplitWire";
    pub const DEFAULT_MTU: u16 = 1420;

    pub fn adapter_name(&self) -> String {
        format!("{}{}", Self::ADAPTER_PREFIX, self.name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_the_adapter_and_never_shows_the_keys() {
        let spec = TunnelSpec {
            name: "AcmeCorp".into(),
            adapter_guid: 1,
            address: Ipv4Addr::new(10, 88, 4, 12),
            mtu: TunnelSpec::DEFAULT_MTU,
            private_key: SecretKey::new([0xc3; 32]),
            peer_public_key: [1; 32],
            preshared_key: Some(SecretKey::new([0xd7; 32])),
            endpoint_host: "vpn.example.com".into(),
            endpoint: "198.51.100.7:51820".parse().unwrap(),
            keepalive: 25,
        };
        assert_eq!(spec.adapter_name(), "SplitWire-AcmeCorp");
        let shown = format!("{spec:?}");
        assert!(shown.contains("SecretKey(redacted)"));
        assert!(!shown.contains("195")); // 0xc3
        assert!(!shown.contains("215")); // 0xd7
    }
}
