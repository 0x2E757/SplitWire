//! The `wireguard.h` structures and flags, laid out as the DLL reads and writes them. A
//! configuration is one interface, then each peer followed by its allowed IPs.

use crate::platform::inet::SockaddrInet;

pub const KEY_LENGTH: usize = 32;

pub const INTERFACE_HAS_PRIVATE_KEY: u32 = 1 << 1;
pub const INTERFACE_REPLACE_PEERS: u32 = 1 << 3;

pub const PEER_HAS_PUBLIC_KEY: u32 = 1 << 0;
pub const PEER_HAS_PRESHARED_KEY: u32 = 1 << 1;
pub const PEER_HAS_PERSISTENT_KEEPALIVE: u32 = 1 << 2;
pub const PEER_HAS_ENDPOINT: u32 = 1 << 3;
pub const PEER_REPLACE_ALLOWED_IPS: u32 = 1 << 5;
pub const PEER_UPDATE_ONLY: u32 = 1 << 7;

/// `WIREGUARD_ADAPTER_STATE`.
pub const ADAPTER_STATE_DOWN: i32 = 0;
pub const ADAPTER_STATE_UP: i32 = 1;
/// `WIREGUARD_ADAPTER_LOG_STATE`.
pub const ADAPTER_LOG_OFF: i32 = 0;
pub const ADAPTER_LOG_ON: i32 = 1;
/// `WIREGUARD_LOGGER_LEVEL`.
pub const LOG_INFO: i32 = 0;
pub const LOG_WARN: i32 = 1;
pub const LOG_ERR: i32 = 2;

/// C `WIREGUARD_INTERFACE`.
#[repr(C, align(8))]
pub struct WireGuardInterface {
    pub flags: u32,
    pub listen_port: u16,
    pub private_key: [u8; KEY_LENGTH],
    pub public_key: [u8; KEY_LENGTH],
    pub peers_count: u32,
}

/// C `WIREGUARD_PEER`. `last_handshake` counts 100 ns intervals since 1601-01-01 UTC, 0 for none.
#[repr(C, align(8))]
pub struct WireGuardPeer {
    pub flags: u32,
    pub reserved: u32,
    pub public_key: [u8; KEY_LENGTH],
    pub preshared_key: [u8; KEY_LENGTH],
    pub persistent_keepalive: u16,
    pub endpoint: SockaddrInet,
    pub tx_bytes: u64,
    pub rx_bytes: u64,
    pub last_handshake: u64,
    pub allowed_ips_count: u32,
}

/// C `WIREGUARD_ALLOWED_IP`; the address is the `IN_ADDR`/`IN6_ADDR` union.
#[repr(C, align(8))]
pub struct WireGuardAllowedIp {
    pub address: [u8; 16],
    pub address_family: u16,
    pub cidr: u8,
    pub flags: u32,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::mem::{offset_of, size_of};

    #[test]
    fn layouts_match_wireguard_h() {
        assert_eq!(size_of::<WireGuardInterface>(), 80); // WIREGUARD_INTERFACE
        assert_eq!(offset_of!(WireGuardInterface, listen_port), 4); // WIREGUARD_INTERFACE.ListenPort
        assert_eq!(offset_of!(WireGuardInterface, private_key), 6); // WIREGUARD_INTERFACE.PrivateKey
        assert_eq!(offset_of!(WireGuardInterface, public_key), 38); // WIREGUARD_INTERFACE.PublicKey
        assert_eq!(offset_of!(WireGuardInterface, peers_count), 72); // WIREGUARD_INTERFACE.PeersCount
        assert_eq!(size_of::<WireGuardPeer>(), 136); // WIREGUARD_PEER
        assert_eq!(offset_of!(WireGuardPeer, public_key), 8); // WIREGUARD_PEER.PublicKey
        assert_eq!(offset_of!(WireGuardPeer, preshared_key), 40); // WIREGUARD_PEER.PresharedKey
        assert_eq!(offset_of!(WireGuardPeer, persistent_keepalive), 72); // WIREGUARD_PEER.PersistentKeepalive
        assert_eq!(offset_of!(WireGuardPeer, endpoint), 76); // WIREGUARD_PEER.Endpoint
        assert_eq!(offset_of!(WireGuardPeer, tx_bytes), 104); // WIREGUARD_PEER.TxBytes
        assert_eq!(offset_of!(WireGuardPeer, rx_bytes), 112); // WIREGUARD_PEER.RxBytes
        assert_eq!(offset_of!(WireGuardPeer, last_handshake), 120); // WIREGUARD_PEER.LastHandshake
        assert_eq!(offset_of!(WireGuardPeer, allowed_ips_count), 128); // WIREGUARD_PEER.AllowedIPsCount
        assert_eq!(size_of::<WireGuardAllowedIp>(), 24); // WIREGUARD_ALLOWED_IP
        assert_eq!(offset_of!(WireGuardAllowedIp, address_family), 16); // WIREGUARD_ALLOWED_IP.AddressFamily
        assert_eq!(offset_of!(WireGuardAllowedIp, cidr), 18); // WIREGUARD_ALLOWED_IP.Cidr
        assert_eq!(offset_of!(WireGuardAllowedIp, flags), 20); // WIREGUARD_ALLOWED_IP.Flags
    }
}
