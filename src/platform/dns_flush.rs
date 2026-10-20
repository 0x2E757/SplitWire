use std::io;

use super::abi::DnsFlushResolverCache;
use crate::dns::DnsCacheFlusher;

/// Flushes the Windows DNS client cache (`DnsFlushResolverCache`); only a live engine calls it.
pub struct DnsCacheFlush;

impl DnsCacheFlusher for DnsCacheFlush {
    fn flush(&self) -> io::Result<()> {
        if unsafe { DnsFlushResolverCache() } != 0 {
            Ok(())
        } else {
            Err(io::Error::other("DnsFlushResolverCache failed"))
        }
    }
}
