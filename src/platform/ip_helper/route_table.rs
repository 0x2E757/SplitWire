use std::io;
use std::net::{Ipv4Addr, SocketAddrV4};

use super::abi::*;
use crate::net::Ipv4Net;
use crate::platform::inet::{AF_INET, SockaddrInet};
use crate::routes::{RouteGuard, RouteTable};

/// `impl RouteTable` over IP Helper. SplitWire's routes are the static (`NETMGMT`) IPv4 routes
/// with metric 7 on a profile's adapter, on-link; nothing else is listed, and only such a route
/// is ever deleted.
pub struct IpHelperRouteTable;

impl IpHelperRouteTable {
    /// Added to the adapter's interface metric; it also marks SplitWire's own routes.
    pub const METRIC: u32 = 7;

    /// The last check before a route is written, after `RouteGuard`'s: never wider than /8, so
    /// never the default route.
    pub fn admit(net: &Ipv4Net) -> io::Result<()> {
        if net.prefix() < RouteGuard::MIN_PREFIX {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("refused {net}: wider than /{}", RouteGuard::MIN_PREFIX),
            ));
        }
        Ok(())
    }

    /// The row for `net` on the adapter `luid`: on-link (next hop 0.0.0.0), static, metric 7,
    /// with the default (infinite) lifetimes.
    fn row(luid: u64, net: &Ipv4Net) -> MibIpforwardRow2 {
        let mut row = zeroed::<MibIpforwardRow2>();
        unsafe { InitializeIpForwardEntry(&mut row) };
        row.interface_luid = luid;
        row.destination_prefix = IpAddressPrefix {
            prefix: SockaddrInet::v4(SocketAddrV4::new(net.addr(), 0)),
            prefix_length: net.prefix(),
        };
        row.next_hop = SockaddrInet::v4(SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, 0));
        row.metric = Self::METRIC;
        row.protocol = ROUTE_PROTOCOL_NETMGMT;
        row
    }

    /// The network of `row` when it is one of SplitWire's routes on `luid`.
    fn owned(row: &MibIpforwardRow2, luid: u64) -> Option<Ipv4Net> {
        let destination = &row.destination_prefix;
        let ours = row.interface_luid == luid
            && row.protocol == ROUTE_PROTOCOL_NETMGMT
            && row.metric == Self::METRIC
            && destination.prefix.family == AF_INET;
        ours.then(|| {
            Ipv4Net::new(
                Ipv4Addr::from(destination.prefix.addr),
                destination.prefix_length,
            )
        })
        .flatten()
    }
}

impl RouteTable for IpHelperRouteTable {
    fn list_owned(&self, luid: u64) -> io::Result<Vec<Ipv4Net>> {
        let table = ForwardTable::ipv4()?;
        Ok(table
            .rows()
            .iter()
            .filter_map(|row| Self::owned(row, luid))
            .collect())
    }

    fn add(&self, luid: u64, net: Ipv4Net) -> io::Result<()> {
        Self::admit(&net)?;
        match unsafe { CreateIpForwardEntry2(&Self::row(luid, &net)) } {
            ERROR_OBJECT_ALREADY_EXISTS => Ok(()),
            code => win32(code),
        }
    }

    /// Deletes the exact row the table lists for `net`, and only when it is SplitWire's.
    fn remove(&self, luid: u64, net: Ipv4Net) -> io::Result<()> {
        let table = ForwardTable::ipv4()?;
        let Some(row) = table
            .rows()
            .iter()
            .find(|row| Self::owned(row, luid) == Some(net))
        else {
            return Ok(());
        };
        match unsafe { DeleteIpForwardEntry2(row) } {
            ERROR_NOT_FOUND => Ok(()),
            code => win32(code),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::platform::ip_helper::addressing::tests::LOOPBACK_LUID;

    fn net(text: &str) -> Ipv4Net {
        text.parse().unwrap()
    }

    #[test]
    fn admits_nothing_wider_than_a_slash_8() {
        assert!(IpHelperRouteTable::admit(&net("10.0.0.0/8")).is_ok());
        assert!(IpHelperRouteTable::admit(&net("192.0.2.93/32")).is_ok());
        for wide in ["0.0.0.0/0", "0.0.0.0/1", "128.0.0.0/1", "10.0.0.0/7"] {
            let error = IpHelperRouteTable::admit(&net(wide)).unwrap_err();
            assert_eq!(error.kind(), io::ErrorKind::InvalidInput, "{wide}");
        }
    }

    #[test]
    fn builds_an_on_link_static_row_without_adding_it() {
        let row = IpHelperRouteTable::row(0x1234, &net("10.66.0.0/16"));
        assert_eq!(row.interface_luid, 0x1234);
        assert_eq!(row.destination_prefix.prefix.family, AF_INET);
        assert_eq!(row.destination_prefix.prefix.addr, [10, 66, 0, 0]);
        assert_eq!(row.destination_prefix.prefix_length, 16);
        assert_eq!(row.next_hop.to_v4(), Some("0.0.0.0:0".parse().unwrap()));
        assert_eq!((row.metric, row.protocol), (7, ROUTE_PROTOCOL_NETMGMT));
        assert_eq!(
            (row.valid_lifetime, row.preferred_lifetime),
            (u32::MAX, u32::MAX)
        );
        assert_eq!(
            IpHelperRouteTable::owned(&row, 0x1234),
            Some(net("10.66.0.0/16"))
        );
    }

    #[test]
    fn lists_only_static_metric_7_routes_on_the_adapter() {
        let ours = IpHelperRouteTable::row(0x1234, &net("10.66.0.0/16"));
        let other_adapter = MibIpforwardRow2 {
            interface_luid: 0x9999,
            ..ours
        };
        let other_metric = MibIpforwardRow2 { metric: 0, ..ours };
        let other_protocol = MibIpforwardRow2 {
            protocol: 2, // MIB_IPPROTO_LOCAL
            ..ours
        };
        for row in [other_adapter, other_metric, other_protocol] {
            assert_eq!(IpHelperRouteTable::owned(&row, 0x1234), None);
        }
    }

    /// A read-only listing: the loopback interface holds none of SplitWire's routes.
    #[test]
    fn the_loopback_interface_has_no_owned_routes() {
        assert!(
            IpHelperRouteTable
                .list_owned(LOOPBACK_LUID)
                .unwrap()
                .is_empty()
        );
    }
}
