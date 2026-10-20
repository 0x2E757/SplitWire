use std::error::Error;
use std::fmt;
use std::io;
use std::net::SocketAddrV4;
use std::time::SystemTime;

use super::{DriverVersion, TunnelSpec};

/// The adapter a profile's tunnel runs on: its routes go on `luid`, its bound DNS socket uses
/// `if_index`. Both change when the adapter is recreated.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TunnelLink {
    pub luid: u64,
    pub if_index: u32,
}

/// What the tunnel reports about its peer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TunnelHealth {
    pub interface_public_key: [u8; 32],
    /// `None` until the first handshake.
    pub last_handshake: Option<SystemTime>,
    pub rx_bytes: u64,
    pub tx_bytes: u64,
    /// The endpoint in use.
    pub endpoint: Option<SocketAddrV4>,
}

/// The part of `Tunnel::up` that failed: the adapter (create, address, MTU, DNS settings) or its
/// peer (configuration, state up). Connect reports it as step 4 or step 5.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UpStage {
    Adapter,
    Peer,
}

impl UpStage {
    /// `error`, marked with this stage; its kind and message stay the same.
    pub fn wrap(self, error: io::Error) -> io::Error {
        if Self::of(&error).is_some() {
            return error;
        }
        io::Error::new(
            error.kind(),
            StagedError {
                stage: self,
                source: error,
            },
        )
    }

    /// The stage an `up` error was marked with.
    pub fn of(error: &io::Error) -> Option<UpStage> {
        error
            .get_ref()
            .and_then(|inner| inner.downcast_ref::<StagedError>())
            .map(|staged| staged.stage)
    }
}

#[derive(Debug)]
struct StagedError {
    stage: UpStage,
    source: io::Error,
}

impl fmt::Display for StagedError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.source.fmt(f)
    }
}

impl Error for StagedError {}

/// A tunnel kind, keyed by profile name. WireGuard is the one kind today; nothing above this trait
/// knows which kind it is.
pub trait Tunnel: Send + Sync {
    /// Creates the profile's adapter, gives it its address and MTU, configures the peer and brings
    /// it up. Fails when an adapter of that name already exists (another process holds it). The
    /// error is marked with its `UpStage`.
    fn up(&self, spec: &TunnelSpec) -> io::Result<TunnelLink>;
    /// Takes the adapter down and closes it, which removes it with its routes and sockets.
    fn down(&self, name: &str) -> io::Result<()>;
    fn health(&self, name: &str) -> io::Result<TunnelHealth>;
    /// Makes the adapter match `spec` again: recreates it when it is gone, restores its state,
    /// address and MTU, and updates a changed endpoint. The link differs after a recreate.
    fn refresh(&self, spec: &TunnelSpec) -> io::Result<TunnelLink>;
    /// The version of the running driver, once the first Connect loaded it.
    fn driver_version(&self) -> Option<DriverVersion>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_stage_survives_in_the_error_and_keeps_its_message() {
        let error = UpStage::Peer.wrap(io::Error::other("refused"));
        assert_eq!(UpStage::of(&error), Some(UpStage::Peer));
        assert_eq!(error.to_string(), "refused");
        assert_eq!(error.kind(), io::ErrorKind::Other);
        let again = UpStage::Adapter.wrap(error);
        assert_eq!(UpStage::of(&again), Some(UpStage::Peer));
        assert_eq!(UpStage::of(&io::Error::other("plain")), None);
    }
}
