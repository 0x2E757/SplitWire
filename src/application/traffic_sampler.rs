use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use super::ActiveProfiles;
use crate::clock::Clock;
use crate::tunnel::Tunnel;

/// Every second, each connected profile's byte counters become a traffic sample (the 5-minute
/// chart and the rates). One sequence number per tick, shared by the profiles.
pub struct TrafficSampler {
    active: Arc<ActiveProfiles>,
    tunnel: Arc<dyn Tunnel>,
    clock: Arc<dyn Clock>,
    seq: AtomicU64,
}

impl TrafficSampler {
    pub fn new(
        active: Arc<ActiveProfiles>,
        tunnel: Arc<dyn Tunnel>,
        clock: Arc<dyn Clock>,
    ) -> Self {
        Self {
            active,
            tunnel,
            clock,
            seq: AtomicU64::new(0),
        }
    }

    pub fn tick(&self) {
        let seq = self.seq.fetch_add(1, Ordering::SeqCst) + 1;
        let now = self.clock.now();
        for profile in self.active.all() {
            if let Ok(health) = self.tunnel.health(&profile.name) {
                profile
                    .traffic
                    .lock()
                    .unwrap()
                    .record(seq, now, health.rx_bytes, health.tx_bytes);
            }
        }
    }

    /// The last sequence number handed out.
    pub fn seq(&self) -> u64 {
        self.seq.load(Ordering::SeqCst)
    }
}
