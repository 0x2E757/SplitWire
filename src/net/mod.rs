//! IPv4 network math and the read-only view of the machine's interfaces.

mod ipv4_net;
mod probe;

pub use ipv4_net::Ipv4Net;
pub use probe::{NetworkProbe, OnLinkNetwork};
