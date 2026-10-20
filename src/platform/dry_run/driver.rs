use std::io;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::SystemTime;

use crate::clock::Clock;
use crate::logs::Log;
use crate::profile::ProfileFacts;
use crate::tunnel::{
    AdapterState, DriverConfig, DriverVersion, TunnelHealth, WireGuardAdapter, WireGuardDriver,
};

/// A wireguard-nt stand-in that logs what would be done: no DLL, no driver, no adapter. Its
/// adapters report a handshake while up and a little traffic, so the health and the traffic
/// samples have something to show.
pub struct DryRunDriver {
    log: Log,
    clock: Arc<dyn Clock>,
    next_luid: AtomicU64,
}

impl DryRunDriver {
    /// Fake LUIDs: interface type 53 (`IF_TYPE_PROP_VIRTUAL`), counting from 1.
    const FIRST_LUID: u64 = (53 << 48) | 1;

    pub fn new(log: Log, clock: Arc<dyn Clock>) -> Self {
        Self {
            log: log.with_source("wireguard"),
            clock,
            next_luid: AtomicU64::new(Self::FIRST_LUID),
        }
    }
}

impl WireGuardDriver for DryRunDriver {
    fn create(
        &self,
        name: &str,
        tunnel_type: &str,
        _guid: u128,
    ) -> io::Result<Box<dyn WireGuardAdapter>> {
        let luid = self.next_luid.fetch_add(1, Ordering::SeqCst);
        self.log.info(format!(
            "dry-run: would create adapter {name} (tunnel type {tunnel_type}): fake LUID {luid:#x}"
        ));
        Ok(Box::new(DryRunAdapter {
            name: name.to_owned(),
            luid,
            created: self.clock.now(),
            up: Mutex::new(false),
            clock: self.clock.clone(),
            log: self.log.clone(),
        }))
    }

    fn open(&self, _name: &str) -> io::Result<Option<Box<dyn WireGuardAdapter>>> {
        Ok(None)
    }

    fn running_version(&self) -> io::Result<Option<DriverVersion>> {
        Ok(None)
    }

    fn delete_driver(&self) -> io::Result<()> {
        self.log
            .info("dry-run: would delete the wireguard-nt driver");
        Ok(())
    }
}

struct DryRunAdapter {
    name: String,
    luid: u64,
    created: SystemTime,
    up: Mutex<bool>,
    clock: Arc<dyn Clock>,
    log: Log,
}

impl WireGuardAdapter for DryRunAdapter {
    fn luid(&self) -> u64 {
        self.luid
    }

    /// Logs the public parts only; the keys never reach a log.
    fn set_config(&self, config: &DriverConfig) -> io::Result<()> {
        let what = match config {
            DriverConfig::Full(spec) => format!(
                "peer {}, endpoint {}, keepalive {}, allowed IP 0.0.0.0/0",
                ProfileFacts::short_key(&spec.peer_public_key),
                spec.endpoint,
                spec.keepalive
            ),
            DriverConfig::Endpoint { endpoint, .. } => format!("endpoint {endpoint} (update only)"),
        };
        self.log
            .info(format!("dry-run: would configure {}: {what}", self.name));
        Ok(())
    }

    fn config(&self) -> io::Result<TunnelHealth> {
        let now = self.clock.now();
        let up = *self.up.lock().unwrap();
        let secs = now
            .duration_since(self.created)
            .unwrap_or_default()
            .as_secs();
        Ok(TunnelHealth {
            interface_public_key: [0; 32],
            last_handshake: up.then_some(now),
            rx_bytes: secs * 2_048,
            tx_bytes: secs * 512,
            endpoint: None,
        })
    }

    fn set_state(&self, state: AdapterState) -> io::Result<()> {
        self.log
            .info(format!("dry-run: would set {} {state:?}", self.name));
        *self.up.lock().unwrap() = state == AdapterState::Up;
        Ok(())
    }

    fn state(&self) -> io::Result<AdapterState> {
        Ok(if *self.up.lock().unwrap() {
            AdapterState::Up
        } else {
            AdapterState::Down
        })
    }

    fn set_logging(&self, _on: bool) -> io::Result<()> {
        Ok(())
    }
}

impl Drop for DryRunAdapter {
    fn drop(&mut self) {
        self.log.info(format!(
            "dry-run: would close {} (which removes it)",
            self.name
        ));
    }
}
