use std::io;
use std::net::{Ipv4Addr, SocketAddr};
use std::sync::Arc;

/// Sends one DNS query to one server and waits for its reply (the network boundary).
pub trait DnsExchange: Send + Sync {
    fn exchange(&self, server: SocketAddr, query: &[u8]) -> io::Result<Vec<u8>>;
}

/// Makes the exchange of a profile's upstream DNS: its queries leave from the tunnel address
/// through the adapter `if_index` only. A new adapter index needs a new exchange.
pub trait DnsExchangeFactory: Send + Sync {
    fn bound(&self, source: Ipv4Addr, if_index: u32) -> Arc<dyn DnsExchange>;
}

/// Asks Windows' DNS client to forget its cached answers.
pub trait DnsCacheFlusher: Send + Sync {
    fn flush(&self) -> io::Result<()>;
}
