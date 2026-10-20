//! The tunnel boundary: what a tunnel kind gets (`TunnelSpec`), what it reports (`TunnelLink`,
//! `TunnelHealth`), the kind itself (`Tunnel`), the wireguard-nt driver one level above its ABI
//! (`WireGuardDriver`) and the adapter's IP configuration (`AdapterAddressing`). Also the rules
//! every tunnel follows: handshake health (`HandshakePolicy`), the wait for the first handshake
//! (`HandshakeWait`), endpoints resolved through the fallback DNS only (`EndpointResolver`,
//! `EndpointAddresses`), and the ten Connect steps with their outcomes (`ConnectStep`,
//! `ConnectAttempt`). Implementations live elsewhere:
//! `WireGuardTunnel` over a driver, the driver over `wireguard.dll`, IP Helper, and fakes.

mod addressing;
mod connect_step;
mod driver;
mod endpoint;
mod handshake;
mod handshake_wait;
mod link;
mod spec;

pub use addressing::{AdapterAddressing, AdapterSettings};
pub use connect_step::{AttemptKind, ConnectAttempt, ConnectStep, StepOutcome, StepResult};
pub use driver::{AdapterState, DriverConfig, DriverVersion, WireGuardAdapter, WireGuardDriver};
pub use endpoint::{EndpointAddresses, EndpointResolver};
pub use handshake::{HandshakeHealth, HandshakePolicy};
pub use handshake_wait::HandshakeWait;
pub use link::{Tunnel, TunnelHealth, TunnelLink, UpStage};
pub use spec::TunnelSpec;

#[cfg(test)]
pub(crate) use endpoint::tests::FakeDns;
