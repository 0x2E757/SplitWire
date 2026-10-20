//! The IP Helper rows and functions (`netioapi.h`) SplitWire uses, IPv4 only. Fields SplitWire
//! never reads or writes are kept as opaque bytes; the layout tests pin the ones it does.

use std::io;
use std::ptr::NonNull;

use crate::platform::abi::Guid;
use crate::platform::inet::SockaddrInet;

pub const ERROR_NOT_FOUND: u32 = 1168;
pub const ERROR_OBJECT_ALREADY_EXISTS: u32 = 5010;
/// `MIB_IPPROTO_NETMGMT`: a static route added through the API.
pub const ROUTE_PROTOCOL_NETMGMT: u32 = 3;
/// `IpDadStatePreferred`.
pub const DAD_STATE_PREFERRED: u32 = 4;
pub const DNS_INTERFACE_SETTINGS_VERSION1: u32 = 1;
pub const DNS_SETTING_NAMESERVER: u64 = 0x0002;
pub const DNS_SETTING_REGISTRATION_ENABLED: u64 = 0x0008;

/// A Win32 status as returned by IP Helper: `Ok` for `NO_ERROR`.
pub fn win32(status: u32) -> io::Result<()> {
    match status {
        0 => Ok(()),
        code => Err(io::Error::from_raw_os_error(code as i32)),
    }
}

/// C `IP_ADDRESS_PREFIX`.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct IpAddressPrefix {
    pub prefix: SockaddrInet,
    pub prefix_length: u8,
}

/// C `MIB_IPFORWARD_ROW2`: one route.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct MibIpforwardRow2 {
    pub interface_luid: u64,
    pub interface_index: u32,
    pub destination_prefix: IpAddressPrefix,
    pub next_hop: SockaddrInet,
    pub site_prefix_length: u8,
    pub valid_lifetime: u32,
    pub preferred_lifetime: u32,
    pub metric: u32,
    pub protocol: u32,
    pub loopback: u8,
    pub autoconfigure_address: u8,
    pub publish: u8,
    pub immortal: u8,
    pub age: u32,
    pub origin: u32,
}

/// C `MIB_UNICASTIPADDRESS_ROW`: one address on an interface.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct MibUnicastIpaddressRow {
    pub address: SockaddrInet,
    pub interface_luid: u64,
    pub interface_index: u32,
    pub prefix_origin: u32,
    pub suffix_origin: u32,
    pub valid_lifetime: u32,
    pub preferred_lifetime: u32,
    pub on_link_prefix_length: u8,
    pub skip_as_source: u8,
    pub dad_state: u32,
    pub scope_id: u32,
    pub creation_time_stamp: i64,
}

/// C `MIB_IPINTERFACE_ROW`: one interface's IPv4 or IPv6 settings.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct MibIpinterfaceRow {
    pub family: u16,
    pub interface_luid: u64,
    pub interface_index: u32,
    _reassembly_to_advertise: [u8; 24],
    pub use_automatic_metric: u8,
    _discovery_to_zones: [u8; 99],
    pub site_prefix_length: u32,
    pub metric: u32,
    pub nl_mtu: u32,
    _connected_to_offload: [u8; 12],
}

/// C `DNS_INTERFACE_SETTINGS` (version 1).
#[repr(C)]
pub struct DnsInterfaceSettings {
    pub version: u32,
    pub flags: u64,
    pub domain: *const u16,
    pub name_server: *const u16,
    pub search_list: *const u16,
    pub registration_enabled: u32,
    pub register_adapter_name: u32,
    pub enable_llmnr: u32,
    pub query_adapter_name: u32,
    pub profile_name_server: *const u16,
}

/// C `MIB_IPFORWARD_TABLE2`: the count, then the rows at an 8-byte boundary.
#[repr(C)]
struct MibIpforwardTable2 {
    num_entries: u32,
    table: [MibIpforwardRow2; 0],
}

/// The IPv4 route table as `GetIpForwardTable2` returns it; freed on drop.
pub struct ForwardTable(NonNull<MibIpforwardTable2>);

impl ForwardTable {
    pub fn ipv4() -> io::Result<Self> {
        let mut table = std::ptr::null_mut();
        win32(unsafe { GetIpForwardTable2(crate::platform::inet::AF_INET, &mut table) })?;
        NonNull::new(table)
            .map(Self)
            .ok_or_else(|| io::Error::other("GetIpForwardTable2 returned no table"))
    }

    pub fn rows(&self) -> &[MibIpforwardRow2] {
        unsafe {
            let table = self.0.as_ptr();
            std::slice::from_raw_parts((*table).table.as_ptr(), (*table).num_entries as usize)
        }
    }
}

impl Drop for ForwardTable {
    fn drop(&mut self) {
        unsafe { FreeMibTable(self.0.as_ptr().cast()) };
    }
}

/// C `MIB_UNICASTIPADDRESS_TABLE`: the count, then the rows at an 8-byte boundary.
#[repr(C)]
struct MibUnicastIpaddressTable {
    num_entries: u32,
    table: [MibUnicastIpaddressRow; 0],
}

/// Every IPv4 address of every interface as `GetUnicastIpAddressTable` returns them (read-only);
/// freed on drop.
pub struct UnicastTable(NonNull<MibUnicastIpaddressTable>);

impl UnicastTable {
    pub fn ipv4() -> io::Result<Self> {
        let mut table = std::ptr::null_mut();
        win32(unsafe { GetUnicastIpAddressTable(crate::platform::inet::AF_INET, &mut table) })?;
        NonNull::new(table)
            .map(Self)
            .ok_or_else(|| io::Error::other("GetUnicastIpAddressTable returned no table"))
    }

    pub fn rows(&self) -> &[MibUnicastIpaddressRow] {
        unsafe {
            let table = self.0.as_ptr();
            std::slice::from_raw_parts((*table).table.as_ptr(), (*table).num_entries as usize)
        }
    }
}

impl Drop for UnicastTable {
    fn drop(&mut self) {
        unsafe { FreeMibTable(self.0.as_ptr().cast()) };
    }
}

/// A row of zeros, which `Initialize*Entry` then fills with the defaults.
pub fn zeroed<T: Copy>() -> T {
    // Only used for the plain-data rows above, for which all zeros is a valid value.
    unsafe { std::mem::zeroed() }
}

#[link(name = "iphlpapi")]
unsafe extern "system" {
    fn GetIpForwardTable2(family: u16, table: *mut *mut MibIpforwardTable2) -> u32;
    fn GetUnicastIpAddressTable(family: u16, table: *mut *mut MibUnicastIpaddressTable) -> u32;
    fn FreeMibTable(memory: *const std::ffi::c_void);
    pub fn ConvertInterfaceLuidToAlias(luid: *const u64, alias: *mut u16, length: usize) -> u32;
    pub fn InitializeIpForwardEntry(row: *mut MibIpforwardRow2);
    pub fn CreateIpForwardEntry2(row: *const MibIpforwardRow2) -> u32;
    pub fn DeleteIpForwardEntry2(row: *const MibIpforwardRow2) -> u32;
    pub fn InitializeUnicastIpAddressEntry(row: *mut MibUnicastIpaddressRow);
    pub fn CreateUnicastIpAddressEntry(row: *const MibUnicastIpaddressRow) -> u32;
    pub fn GetUnicastIpAddressEntry(row: *mut MibUnicastIpaddressRow) -> u32;
    pub fn InitializeIpInterfaceEntry(row: *mut MibIpinterfaceRow);
    pub fn GetIpInterfaceEntry(row: *mut MibIpinterfaceRow) -> u32;
    pub fn SetIpInterfaceEntry(row: *mut MibIpinterfaceRow) -> u32;
    pub fn SetInterfaceDnsSettings(interface: Guid, settings: *const DnsInterfaceSettings) -> u32;
    pub fn ConvertInterfaceLuidToGuid(luid: *const u64, guid: *mut Guid) -> u32;
    pub fn ConvertInterfaceLuidToIndex(luid: *const u64, index: *mut u32) -> u32;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::platform::inet::AF_INET;
    use std::mem::{offset_of, size_of};

    #[test]
    fn layouts_match_the_sdk() {
        assert_eq!(size_of::<IpAddressPrefix>(), 32); // IP_ADDRESS_PREFIX
        assert_eq!(offset_of!(IpAddressPrefix, prefix_length), 28); // IP_ADDRESS_PREFIX.PrefixLength
        assert_eq!(size_of::<MibIpforwardRow2>(), 104); // MIB_IPFORWARD_ROW2
        assert_eq!(offset_of!(MibIpforwardRow2, interface_index), 8); // MIB_IPFORWARD_ROW2.InterfaceIndex
        assert_eq!(offset_of!(MibIpforwardRow2, destination_prefix), 12); // MIB_IPFORWARD_ROW2.DestinationPrefix
        assert_eq!(offset_of!(MibIpforwardRow2, next_hop), 44); // MIB_IPFORWARD_ROW2.NextHop
        assert_eq!(offset_of!(MibIpforwardRow2, site_prefix_length), 72); // MIB_IPFORWARD_ROW2.SitePrefixLength
        assert_eq!(offset_of!(MibIpforwardRow2, valid_lifetime), 76); // MIB_IPFORWARD_ROW2.ValidLifetime
        assert_eq!(offset_of!(MibIpforwardRow2, preferred_lifetime), 80); // MIB_IPFORWARD_ROW2.PreferredLifetime
        assert_eq!(offset_of!(MibIpforwardRow2, metric), 84); // MIB_IPFORWARD_ROW2.Metric
        assert_eq!(offset_of!(MibIpforwardRow2, protocol), 88); // MIB_IPFORWARD_ROW2.Protocol
        assert_eq!(offset_of!(MibIpforwardRow2, loopback), 92); // MIB_IPFORWARD_ROW2.Loopback
        assert_eq!(offset_of!(MibIpforwardRow2, immortal), 95); // MIB_IPFORWARD_ROW2.Immortal
        assert_eq!(offset_of!(MibIpforwardRow2, age), 96); // MIB_IPFORWARD_ROW2.Age
        assert_eq!(offset_of!(MibIpforwardRow2, origin), 100); // MIB_IPFORWARD_ROW2.Origin
        assert_eq!(offset_of!(MibIpforwardTable2, table), 8); // MIB_IPFORWARD_TABLE2.Table
        assert_eq!(offset_of!(MibUnicastIpaddressTable, table), 8); // MIB_UNICASTIPADDRESS_TABLE.Table
        assert_eq!(size_of::<MibUnicastIpaddressRow>(), 80); // MIB_UNICASTIPADDRESS_ROW
        assert_eq!(offset_of!(MibUnicastIpaddressRow, interface_luid), 32); // MIB_UNICASTIPADDRESS_ROW.InterfaceLuid
        assert_eq!(offset_of!(MibUnicastIpaddressRow, interface_index), 40); // MIB_UNICASTIPADDRESS_ROW.InterfaceIndex
        assert_eq!(offset_of!(MibUnicastIpaddressRow, prefix_origin), 44); // MIB_UNICASTIPADDRESS_ROW.PrefixOrigin
        assert_eq!(offset_of!(MibUnicastIpaddressRow, valid_lifetime), 52); // MIB_UNICASTIPADDRESS_ROW.ValidLifetime
        assert_eq!(offset_of!(MibUnicastIpaddressRow, preferred_lifetime), 56); // MIB_UNICASTIPADDRESS_ROW.PreferredLifetime
        assert_eq!(
            offset_of!(MibUnicastIpaddressRow, on_link_prefix_length),
            60
        ); // MIB_UNICASTIPADDRESS_ROW.OnLinkPrefixLength
        assert_eq!(offset_of!(MibUnicastIpaddressRow, skip_as_source), 61); // MIB_UNICASTIPADDRESS_ROW.SkipAsSource
        assert_eq!(offset_of!(MibUnicastIpaddressRow, dad_state), 64); // MIB_UNICASTIPADDRESS_ROW.DadState
        assert_eq!(offset_of!(MibUnicastIpaddressRow, scope_id), 68); // MIB_UNICASTIPADDRESS_ROW.ScopeId
        assert_eq!(offset_of!(MibUnicastIpaddressRow, creation_time_stamp), 72); // MIB_UNICASTIPADDRESS_ROW.CreationTimeStamp
        assert_eq!(size_of::<MibIpinterfaceRow>(), 168); // MIB_IPINTERFACE_ROW
        assert_eq!(offset_of!(MibIpinterfaceRow, interface_luid), 8); // MIB_IPINTERFACE_ROW.InterfaceLuid
        assert_eq!(offset_of!(MibIpinterfaceRow, interface_index), 16); // MIB_IPINTERFACE_ROW.InterfaceIndex
        assert_eq!(offset_of!(MibIpinterfaceRow, use_automatic_metric), 44); // MIB_IPINTERFACE_ROW.UseAutomaticMetric
        assert_eq!(offset_of!(MibIpinterfaceRow, site_prefix_length), 144); // MIB_IPINTERFACE_ROW.SitePrefixLength
        assert_eq!(offset_of!(MibIpinterfaceRow, metric), 148); // MIB_IPINTERFACE_ROW.Metric
        assert_eq!(offset_of!(MibIpinterfaceRow, nl_mtu), 152); // MIB_IPINTERFACE_ROW.NlMtu
        assert_eq!(size_of::<DnsInterfaceSettings>(), 64); // DNS_INTERFACE_SETTINGS
        assert_eq!(offset_of!(DnsInterfaceSettings, flags), 8); // DNS_INTERFACE_SETTINGS.Flags
        assert_eq!(offset_of!(DnsInterfaceSettings, name_server), 24); // DNS_INTERFACE_SETTINGS.NameServer
        assert_eq!(offset_of!(DnsInterfaceSettings, registration_enabled), 40); // DNS_INTERFACE_SETTINGS.RegistrationEnabled
        assert_eq!(offset_of!(DnsInterfaceSettings, profile_name_server), 56); // DNS_INTERFACE_SETTINGS.ProfileNameServer
    }

    /// A read-only look at the real route table checks the row layout against live data.
    #[test]
    fn reads_the_ipv4_route_table() {
        let table = ForwardTable::ipv4().unwrap();
        let rows = table.rows();
        assert!(!rows.is_empty());
        for row in rows {
            assert_eq!(row.destination_prefix.prefix.family, AF_INET);
            assert!(row.destination_prefix.prefix_length <= 32);
            assert_ne!(row.interface_luid, 0);
        }
    }
}
