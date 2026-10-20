use std::collections::BTreeMap;
use std::io;
use std::net::Ipv4Addr;

use super::abi::*;
use crate::net::{Ipv4Net, NetworkProbe, OnLinkNetwork};
use crate::platform::abi::Guid;
use crate::platform::inet::AF_INET;
use crate::platform::registry::{RegistryKey, RegistryRoot};
use crate::routes::RouteGuard;
use crate::tunnel::TunnelSpec;

/// `impl NetworkProbe` over IP Helper and the TCP/IP interface keys, read-only: the networks of
/// the other interfaces and the DNS servers of the default-route interface. SplitWire's own
/// adapters (`SplitWire-*`) are left out.
pub struct IpHelperProbe;

impl IpHelperProbe {
    const INTERFACES: &'static str =
        r"SYSTEM\CurrentControlSet\Services\Tcpip\Parameters\Interfaces";

    /// The interface's alias, such as `Ethernet`; empty when it cannot be read.
    fn alias(luid: u64) -> String {
        let mut buffer = [0u16; 257];
        let status =
            unsafe { ConvertInterfaceLuidToAlias(&luid, buffer.as_mut_ptr(), buffer.len()) };
        if status != 0 {
            return String::new();
        }
        let len = buffer.iter().position(|c| *c == 0).unwrap_or(buffer.len());
        String::from_utf16_lossy(&buffer[..len])
    }

    fn ours(alias: &str) -> bool {
        alias.starts_with(TunnelSpec::ADAPTER_PREFIX)
    }

    /// The interface's own metric; a default route's cost is its metric plus this one.
    fn interface_metric(luid: u64) -> u32 {
        let mut row = zeroed::<MibIpinterfaceRow>();
        unsafe { InitializeIpInterfaceEntry(&mut row) };
        row.family = AF_INET;
        row.interface_luid = luid;
        match unsafe { GetIpInterfaceEntry(&mut row) } {
            0 => row.metric,
            _ => u32::MAX / 2,
        }
    }

    /// The interface of the cheapest IPv4 default route that is not SplitWire's.
    fn default_route_interface() -> io::Result<Option<u64>> {
        let table = ForwardTable::ipv4()?;
        Ok(table
            .rows()
            .iter()
            .filter(|row| row.destination_prefix.prefix_length == 0)
            .filter(|row| !Self::ours(&Self::alias(row.interface_luid)))
            .min_by_key(|row| {
                u64::from(row.metric) + u64::from(Self::interface_metric(row.interface_luid))
            })
            .map(|row| row.interface_luid))
    }

    /// Static DNS servers win over DHCP ones, as in Windows itself.
    fn dns_servers(luid: u64) -> io::Result<Vec<Ipv4Addr>> {
        let mut guid = Guid::from_u128(0);
        win32(unsafe { ConvertInterfaceLuidToGuid(&luid, &mut guid) })?;
        let path = format!(r"{}\{}", Self::INTERFACES, Self::braced(guid));
        let Some(key) = RegistryKey::open(RegistryRoot::LocalMachine, &path, false)? else {
            return Ok(Vec::new());
        };
        let configured = key.string("NameServer")?.filter(|s| !s.trim().is_empty());
        let text = match configured {
            Some(text) => text,
            None => key.string("DhcpNameServer")?.unwrap_or_default(),
        };
        Ok(text
            .split([',', ' ', ';'])
            .filter_map(|s| s.trim().parse().ok())
            .collect())
    }

    /// `{6B29FC40-CA47-1067-B31D-00DD010662DA}`, as the interface keys are named.
    fn braced(guid: Guid) -> String {
        let value = guid.to_u128();
        format!(
            "{{{:08X}-{:04X}-{:04X}-{:04X}-{:012X}}}",
            value >> 96,
            (value >> 80) & 0xffff,
            (value >> 64) & 0xffff,
            (value >> 48) & 0xffff,
            value & 0xffff_ffff_ffff
        )
    }
}

impl NetworkProbe for IpHelperProbe {
    fn on_link_networks(&self) -> io::Result<Vec<OnLinkNetwork>> {
        let table = UnicastTable::ipv4()?;
        let mut networks: BTreeMap<Ipv4Net, String> = BTreeMap::new();
        for row in table.rows() {
            let Some(address) = row.address.to_v4() else {
                continue;
            };
            // An odd prefix (0 on some virtual adapters) would forbid every route.
            if row.on_link_prefix_length < RouteGuard::MIN_PREFIX {
                continue;
            }
            let alias = Self::alias(row.interface_luid);
            if Self::ours(&alias) {
                continue;
            }
            if let Some(net) = Ipv4Net::new(*address.ip(), row.on_link_prefix_length) {
                networks.entry(net).or_insert(alias);
            }
        }
        Ok(networks
            .into_iter()
            .map(|(net, interface)| OnLinkNetwork { net, interface })
            .collect())
    }

    fn default_route_dns(&self) -> io::Result<Vec<Ipv4Addr>> {
        match Self::default_route_interface()? {
            Some(luid) => Self::dns_servers(luid),
            None => Ok(Vec::new()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::platform::ip_helper::addressing::tests::LOOPBACK_LUID;

    #[test]
    fn names_interface_keys_like_windows() {
        let guid = Guid::from_u128(0x6b29fc40_ca47_1067_b31d_00dd010662da);
        assert_eq!(
            IpHelperProbe::braced(guid),
            "{6B29FC40-CA47-1067-B31D-00DD010662DA}"
        );
    }

    /// Read-only: the loopback network is found, and no SplitWire adapter is listed.
    #[test]
    fn lists_the_on_link_networks_read_only() {
        let networks = IpHelperProbe.on_link_networks().unwrap();
        assert!(
            networks.iter().any(|n| n.net.to_string() == "127.0.0.0/8"),
            "{networks:?}"
        );
        assert!(networks.iter().all(|n| !IpHelperProbe::ours(&n.interface)));
        assert!(!IpHelperProbe::alias(LOOPBACK_LUID).is_empty());
    }

    /// Read-only: whatever the machine's default route is, the lookup succeeds.
    #[test]
    fn reads_the_default_route_dns_read_only() {
        let servers = IpHelperProbe.default_route_dns().unwrap();
        assert!(servers.len() <= 16);
    }
}
