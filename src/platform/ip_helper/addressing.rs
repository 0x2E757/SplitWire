use std::io;
use std::net::{Ipv4Addr, SocketAddrV4};

use super::abi::*;
use crate::platform::abi::Guid;
use crate::platform::inet::{AF_INET, SockaddrInet};
use crate::tunnel::{AdapterAddressing, AdapterSettings};

/// `impl AdapterAddressing` over IP Helper: `CreateUnicastIpAddressEntry` for the /32,
/// `SetIpInterfaceEntry` for the MTU and metric, `SetInterfaceDnsSettings` for no DNS. IPv6 is
/// never configured.
pub struct IpHelperAddressing;

impl IpHelperAddressing {
    /// High, so the multicast and broadcast routes Windows adds to every interface never beat
    /// the LAN's.
    pub const INTERFACE_METRIC: u32 = 9000;
    const HOST_PREFIX: u8 = 32;

    /// The /32 address row, preferred at once (no duplicate detection on a tunnel), forever.
    fn address_row(luid: u64, address: Ipv4Addr) -> MibUnicastIpaddressRow {
        let mut row = zeroed::<MibUnicastIpaddressRow>();
        unsafe { InitializeUnicastIpAddressEntry(&mut row) };
        row.address = SockaddrInet::v4(SocketAddrV4::new(address, 0));
        row.interface_luid = luid;
        row.on_link_prefix_length = Self::HOST_PREFIX;
        row.dad_state = DAD_STATE_PREFERRED;
        row.valid_lifetime = u32::MAX;
        row.preferred_lifetime = u32::MAX;
        row
    }

    /// The address as it is on the adapter now, `None` when it is not there (read-only).
    fn address_entry(luid: u64, address: Ipv4Addr) -> io::Result<Option<MibUnicastIpaddressRow>> {
        let mut row = Self::address_row(luid, address);
        match unsafe { GetUnicastIpAddressEntry(&mut row) } {
            0 => Ok(Some(row)),
            ERROR_NOT_FOUND => Ok(None),
            code => win32(code).map(|_| None),
        }
    }

    /// The adapter's IPv4 settings as they are now (read-only).
    fn interface_row(luid: u64) -> io::Result<MibIpinterfaceRow> {
        let mut row = zeroed::<MibIpinterfaceRow>();
        unsafe { InitializeIpInterfaceEntry(&mut row) };
        row.family = AF_INET;
        row.interface_luid = luid;
        win32(unsafe { GetIpInterfaceEntry(&mut row) })?;
        Ok(row)
    }

    /// The changes `SetIpInterfaceEntry` writes; `SitePrefixLength` must be 0 for IPv4.
    fn configure(row: &mut MibIpinterfaceRow, mtu: u16) {
        row.nl_mtu = u32::from(mtu);
        row.use_automatic_metric = 0;
        row.metric = Self::INTERFACE_METRIC;
        row.site_prefix_length = 0;
    }

    /// No name servers and no registration; `empty` is the empty string the name servers are
    /// set to.
    fn dns_settings(empty: &[u16; 1]) -> DnsInterfaceSettings {
        DnsInterfaceSettings {
            version: DNS_INTERFACE_SETTINGS_VERSION1,
            flags: DNS_SETTING_NAMESERVER | DNS_SETTING_REGISTRATION_ENABLED,
            domain: std::ptr::null(),
            name_server: empty.as_ptr(),
            search_list: std::ptr::null(),
            registration_enabled: 0,
            register_adapter_name: 0,
            enable_llmnr: 0,
            query_adapter_name: 0,
            profile_name_server: std::ptr::null(),
        }
    }

    fn guid(luid: u64) -> io::Result<Guid> {
        let mut guid = Guid::from_u128(0);
        win32(unsafe { ConvertInterfaceLuidToGuid(&luid, &mut guid) })?;
        Ok(guid)
    }
}

impl AdapterAddressing for IpHelperAddressing {
    fn apply(&self, luid: u64, settings: &AdapterSettings) -> io::Result<()> {
        let address = Self::address_row(luid, settings.address);
        match unsafe { CreateUnicastIpAddressEntry(&address) } {
            ERROR_OBJECT_ALREADY_EXISTS => {}
            code => win32(code)?,
        }
        let mut interface = Self::interface_row(luid)?;
        Self::configure(&mut interface, settings.mtu);
        win32(unsafe { SetIpInterfaceEntry(&mut interface) })?;
        let empty = [0u16; 1];
        let dns = Self::dns_settings(&empty);
        win32(unsafe { SetInterfaceDnsSettings(Self::guid(luid)?, &dns) })
    }

    fn applied(&self, luid: u64, settings: &AdapterSettings) -> io::Result<bool> {
        let has_address = Self::address_entry(luid, settings.address)?
            .is_some_and(|row| row.on_link_prefix_length == Self::HOST_PREFIX);
        Ok(has_address && Self::interface_row(luid)?.nl_mtu == u32::from(settings.mtu))
    }

    fn index(&self, luid: u64) -> io::Result<u32> {
        let mut index = 0;
        win32(unsafe { ConvertInterfaceLuidToIndex(&luid, &mut index) })?;
        Ok(index)
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// The software loopback interface: type 24, the first of its type, interface index 1.
    pub(crate) const LOOPBACK_LUID: u64 = 24 << 48;
    const LOOPBACK_INDEX: u32 = 1;

    #[test]
    fn builds_a_host_address_row_without_adding_it() {
        let row = IpHelperAddressing::address_row(0x1234, Ipv4Addr::new(10, 88, 4, 12));
        assert_eq!(row.interface_luid, 0x1234);
        assert_eq!(row.address.family, AF_INET);
        assert_eq!(row.address.addr, [10, 88, 4, 12]);
        assert_eq!(row.on_link_prefix_length, 32);
        assert_eq!(row.dad_state, DAD_STATE_PREFERRED);
        assert_eq!(
            (row.valid_lifetime, row.preferred_lifetime),
            (u32::MAX, u32::MAX)
        );
    }

    #[test]
    fn configures_the_mtu_and_a_fixed_metric_on_a_copy() {
        let mut row = IpHelperAddressing::interface_row(LOOPBACK_LUID).unwrap();
        IpHelperAddressing::configure(&mut row, 1380);
        assert_eq!(row.nl_mtu, 1380);
        assert_eq!(row.use_automatic_metric, 0);
        assert_eq!(row.metric, 9000);
        assert_eq!(row.site_prefix_length, 0);
    }

    #[test]
    fn dns_settings_clear_the_servers_and_registration() {
        let empty = [0u16; 1];
        let settings = IpHelperAddressing::dns_settings(&empty);
        assert_eq!(settings.version, 1);
        assert_eq!(settings.flags, 0x2 | 0x8);
        assert_eq!(unsafe { *settings.name_server }, 0);
        assert_eq!(settings.registration_enabled, 0);
        assert!(settings.domain.is_null() && settings.search_list.is_null());
    }

    /// Read-only calls on the loopback interface check the rows against live data.
    #[test]
    fn reads_the_loopback_interface() {
        let addressing = IpHelperAddressing;
        assert_eq!(addressing.index(LOOPBACK_LUID).unwrap(), LOOPBACK_INDEX);
        let row = IpHelperAddressing::interface_row(LOOPBACK_LUID).unwrap();
        assert_eq!(row.interface_luid, LOOPBACK_LUID);
        assert_eq!(row.interface_index, LOOPBACK_INDEX);
        let address = IpHelperAddressing::address_entry(LOOPBACK_LUID, Ipv4Addr::LOCALHOST)
            .unwrap()
            .unwrap();
        assert_eq!(address.on_link_prefix_length, 8);
        assert_eq!(address.interface_index, LOOPBACK_INDEX);
        let missing = IpHelperAddressing::address_entry(LOOPBACK_LUID, Ipv4Addr::new(10, 1, 2, 3));
        assert!(missing.unwrap().is_none());
        // 127.0.0.1 is a /8 there, so it does not count as a tunnel's /32.
        let settings = AdapterSettings {
            address: Ipv4Addr::LOCALHOST,
            mtu: 1420,
        };
        assert!(!addressing.applied(LOOPBACK_LUID, &settings).unwrap());
        assert!(IpHelperAddressing::guid(LOOPBACK_LUID).is_ok());
    }

    #[test]
    fn a_missing_adapter_has_no_index() {
        assert!(IpHelperAddressing.index(0x7fff << 48).is_err());
    }
}
