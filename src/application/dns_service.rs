use std::net::Ipv4Addr;
use std::sync::{Arc, Mutex};

use super::{ActiveProfiles, AlertService};
use crate::alerts::AlertTexts;
use crate::clock::Clock;
use crate::dns::{DnsCacheFlusher, QueryHandler, QueryOutcome};
use crate::logs::Log;
use crate::routes::HostClaims;

/// Answers a query and routes a listed answer's addresses on its profile's adapter before the
/// answer leaves, so the client's first connection already goes through the tunnel. A held
/// address raises `address_held` for the profile that was refused it. The first answer of a
/// profile's upstream since its Connect or since it failed flushes the Windows DNS cache, which
/// may remember the failures.
pub struct DnsService {
    handler: QueryHandler,
    active: Arc<ActiveProfiles>,
    claims: Arc<Mutex<HostClaims>>,
    alerts: Arc<AlertService>,
    flusher: Arc<dyn DnsCacheFlusher>,
    clock: Arc<dyn Clock>,
    log: Log,
}

/// What a `DnsService` acts on besides its handler.
pub struct DnsServiceParts {
    pub active: Arc<ActiveProfiles>,
    pub claims: Arc<Mutex<HostClaims>>,
    pub alerts: Arc<AlertService>,
    pub flusher: Arc<dyn DnsCacheFlusher>,
    pub clock: Arc<dyn Clock>,
}

impl DnsService {
    pub fn new(handler: QueryHandler, parts: DnsServiceParts, log: Log) -> Self {
        Self {
            handler,
            active: parts.active,
            claims: parts.claims,
            alerts: parts.alerts,
            flusher: parts.flusher,
            clock: parts.clock,
            log: log.with_source("dns"),
        }
    }

    /// `None` for a message that is not a query.
    pub fn answer(&self, raw: &[u8]) -> Option<QueryOutcome> {
        let outcome = self.handler.handle(raw)?;
        let Some(listed) = &outcome.listed else {
            return Some(outcome);
        };
        if listed.recovered {
            self.flush(&listed.profile);
        }
        let profile = self.active.get(&listed.profile);
        for (ip, holder) in &listed.held {
            self.alerts.raise(AlertTexts::address_held(
                &listed.profile,
                *ip,
                &outcome.name,
                holder,
            ));
            if let Some(profile) = &profile {
                profile
                    .held
                    .lock()
                    .unwrap()
                    .insert(*ip, (outcome.name.clone(), holder.clone()));
            }
        }
        if listed.learn.is_empty() {
            return Some(outcome);
        }
        match profile {
            Some(profile) => {
                let learned = profile.routes.lock().unwrap().learn(
                    &outcome.name,
                    &listed.learn,
                    self.clock.now(),
                );
                let refused: Vec<Ipv4Addr> = listed
                    .learn
                    .iter()
                    .filter(|ip| !learned.contains(ip))
                    .copied()
                    .collect();
                self.claims
                    .lock()
                    .unwrap()
                    .release(&listed.profile, &refused);
            }
            // Disconnected in between: nothing is routed, so nothing is held.
            None => self
                .claims
                .lock()
                .unwrap()
                .release(&listed.profile, &listed.learn),
        }
        Some(outcome)
    }

    fn flush(&self, profile: &str) {
        let log = self.log.with_profile(profile);
        match self.flusher.flush() {
            Ok(()) => log.info("the upstream answers: Windows DNS cache flushed"),
            Err(e) => log.error(format!("flushing the Windows DNS cache failed: {e}")),
        }
    }
}
