use std::collections::BTreeSet;
use std::io;
use std::sync::Mutex;

use crate::logs::Log;
use crate::net::Ipv4Net;
use crate::routes::RouteTable;

/// Remembers the routes it was asked for, per adapter, and logs each change; changes nothing.
pub struct DryRunRouteTable {
    routes: Mutex<BTreeSet<(u64, Ipv4Net)>>,
    log: Log,
}

impl DryRunRouteTable {
    pub fn new(log: Log) -> Self {
        Self {
            routes: Mutex::new(BTreeSet::new()),
            log: log.with_source("routes"),
        }
    }

    /// The routes on the adapter `luid`.
    pub fn routes(&self, luid: u64) -> Vec<Ipv4Net> {
        self.routes
            .lock()
            .unwrap()
            .iter()
            .filter(|(on, _)| *on == luid)
            .map(|(_, net)| *net)
            .collect()
    }

    /// Every route with its adapter.
    pub fn all(&self) -> Vec<(u64, Ipv4Net)> {
        self.routes.lock().unwrap().iter().copied().collect()
    }

    pub fn is_empty(&self) -> bool {
        self.routes.lock().unwrap().is_empty()
    }
}

impl RouteTable for DryRunRouteTable {
    fn list_owned(&self, luid: u64) -> io::Result<Vec<Ipv4Net>> {
        Ok(self.routes(luid))
    }

    fn add(&self, luid: u64, net: Ipv4Net) -> io::Result<()> {
        self.log
            .info(format!("dry-run: would add route {net} on LUID {luid:#x}"));
        self.routes.lock().unwrap().insert((luid, net));
        Ok(())
    }

    fn remove(&self, luid: u64, net: Ipv4Net) -> io::Result<()> {
        if !self.routes.lock().unwrap().remove(&(luid, net)) {
            return Err(io::ErrorKind::NotFound.into());
        }
        self.log.info(format!(
            "dry-run: would remove route {net} on LUID {luid:#x}"
        ));
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::clock::ManualClock;
    use std::sync::Arc;
    use std::time::UNIX_EPOCH;

    #[test]
    fn keeps_routes_per_adapter_and_logs_them() {
        let log = Log::new(1 << 16, Arc::new(ManualClock::new(UNIX_EPOCH)), false);
        let table = DryRunRouteTable::new(log.clone());
        let net: Ipv4Net = "10.1.2.3".parse().unwrap();
        table.add(1, net).unwrap();
        table.add(2, net).unwrap();
        assert_eq!(table.list_owned(1).unwrap(), [net]);
        table.remove(1, net).unwrap();
        assert!(table.routes(1).is_empty());
        assert_eq!(table.routes(2), [net]);
        assert_eq!(
            table.remove(1, net).unwrap_err().kind(),
            io::ErrorKind::NotFound
        );
        assert!(log.contains("dry-run: would add route 10.1.2.3/32 on LUID 0x2"));
    }
}
