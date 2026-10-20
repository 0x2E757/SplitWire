use std::io;
use std::mem::{align_of, size_of};
use std::net::SocketAddrV4;
use std::sync::atomic::{Ordering, compiler_fence};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use super::layout::*;
use crate::platform::inet::{AF_INET, SockaddrInet};
use crate::tunnel::{DriverConfig, TunnelHealth, TunnelSpec};

const INTERFACE: usize = size_of::<WireGuardInterface>();
const PEER: usize = size_of::<WireGuardPeer>();
const ALLOWED_IP: usize = size_of::<WireGuardAllowedIp>();
/// 100 ns intervals from 1601-01-01 to 1970-01-01.
const FILETIME_UNIX_EPOCH: u64 = 116_444_736_000_000_000;

/// The bytes `WireGuardSetConfiguration` reads and `WireGuardGetConfiguration` writes: 8-byte
/// aligned, as the structures require, and zeroed on drop because they hold the private key.
pub struct ConfigBuffer {
    words: Vec<u64>,
    len: usize,
}

impl ConfigBuffer {
    /// An empty buffer of `len` bytes for the driver to fill.
    pub fn zeroed(len: usize) -> Self {
        Self {
            words: vec![0; len.div_ceil(8)],
            len,
        }
    }

    pub fn for_config(config: &DriverConfig) -> Self {
        match config {
            DriverConfig::Full(spec) => Self::full(spec),
            DriverConfig::Endpoint {
                peer_public_key,
                endpoint,
            } => Self::endpoint(peer_public_key, *endpoint),
        }
    }

    /// The interface with its private key, replacing every peer by the one peer with its keys,
    /// endpoint, keepalive and the allowed IP `0.0.0.0/0`. The listen port is left to the driver.
    fn full(spec: &TunnelSpec) -> Self {
        let mut buffer = Self::zeroed(INTERFACE + PEER + ALLOWED_IP);
        let interface = buffer.at_mut::<WireGuardInterface>(0);
        interface.flags = INTERFACE_HAS_PRIVATE_KEY | INTERFACE_REPLACE_PEERS;
        interface
            .private_key
            .copy_from_slice(spec.private_key.expose());
        interface.peers_count = 1;
        let peer = buffer.at_mut::<WireGuardPeer>(INTERFACE);
        peer.flags = PEER_HAS_PUBLIC_KEY
            | PEER_HAS_ENDPOINT
            | PEER_HAS_PERSISTENT_KEEPALIVE
            | PEER_REPLACE_ALLOWED_IPS;
        peer.public_key = spec.peer_public_key;
        if let Some(preshared) = &spec.preshared_key {
            peer.flags |= PEER_HAS_PRESHARED_KEY;
            peer.preshared_key.copy_from_slice(preshared.expose());
        }
        peer.persistent_keepalive = spec.keepalive;
        peer.endpoint = SockaddrInet::v4(spec.endpoint);
        peer.allowed_ips_count = 1;
        // Zero address and prefix: 0.0.0.0/0, for cryptokey routing only.
        buffer
            .at_mut::<WireGuardAllowedIp>(INTERFACE + PEER)
            .address_family = AF_INET;
        buffer
    }

    /// Only the existing peer's endpoint: `UPDATE_ONLY` never adds a peer, and the interface
    /// flags are empty, so its keys and the other peer fields stay as they are.
    fn endpoint(peer_public_key: &[u8; KEY_LENGTH], endpoint: SocketAddrV4) -> Self {
        let mut buffer = Self::zeroed(INTERFACE + PEER);
        buffer.at_mut::<WireGuardInterface>(0).peers_count = 1;
        let peer = buffer.at_mut::<WireGuardPeer>(INTERFACE);
        peer.flags = PEER_HAS_PUBLIC_KEY | PEER_HAS_ENDPOINT | PEER_UPDATE_ONLY;
        peer.public_key = *peer_public_key;
        peer.endpoint = SockaddrInet::v4(endpoint);
        buffer
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn as_ptr(&self) -> *const WireGuardInterface {
        self.words.as_ptr().cast()
    }

    pub fn as_mut_ptr(&mut self) -> *mut WireGuardInterface {
        self.words.as_mut_ptr().cast()
    }

    /// Reads what `WireGuardGetConfiguration` wrote into the first `filled` bytes: the interface
    /// public key and the first peer's handshake, traffic and endpoint.
    pub fn health(&self, filled: usize) -> io::Result<TunnelHealth> {
        let filled = filled.min(self.len);
        if filled < INTERFACE {
            return Err(truncated());
        }
        let interface = self.at::<WireGuardInterface>(0);
        let mut health = TunnelHealth {
            interface_public_key: interface.public_key,
            last_handshake: None,
            rx_bytes: 0,
            tx_bytes: 0,
            endpoint: None,
        };
        if interface.peers_count == 0 {
            return Ok(health);
        }
        if filled < INTERFACE + PEER {
            return Err(truncated());
        }
        let peer = self.at::<WireGuardPeer>(INTERFACE);
        health.last_handshake = handshake_time(peer.last_handshake);
        health.rx_bytes = peer.rx_bytes;
        health.tx_bytes = peer.tx_bytes;
        health.endpoint = peer.endpoint.to_v4();
        Ok(health)
    }

    #[cfg(test)]
    fn as_bytes(&self) -> &[u8] {
        unsafe { std::slice::from_raw_parts(self.words.as_ptr().cast(), self.len) }
    }

    fn wipe(&mut self) {
        for word in &mut self.words {
            // Volatile, so the optimizer cannot drop the write to memory that is about to be freed.
            unsafe { std::ptr::write_volatile(word, 0) };
        }
        compiler_fence(Ordering::SeqCst);
    }

    fn check<T>(&self, offset: usize) {
        assert!(offset.is_multiple_of(align_of::<T>()) && offset + size_of::<T>() <= self.len);
    }

    // The structures hold only integers and byte arrays, so every bit pattern is a valid value.
    fn at<T>(&self, offset: usize) -> &T {
        self.check::<T>(offset);
        unsafe { &*self.words.as_ptr().cast::<u8>().add(offset).cast::<T>() }
    }

    fn at_mut<T>(&mut self, offset: usize) -> &mut T {
        self.check::<T>(offset);
        unsafe { &mut *self.words.as_mut_ptr().cast::<u8>().add(offset).cast::<T>() }
    }
}

impl Drop for ConfigBuffer {
    fn drop(&mut self) {
        self.wipe();
    }
}

fn truncated() -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidData,
        "the driver returned a truncated configuration",
    )
}

/// `None` for 0 (no handshake yet) and for times before 1970.
fn handshake_time(filetime: u64) -> Option<SystemTime> {
    let since_unix = filetime.checked_sub(FILETIME_UNIX_EPOCH)?;
    UNIX_EPOCH.checked_add(Duration::from_nanos(since_unix).checked_mul(100)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profile::SecretKey;

    fn spec(preshared: bool) -> TunnelSpec {
        TunnelSpec {
            name: "AcmeCorp".into(),
            adapter_guid: 7,
            address: "10.88.4.12".parse().unwrap(),
            mtu: 1420,
            private_key: SecretKey::new([0xc3; 32]),
            peer_public_key: [0x11; 32],
            preshared_key: preshared.then(|| SecretKey::new([0xd7; 32])),
            endpoint_host: "vpn.example.com".into(),
            endpoint: "198.51.100.7:51820".parse().unwrap(),
            keepalive: 25,
        }
    }

    fn u32_at(bytes: &[u8], at: usize) -> u32 {
        u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap())
    }

    #[test]
    fn a_full_config_is_one_interface_one_peer_and_all_ipv4_allowed() {
        let spec = spec(true);
        let buffer = ConfigBuffer::for_config(&DriverConfig::Full(&spec));
        let bytes = buffer.as_bytes();
        assert_eq!(bytes.len(), 80 + 136 + 24);
        assert_eq!(buffer.as_ptr() as usize % 8, 0);
        assert_eq!(
            u32_at(bytes, 0),
            INTERFACE_HAS_PRIVATE_KEY | INTERFACE_REPLACE_PEERS
        );
        assert_eq!(&bytes[4..6], [0, 0]); // listen port: the driver picks one
        assert_eq!(&bytes[6..38], [0xc3; 32]);
        assert_eq!(u32_at(bytes, 72), 1);
        let peer = &bytes[80..216];
        assert_eq!(
            u32_at(peer, 0),
            PEER_HAS_PUBLIC_KEY
                | PEER_HAS_ENDPOINT
                | PEER_HAS_PERSISTENT_KEEPALIVE
                | PEER_REPLACE_ALLOWED_IPS
                | PEER_HAS_PRESHARED_KEY
        );
        assert_eq!(&peer[8..40], [0x11; 32]);
        assert_eq!(&peer[40..72], [0xd7; 32]);
        assert_eq!(&peer[72..74], 25u16.to_le_bytes());
        assert_eq!(&peer[76..84], [2, 0, 0xca, 0x6c, 198, 51, 100, 7]);
        assert_eq!(u32_at(peer, 128), 1);
        let allowed = &bytes[216..];
        assert_eq!(&allowed[..16], [0; 16]);
        assert_eq!(&allowed[16..18], AF_INET.to_le_bytes());
        assert_eq!(allowed[18], 0);
    }

    #[test]
    fn without_a_preshared_key_the_field_stays_empty_and_unflagged() {
        let spec = spec(false);
        let buffer = ConfigBuffer::for_config(&DriverConfig::Full(&spec));
        let peer = &buffer.as_bytes()[80..216];
        assert_eq!(u32_at(peer, 0) & PEER_HAS_PRESHARED_KEY, 0);
        assert_eq!(&peer[40..72], [0; 32]);
    }

    #[test]
    fn an_endpoint_update_touches_only_the_existing_peer() {
        let buffer = ConfigBuffer::for_config(&DriverConfig::Endpoint {
            peer_public_key: [0x11; 32],
            endpoint: "203.0.113.9:51820".parse().unwrap(),
        });
        let bytes = buffer.as_bytes();
        assert_eq!(bytes.len(), 80 + 136);
        assert_eq!(u32_at(bytes, 0), 0);
        assert_eq!(&bytes[6..38], [0; 32]);
        assert_eq!(u32_at(bytes, 72), 1);
        let peer = &bytes[80..];
        assert_eq!(
            u32_at(peer, 0),
            PEER_HAS_PUBLIC_KEY | PEER_HAS_ENDPOINT | PEER_UPDATE_ONLY
        );
        assert_eq!(&peer[76..84], [2, 0, 0xca, 0x6c, 203, 0, 113, 9]);
        assert_eq!(u32_at(peer, 128), 0);
    }

    #[test]
    fn reads_the_health_the_driver_wrote() {
        let mut buffer = ConfigBuffer::zeroed(80 + 136 + 24);
        buffer.at_mut::<WireGuardInterface>(0).public_key = [0x42; 32];
        buffer.at_mut::<WireGuardInterface>(0).peers_count = 1;
        let peer = buffer.at_mut::<WireGuardPeer>(80);
        peer.rx_bytes = 1_000;
        peer.tx_bytes = 2_000;
        peer.last_handshake = FILETIME_UNIX_EPOCH + 1_792_423_315 * 10_000_000;
        peer.endpoint = SockaddrInet::v4("198.51.100.7:51820".parse().unwrap());
        let health = buffer.health(240).unwrap();
        assert_eq!(health.interface_public_key, [0x42; 32]);
        assert_eq!((health.rx_bytes, health.tx_bytes), (1_000, 2_000));
        assert_eq!(
            health.last_handshake,
            Some(UNIX_EPOCH + Duration::from_secs(1_792_423_315))
        );
        assert_eq!(health.endpoint, Some("198.51.100.7:51820".parse().unwrap()));
        assert!(buffer.health(100).is_err());
        assert!(buffer.health(10).is_err());
    }

    #[test]
    fn no_handshake_and_no_peer_read_as_none() {
        assert_eq!(handshake_time(0), None);
        assert_eq!(handshake_time(5), None);
        let buffer = ConfigBuffer::zeroed(80);
        let health = buffer.health(80).unwrap();
        assert_eq!((health.last_handshake, health.endpoint), (None, None));
    }

    #[test]
    fn wiping_zeroes_every_byte() {
        let spec = spec(true);
        let mut buffer = ConfigBuffer::for_config(&DriverConfig::Full(&spec));
        buffer.wipe();
        assert!(buffer.as_bytes().iter().all(|byte| *byte == 0));
    }
}
