use std::io;
use std::net::Ipv4Addr;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use crate::application::FolderAccess;
use crate::dns::{DnsCacheFlusher, DnsExchange, DnsExchangeFactory};
use crate::logs::Log;
use crate::platform::dns_exchange::BoundUdpExchange;

/// Counts and logs flush requests instead of flushing Windows' DNS cache.
pub struct DryRunDnsFlush {
    flushes: AtomicUsize,
    log: Log,
}

impl DryRunDnsFlush {
    pub fn new(log: Log) -> Self {
        Self {
            flushes: AtomicUsize::new(0),
            log: log.with_source("dns"),
        }
    }

    pub fn flushes(&self) -> usize {
        self.flushes.load(Ordering::Relaxed)
    }
}

impl DnsCacheFlusher for DryRunDnsFlush {
    fn flush(&self) -> io::Result<()> {
        self.flushes.fetch_add(1, Ordering::Relaxed);
        self.log
            .info("dry-run: would flush the Windows DNS client cache");
        Ok(())
    }
}

/// A profile's upstream exchange over loopback: there is no tunnel to bind to, so the socket is
/// bound to `127.0.0.1` and pinned to the loopback interface, and an upstream on loopback (a
/// fake DNS server) answers.
pub struct DryRunExchangeFactory {
    timeout: Duration,
    log: Log,
}

impl DryRunExchangeFactory {
    const LOOPBACK_INDEX: u32 = 1;

    pub fn new(timeout: Duration, log: Log) -> Self {
        Self {
            timeout,
            log: log.with_source("dns"),
        }
    }
}

impl DnsExchangeFactory for DryRunExchangeFactory {
    fn bound(&self, source: Ipv4Addr, if_index: u32) -> Arc<dyn DnsExchange> {
        self.log.info(format!(
            "dry-run: upstream DNS would leave from {source} on interface {if_index}; \
             it goes over loopback instead"
        ));
        Arc::new(BoundUdpExchange::new(
            Ipv4Addr::LOCALHOST,
            Self::LOOPBACK_INDEX,
            self.timeout,
        ))
    }
}

/// Leaves the data folder's ACL as it is.
pub struct DryRunFolderAccess {
    log: Log,
}

impl DryRunFolderAccess {
    pub fn new(log: Log) -> Self {
        Self { log }
    }
}

impl FolderAccess for DryRunFolderAccess {
    fn restrict(&self, root: &Path) -> io::Result<()> {
        self.log.info(format!(
            "dry-run: would restrict {} to SYSTEM and Administrators",
            root.display()
        ));
        Ok(())
    }
}
