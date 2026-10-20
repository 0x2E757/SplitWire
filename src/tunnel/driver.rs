use std::fmt;
use std::io;
use std::net::SocketAddrV4;

use super::{TunnelHealth, TunnelSpec};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AdapterState {
    Down,
    Up,
}

/// One configuration write (`WireGuardSetConfiguration`).
#[derive(Debug)]
pub enum DriverConfig<'a> {
    /// The whole interface: the private key, `REPLACE_PEERS`, listen port 0, and the one peer with
    /// its keys, endpoint, keepalive and the allowed IP `0.0.0.0/0` (cryptokey routing only).
    Full(&'a TunnelSpec),
    /// Only the peer's endpoint (`WIREGUARD_PEER_UPDATE_ONLY`), after the endpoint re-resolved.
    Endpoint {
        peer_public_key: [u8; 32],
        endpoint: SocketAddrV4,
    },
}

/// The version of the loaded driver (`WireGuardGetRunningDriverVersion`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DriverVersion {
    pub major: u16,
    pub minor: u16,
}

impl DriverVersion {
    /// The driver's DWORD: the major version in the high word, the minor in the low one.
    pub fn from_dword(value: u32) -> Self {
        Self {
            major: (value >> 16) as u16,
            minor: value as u16,
        }
    }
}

impl fmt::Display for DriverVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}", self.major, self.minor)
    }
}

/// The wireguard-nt kernel driver, one level above the DLL's ABI. Tests use a fake; a dry run
/// uses a fake that only logs.
pub trait WireGuardDriver: Send + Sync {
    /// `WireGuardCreateAdapter`; the first create installs the signed driver the DLL carries.
    fn create(
        &self,
        name: &str,
        tunnel_type: &str,
        guid: u128,
    ) -> io::Result<Box<dyn WireGuardAdapter>>;
    /// `WireGuardOpenAdapter`: `Some` when an adapter of that name exists.
    fn open(&self, name: &str) -> io::Result<Option<Box<dyn WireGuardAdapter>>>;
    /// `None` while no driver is loaded.
    fn running_version(&self) -> io::Result<Option<DriverVersion>>;
    /// `WireGuardDeleteDriver` (uninstall only; fails harmlessly while any adapter exists).
    fn delete_driver(&self) -> io::Result<()>;
}

/// One open adapter. Dropping it closes the handle, which removes an adapter this process created.
pub trait WireGuardAdapter: Send {
    fn luid(&self) -> u64;
    fn set_config(&self, config: &DriverConfig) -> io::Result<()>;
    /// `WireGuardGetConfiguration`: the interface key and the peer's handshake, traffic and endpoint.
    fn config(&self) -> io::Result<TunnelHealth>;
    fn set_state(&self, state: AdapterState) -> io::Result<()>;
    fn state(&self) -> io::Result<AdapterState>;
    fn set_logging(&self, on: bool) -> io::Result<()>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_driver_version_dword() {
        let version = DriverVersion::from_dword(0x0001_0001);
        assert_eq!(version, DriverVersion { major: 1, minor: 1 });
        assert_eq!(version.to_string(), "1.1");
    }

    #[test]
    fn the_boundaries_are_object_safe() {
        let tunnel: Option<Box<dyn super::super::Tunnel>> = None;
        let driver: Option<Box<dyn WireGuardDriver>> = None;
        assert!(tunnel.is_none() && driver.is_none());
    }
}
