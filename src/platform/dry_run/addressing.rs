use std::io;

use crate::logs::Log;
use crate::tunnel::{AdapterAddressing, AdapterSettings};

/// Logs the address, MTU and DNS settings an adapter would get; changes nothing.
pub struct DryRunAddressing {
    log: Log,
}

impl DryRunAddressing {
    pub fn new(log: Log) -> Self {
        Self {
            log: log.with_source("tunnel"),
        }
    }

    /// A fake interface index for a fake LUID.
    pub fn index_of(luid: u64) -> u32 {
        1_000 + (luid & 0xffff) as u32
    }
}

impl AdapterAddressing for DryRunAddressing {
    fn apply(&self, luid: u64, settings: &AdapterSettings) -> io::Result<()> {
        self.log.info(format!(
            "dry-run: would give LUID {luid:#x} {}/32, MTU {}, metric 9000, no DNS servers",
            settings.address, settings.mtu
        ));
        Ok(())
    }

    fn applied(&self, _luid: u64, _settings: &AdapterSettings) -> io::Result<bool> {
        Ok(true)
    }

    fn index(&self, luid: u64) -> io::Result<u32> {
        Ok(Self::index_of(luid))
    }
}
