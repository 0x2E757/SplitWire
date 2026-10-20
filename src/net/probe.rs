use std::io;
use std::net::Ipv4Addr;

use super::Ipv4Net;

/// A network directly attached to an interface that is not a SplitWire adapter (LAN, Hyper-V, WSL).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OnLinkNetwork {
    pub net: Ipv4Net,
    pub interface: String,
}

/// Read-only questions about the machine's interfaces; SplitWire's own adapters are left out.
pub trait NetworkProbe: Send + Sync {
    /// On-link IPv4 networks of every interface except SplitWire's adapters.
    fn on_link_networks(&self) -> io::Result<Vec<OnLinkNetwork>>;
    /// DNS servers of the interface holding the default route (never a SplitWire adapter).
    fn default_route_dns(&self) -> io::Result<Vec<Ipv4Addr>>;
}
