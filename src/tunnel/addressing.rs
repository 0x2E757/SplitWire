use std::io;
use std::net::Ipv4Addr;

/// What a tunnel adapter gets from the OS besides its peer: the tunnel address as a /32 and the
/// MTU. The rest is fixed (a high interface metric, no DNS servers, no DNS registration).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AdapterSettings {
    pub address: Ipv4Addr,
    pub mtu: u16,
}

/// The adapter's IP configuration (IP Helper). Tests use a fake; a dry run uses one that logs.
pub trait AdapterAddressing: Send + Sync {
    /// Gives the adapter `luid` its /32 address, its MTU with the fixed metric, no DNS servers
    /// and no DNS registration. Repeating it is harmless.
    fn apply(&self, luid: u64, settings: &AdapterSettings) -> io::Result<()>;
    /// True while the adapter still has the /32 address and the MTU.
    fn applied(&self, luid: u64, settings: &AdapterSettings) -> io::Result<bool>;
    /// The adapter's interface index; fails once the adapter is gone.
    fn index(&self, luid: u64) -> io::Result<u32>;
}
