//! DNS over UDP on std sockets: `UdpDnsExchange` for the fallback servers, and
//! `BoundUdpExchange` for a profile's upstream, whose queries leave only through its adapter.
//! Each query gets a fresh socket, so concurrent queries never read each other's replies and
//! every query has its own source port. `BoundExchangeFactory` makes the bound one per adapter.

use std::io;
use std::net::{Ipv4Addr, SocketAddr, UdpSocket};
use std::os::windows::io::AsRawSocket;
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::dns::{DnsExchange, DnsExchangeFactory};

/// `impl DnsExchangeFactory`: a `BoundUdpExchange` on the tunnel address and adapter index.
pub struct BoundExchangeFactory {
    timeout: Duration,
}

impl BoundExchangeFactory {
    pub fn new(timeout: Duration) -> Self {
        Self { timeout }
    }
}

impl DnsExchangeFactory for BoundExchangeFactory {
    fn bound(&self, source: Ipv4Addr, if_index: u32) -> Arc<dyn DnsExchange> {
        Arc::new(BoundUdpExchange::new(source, if_index, self.timeout))
    }
}

const IPPROTO_IP: i32 = 0;
const IP_UNICAST_IF: i32 = 31;
const SOCKET_ERROR: i32 = -1;
const MAX_REPLY: usize = 65_535;

/// Plain UDP from any local address, as the routing table picks it.
pub struct UdpDnsExchange {
    timeout: Duration,
}

impl UdpDnsExchange {
    pub fn new(timeout: Duration) -> Self {
        Self { timeout }
    }
}

impl DnsExchange for UdpDnsExchange {
    fn exchange(&self, server: SocketAddr, query: &[u8]) -> io::Result<Vec<u8>> {
        let local: SocketAddr = match server {
            SocketAddr::V4(_) => (Ipv4Addr::UNSPECIFIED, 0).into(),
            SocketAddr::V6(_) => (std::net::Ipv6Addr::UNSPECIFIED, 0).into(),
        };
        ask(&UdpSocket::bind(local)?, server, query, self.timeout)
    }
}

/// UDP bound to a profile's tunnel address, with `IP_UNICAST_IF` pinning its adapter: the
/// strong host model and the interface pin both keep the query inside that tunnel.
/// A new adapter index needs a new exchange.
pub struct BoundUdpExchange {
    source: Ipv4Addr,
    if_index: u32,
    timeout: Duration,
}

impl BoundUdpExchange {
    pub fn new(source: Ipv4Addr, if_index: u32, timeout: Duration) -> Self {
        Self {
            source,
            if_index,
            timeout,
        }
    }

    fn socket(&self) -> io::Result<UdpSocket> {
        let socket = UdpSocket::bind((self.source, 0))?;
        // The option takes the index in network byte order.
        let value = self.if_index.to_be();
        let result = unsafe {
            setsockopt(
                socket.as_raw_socket() as usize,
                IPPROTO_IP,
                IP_UNICAST_IF,
                (&value as *const u32).cast(),
                size_of::<u32>() as i32,
            )
        };
        if result == SOCKET_ERROR {
            return Err(io::Error::from_raw_os_error(unsafe { WSAGetLastError() }));
        }
        Ok(socket)
    }
}

impl DnsExchange for BoundUdpExchange {
    fn exchange(&self, server: SocketAddr, query: &[u8]) -> io::Result<Vec<u8>> {
        if !server.is_ipv4() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("{server}: a tunnel's upstream is IPv4 only"),
            ));
        }
        ask(&self.socket()?, server, query, self.timeout)
    }
}

/// Sends `query` to `server` and waits for the reply carrying its ID; stray datagrams are
/// skipped until the time is up.
fn ask(
    socket: &UdpSocket,
    server: SocketAddr,
    query: &[u8],
    timeout: Duration,
) -> io::Result<Vec<u8>> {
    if query.len() < 2 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "query too short",
        ));
    }
    socket.connect(server)?;
    socket.send(query)?;
    let deadline = Instant::now() + timeout;
    let mut buffer = vec![0u8; MAX_REPLY];
    loop {
        let left = deadline.saturating_duration_since(Instant::now());
        if left.is_zero() {
            return Err(io::Error::new(io::ErrorKind::TimedOut, "no answer in time"));
        }
        socket.set_read_timeout(Some(left))?;
        let len = match socket.recv(&mut buffer) {
            Ok(len) => len,
            Err(e)
                if matches!(
                    e.kind(),
                    io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
                ) =>
            {
                return Err(io::Error::new(io::ErrorKind::TimedOut, "no answer in time"));
            }
            Err(e) => return Err(e),
        };
        if len >= 12 && buffer[..2] == query[..2] {
            buffer.truncate(len);
            return Ok(buffer);
        }
    }
}

#[link(name = "ws2_32")]
unsafe extern "system" {
    fn setsockopt(socket: usize, level: i32, name: i32, value: *const u8, len: i32) -> i32;
    fn WSAGetLastError() -> i32;
    #[cfg(test)]
    fn getsockopt(socket: usize, level: i32, name: i32, value: *mut u8, len: *mut i32) -> i32;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dns::ResponseBuilder;
    use crate::dns::wire::{DnsQuery, DnsResponse, TYPE_A};
    use std::thread;

    const LOOPBACK_INDEX: u32 = 1;

    /// A local responder on loopback: first a stray datagram with another ID, then the answer.
    /// Returns its address and the source address the query came from.
    fn responder() -> (SocketAddr, thread::JoinHandle<SocketAddr>) {
        let socket = UdpSocket::bind("127.0.0.1:0").unwrap();
        let address = socket.local_addr().unwrap();
        let handle = thread::spawn(move || {
            let mut buffer = [0u8; 512];
            let (len, from) = socket.recv_from(&mut buffer).unwrap();
            let query = DnsQuery::parse(&buffer[..len]).unwrap();
            let mut stray = query.reply(0);
            stray[0] ^= 0xff;
            socket.send_to(&stray, from).unwrap();
            let reply = ResponseBuilder::new(&query)
                .a(&query.question.name, Ipv4Addr::new(10, 1, 2, 3), 60)
                .build();
            socket.send_to(&reply, from).unwrap();
            from
        });
        (address, handle)
    }

    #[test]
    fn a_bound_exchange_asks_from_its_address_and_skips_stray_replies() {
        let (server, handle) = responder();
        let exchange =
            BoundUdpExchange::new(Ipv4Addr::LOCALHOST, LOOPBACK_INDEX, Duration::from_secs(2));
        let query = DnsQuery::build(0x5157, "intranet.example.com", TYPE_A);
        let reply = DnsResponse::parse(exchange.exchange(server, &query).unwrap()).unwrap();
        assert_eq!(reply.id(), 0x5157);
        assert_eq!(
            reply.ipv4_chain("intranet.example.com"),
            [Ipv4Addr::new(10, 1, 2, 3)]
        );
        assert_eq!(handle.join().unwrap().ip(), Ipv4Addr::LOCALHOST);
    }

    #[test]
    fn the_bound_socket_is_pinned_to_its_interface() {
        let exchange = BoundUdpExchange::new(Ipv4Addr::LOCALHOST, LOOPBACK_INDEX, Duration::ZERO);
        let socket = exchange.socket().unwrap();
        assert_eq!(socket.local_addr().unwrap().ip(), Ipv4Addr::LOCALHOST);
        let mut value = 0u32;
        let mut len = size_of::<u32>() as i32;
        let result = unsafe {
            getsockopt(
                socket.as_raw_socket() as usize,
                IPPROTO_IP,
                IP_UNICAST_IF,
                (&mut value as *mut u32).cast(),
                &mut len,
            )
        };
        assert_eq!(result, 0);
        // Set in network byte order, reported back in host order: Windows read it as index 1.
        assert_eq!(value, LOOPBACK_INDEX);
    }

    #[test]
    fn a_bound_exchange_refuses_ipv6_and_times_out_on_silence() {
        let exchange = BoundUdpExchange::new(
            Ipv4Addr::LOCALHOST,
            LOOPBACK_INDEX,
            Duration::from_millis(200),
        );
        let query = DnsQuery::build(1, "example.com", TYPE_A);
        let ipv6 = exchange.exchange("[::1]:53".parse().unwrap(), &query);
        assert_eq!(ipv6.unwrap_err().kind(), io::ErrorKind::InvalidInput);
        let silent = UdpSocket::bind("127.0.0.1:0").unwrap();
        let error = exchange
            .exchange(silent.local_addr().unwrap(), &query)
            .unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::TimedOut);
    }

    #[test]
    fn the_plain_exchange_asks_over_loopback() {
        let (server, handle) = responder();
        let exchange = UdpDnsExchange::new(Duration::from_secs(2));
        let query = DnsQuery::build(9, "example.com", TYPE_A);
        let reply = DnsResponse::parse(exchange.exchange(server, &query).unwrap()).unwrap();
        assert_eq!(
            reply.ipv4_chain("example.com"),
            [Ipv4Addr::new(10, 1, 2, 3)]
        );
        handle.join().unwrap();
    }
}
