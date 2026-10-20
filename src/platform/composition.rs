//! The only place that names the concrete platform types: the live set and the dry-run set the
//! engine is built from.

use std::io;
use std::net::SocketAddrV4;
use std::sync::Arc;
use std::time::Duration;

use super::dns_exchange::{BoundExchangeFactory, UdpDnsExchange};
use super::dns_flush::DnsCacheFlush;
use super::dpapi::DpapiSecretStore;
use super::dry_run::{
    DryRunAddressing, DryRunDnsFlush, DryRunDriver, DryRunExchangeFactory, DryRunFolderAccess,
    DryRunNrpt, DryRunRouteTable,
};
use super::folder_security::FolderSecurity;
use super::ip_helper::{IpHelperAddressing, IpHelperProbe, IpHelperRouteTable};
use super::library::AppLibrary;
use super::nrpt::{GroupPolicyNrptProbe, RegistryNrpt};
use super::wireguard::WireGuardNt;
use crate::application::FolderAccess;
use crate::clock::Clock;
use crate::dns::{DnsCacheFlusher, DnsExchange, DnsExchangeFactory, NrptPolicyProbe, NrptRules};
use crate::logs::Log;
use crate::net::NetworkProbe;
use crate::profile::SecretStore;
use crate::routes::RouteTable;
use crate::tunnel::{AdapterAddressing, WireGuardDriver};

/// Everything the engine needs from the machine, chosen once at start.
#[derive(Clone)]
pub struct PlatformSet {
    pub driver: Arc<dyn WireGuardDriver>,
    pub addressing: Arc<dyn AdapterAddressing>,
    pub routes: Arc<dyn RouteTable>,
    pub nrpt: Arc<dyn NrptRules>,
    pub nrpt_policy: Arc<dyn NrptPolicyProbe>,
    pub flusher: Arc<dyn DnsCacheFlusher>,
    pub probe: Arc<dyn NetworkProbe>,
    /// The fallback servers' exchange, unbound.
    pub exchange: Arc<dyn DnsExchange>,
    /// A profile's upstream exchange, bound to its adapter.
    pub bound: Arc<dyn DnsExchangeFactory>,
    pub secrets: Arc<dyn SecretStore>,
    pub folder: Arc<dyn FolderAccess>,
    pub dry_run: bool,
}

impl PlatformSet {
    /// Each upstream exchange waits at most this long.
    pub const DNS_TIMEOUT: Duration = Duration::from_secs(2);

    /// The real thing: wireguard-nt, IP Helper, NRPT under HKLM, the DNS flush and the folder
    /// ACL (an administrator or LocalSystem only). NRPT can only point at port 53.
    pub fn live(listen: SocketAddrV4, log: &Log) -> io::Result<Self> {
        if listen.port() != 53 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("listen {listen}: NRPT rules point at port 53 only; use --dry-run"),
            ));
        }
        AppLibrary::restrict_default_search()?;
        Ok(Self {
            driver: Arc::new(WireGuardNt::new(AppLibrary::beside_exe()?, log.clone())),
            addressing: Arc::new(IpHelperAddressing),
            routes: Arc::new(IpHelperRouteTable),
            nrpt: Arc::new(RegistryNrpt::live(*listen.ip())),
            nrpt_policy: Arc::new(GroupPolicyNrptProbe::live()),
            flusher: Arc::new(DnsCacheFlush),
            probe: Arc::new(IpHelperProbe),
            exchange: Arc::new(UdpDnsExchange::new(Self::DNS_TIMEOUT)),
            bound: Arc::new(BoundExchangeFactory::new(Self::DNS_TIMEOUT)),
            secrets: Arc::new(DpapiSecretStore::profiles()),
            folder: Arc::new(FolderSecurity),
            dry_run: false,
        })
    }

    /// Adapters, routes, NRPT, the flush and the ACL only log; the interface queries and the
    /// Group Policy probe (read-only), DNS and DPAPI are real. Runs unelevated.
    pub fn dry_run(log: &Log, clock: Arc<dyn Clock>) -> Self {
        Self {
            driver: Arc::new(DryRunDriver::new(log.clone(), clock)),
            addressing: Arc::new(DryRunAddressing::new(log.clone())),
            routes: Arc::new(DryRunRouteTable::new(log.clone())),
            nrpt: Arc::new(DryRunNrpt::new(log.clone())),
            nrpt_policy: Arc::new(GroupPolicyNrptProbe::live()),
            flusher: Arc::new(DryRunDnsFlush::new(log.clone())),
            probe: Arc::new(IpHelperProbe),
            exchange: Arc::new(UdpDnsExchange::new(Self::DNS_TIMEOUT)),
            bound: Arc::new(DryRunExchangeFactory::new(Self::DNS_TIMEOUT, log.clone())),
            secrets: Arc::new(DpapiSecretStore::profiles()),
            folder: Arc::new(DryRunFolderAccess::new(log.clone())),
            dry_run: true,
        }
    }
}
