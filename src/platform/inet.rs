//! The socket address type IP Helper and wireguard-nt share (`SOCKADDR_INET`), IPv4 only.

use std::net::{Ipv4Addr, SocketAddrV4};

pub const AF_INET: u16 = 2;

/// C `SOCKADDR_INET`: the union of `SOCKADDR_IN` and `SOCKADDR_IN6`, read and written as
/// `SOCKADDR_IN` only. The port and address are in network byte order.
#[repr(C, align(4))]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct SockaddrInet {
    pub family: u16,
    pub port: [u8; 2],
    pub addr: [u8; 4],
    rest: [u8; 20],
}

impl SockaddrInet {
    pub const ZERO: Self = Self {
        family: 0,
        port: [0; 2],
        addr: [0; 4],
        rest: [0; 20],
    };

    pub fn v4(address: SocketAddrV4) -> Self {
        Self {
            family: AF_INET,
            port: address.port().to_be_bytes(),
            addr: address.ip().octets(),
            ..Self::ZERO
        }
    }

    /// `None` for any other family (an IPv6 endpoint, an unset one).
    pub fn to_v4(&self) -> Option<SocketAddrV4> {
        (self.family == AF_INET)
            .then(|| SocketAddrV4::new(Ipv4Addr::from(self.addr), u16::from_be_bytes(self.port)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::mem::{offset_of, size_of};

    #[test]
    fn layouts_match_the_sdk() {
        assert_eq!(size_of::<SockaddrInet>(), 28); // SOCKADDR_INET
        assert_eq!(offset_of!(SockaddrInet, port), 2); // SOCKADDR_IN.sin_port
        assert_eq!(offset_of!(SockaddrInet, addr), 4); // SOCKADDR_IN.sin_addr
    }

    #[test]
    fn ipv4_addresses_round_trip_in_network_order() {
        let address: SocketAddrV4 = "198.51.100.7:51820".parse().unwrap();
        let raw = SockaddrInet::v4(address);
        assert_eq!(raw.port, [0xca, 0x6c]);
        assert_eq!(raw.addr, [198, 51, 100, 7]);
        assert_eq!(raw.to_v4(), Some(address));
        assert_eq!(SockaddrInet::ZERO.to_v4(), None);
    }
}
