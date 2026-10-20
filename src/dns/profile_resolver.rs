use std::collections::HashMap;
use std::io;
use std::net::{Ipv4Addr, SocketAddr};
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, Instant, SystemTime};

use super::wire::{DnsQuery, DnsResponse, RCODE_NOERROR, TYPE_AAAA};
use super::{CacheKey, DnsCache, DnsExchange, Forwarder, ServerSet, UpstreamWarmup};
use crate::clock::Clock;
use crate::stats::{UpstreamHealth, UpstreamHealthState};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AnswerSource {
    Upstream,
    Cache,
    /// AAAA for a listed name: answered empty, because the tunnels carry no IPv6.
    NoIpv6,
}

#[derive(Debug)]
pub struct Resolution {
    pub response: DnsResponse,
    pub ips: Vec<Ipv4Addr>,
    pub source: AnswerSource,
    /// The upstream's first answer since the Connect or since it failed: clients may remember
    /// the failures, so the Windows DNS cache is flushed.
    pub recovered: bool,
}

/// One connected profile's resolver: its own cache partition, its upstream servers asked
/// through its bound exchange (with patience while the tunnel warms up), and the upstream's
/// health.
pub struct ProfileResolver {
    servers: Arc<ServerSet>,
    exchange: RwLock<Arc<dyn DnsExchange>>,
    cache: Mutex<DnsCache>,
    health: UpstreamHealth,
    warmup: Mutex<UpstreamWarmup>,
    clock: Arc<dyn Clock>,
    since: SystemTime,
}

impl ProfileResolver {
    pub const CACHE_CAPACITY: usize = 10_000;

    pub fn new(
        servers: Vec<SocketAddr>,
        exchange: Arc<dyn DnsExchange>,
        clock: Arc<dyn Clock>,
    ) -> Self {
        Self {
            servers: Arc::new(ServerSet::new(servers)),
            exchange: RwLock::new(exchange),
            cache: Mutex::new(DnsCache::new(Self::CACHE_CAPACITY)),
            health: UpstreamHealth::default(),
            warmup: Mutex::new(UpstreamWarmup::new(clock.now())),
            since: clock.now(),
            clock,
        }
    }

    /// The tunnel's handshake, as the driver reports it; the first one starts the warm-up's end.
    pub fn handshake(&self, at: SystemTime) {
        self.warmup.lock().unwrap().handshake(at);
    }

    /// Sends `query` to the upstream once, past the cache and without retries: Connect's probe,
    /// whose packet makes a peer without a keepalive handshake. An answer counts as the
    /// upstream's first; a failure is not recorded, since the tunnel may not carry it yet.
    pub fn probe(&self, query: &[u8]) -> bool {
        let exchange = self.exchange.read().unwrap().clone();
        let rtt = Instant::now();
        let answered = Forwarder::new(exchange, self.servers.clone())
            .forward(query)
            .is_ok();
        if answered {
            self.health.record_success(self.clock.now(), rtt.elapsed());
        }
        answered
    }

    /// Uses a new exchange from now on (the adapter's index changed).
    pub fn rebind(&self, exchange: Arc<dyn DnsExchange>) {
        *self.exchange.write().unwrap() = exchange;
    }

    /// The answer from the cache, else from the upstream, with TTLs capped at `max_ttl`.
    pub fn resolve(&self, query: &DnsQuery, max_ttl: u32) -> io::Result<Resolution> {
        if query.question.qtype == TYPE_AAAA {
            let empty = DnsResponse::parse(query.reply(RCODE_NOERROR)).map_err(io::Error::other)?;
            return Ok(Resolution {
                response: empty,
                ips: Vec::new(),
                source: AnswerSource::NoIpv6,
                recovered: false,
            });
        }
        let key = CacheKey::of(&query.question);
        let cached = self.cache.lock().unwrap().get(&key, self.clock.now());
        if let Some(mut hit) = cached {
            hit.response.set_id(query.id);
            return Ok(Resolution {
                response: hit.response,
                ips: hit.ips,
                source: AnswerSource::Cache,
                recovered: false,
            });
        }
        let (mut response, recovered) = self.ask_upstream(query)?;
        let ips = response.ipv4_chain(&query.question.name);
        response.cap_ttls(max_ttl);
        self.remember(key, &response, &ips);
        Ok(Resolution {
            response,
            ips,
            source: AnswerSource::Upstream,
            recovered,
        })
    }

    pub fn clear_cache(&self) -> usize {
        self.cache.lock().unwrap().clear()
    }

    pub fn cache_len(&self) -> usize {
        self.cache.lock().unwrap().len()
    }

    pub fn servers(&self) -> Vec<SocketAddr> {
        self.servers.get()
    }

    pub fn health(&self) -> UpstreamHealthState {
        self.health.snapshot()
    }

    /// The upstream failed three times in a row, or 30 s without an answer.
    pub fn unreachable(&self) -> bool {
        self.health.snapshot().unreachable(self.since)
    }

    /// The upstream's answer, and whether it is the first since the Connect or since failures.
    /// While warming up, a failed exchange is retried within the budget; only the last failure
    /// is recorded.
    fn ask_upstream(&self, query: &DnsQuery) -> io::Result<(DnsResponse, bool)> {
        let started = self.clock.now();
        loop {
            let exchange = self.exchange.read().unwrap().clone();
            let (before, rtt) = (self.clock.now(), Instant::now());
            let error = match Forwarder::new(exchange, self.servers.clone()).forward(query.raw()) {
                Ok(raw) => {
                    let response = DnsResponse::parse(raw).map_err(io::Error::other);
                    return self.record(response, rtt.elapsed());
                }
                Err(error) => error,
            };
            let now = self.clock.now();
            let took = now.duration_since(before).unwrap_or_default();
            if !self.warmup.lock().unwrap().retry(started, now, took) {
                return self.record(Err(error), rtt.elapsed());
            }
            self.clock.sleep(UpstreamWarmup::PAUSE);
        }
    }

    fn record(
        &self,
        response: io::Result<DnsResponse>,
        rtt: Duration,
    ) -> io::Result<(DnsResponse, bool)> {
        let now = self.clock.now();
        match response {
            Ok(response) => Ok((response, self.health.record_success(now, rtt))),
            Err(e) => {
                self.health.record_failure(now, e.to_string());
                Err(e)
            }
        }
    }

    fn remember(&self, key: CacheKey, response: &DnsResponse, ips: &[Ipv4Addr]) {
        let ttl = match response.min_answer_ttl() {
            Some(ttl) if response.rcode() == RCODE_NOERROR => ttl,
            _ => return,
        };
        self.cache.lock().unwrap().put(
            key,
            response.clone(),
            ips.to_vec(),
            Duration::from_secs(u64::from(ttl)),
            self.clock.now(),
        );
    }
}

/// The resolvers of the connected profiles, by profile name.
#[derive(Default)]
pub struct ResolverSet {
    resolvers: RwLock<HashMap<String, Arc<ProfileResolver>>>,
}

impl ResolverSet {
    pub fn insert(&self, profile: &str, resolver: Arc<ProfileResolver>) {
        self.resolvers
            .write()
            .unwrap()
            .insert(profile.to_owned(), resolver);
    }

    pub fn remove(&self, profile: &str) -> Option<Arc<ProfileResolver>> {
        self.resolvers.write().unwrap().remove(profile)
    }

    pub fn get(&self, profile: &str) -> Option<Arc<ProfileResolver>> {
        self.resolvers.read().unwrap().get(profile).cloned()
    }

    pub fn names(&self) -> Vec<String> {
        let mut names: Vec<String> = self.resolvers.read().unwrap().keys().cloned().collect();
        names.sort();
        names
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::clock::ManualClock;
    use crate::dns::ResponseBuilder;
    use crate::dns::wire::TYPE_A;
    use std::time::UNIX_EPOCH;

    /// An upstream answering from a table of names; records which server was asked for what
    /// through which exchange (`via`). It times out while `down`, and for the next `failing`
    /// exchanges.
    #[derive(Default)]
    pub(crate) struct Zone {
        pub(crate) a: Mutex<HashMap<String, Vec<Ipv4Addr>>>,
        pub(crate) calls: Mutex<Vec<(String, SocketAddr, String)>>,
        pub(crate) down: Mutex<bool>,
        pub(crate) failing: Mutex<u32>,
    }

    impl Zone {
        pub(crate) fn answer(&self, name: &str, ips: &[&str]) {
            let ips = ips.iter().map(|ip| ip.parse().unwrap()).collect();
            self.a.lock().unwrap().insert(name.into(), ips);
        }

        pub(crate) fn calls(&self) -> Vec<(String, SocketAddr, String)> {
            self.calls.lock().unwrap().clone()
        }
    }

    /// One exchange of the zone, named after the adapter it is bound to.
    pub(crate) struct ZoneExchange {
        pub(crate) zone: Arc<Zone>,
        pub(crate) via: String,
    }

    impl DnsExchange for ZoneExchange {
        fn exchange(&self, server: SocketAddr, query: &[u8]) -> io::Result<Vec<u8>> {
            let query = DnsQuery::parse(query).unwrap();
            let name = query.question.name.to_ascii_lowercase();
            self.zone
                .calls
                .lock()
                .unwrap()
                .push((self.via.clone(), server, name.clone()));
            if *self.zone.down.lock().unwrap() {
                return Err(io::ErrorKind::TimedOut.into());
            }
            let mut failing = self.zone.failing.lock().unwrap();
            if *failing > 0 {
                *failing -= 1;
                return Err(io::ErrorKind::TimedOut.into());
            }
            drop(failing);
            let zone = self.zone.a.lock().unwrap();
            let builder = zone
                .get(&name)
                .into_iter()
                .flatten()
                .fold(ResponseBuilder::new(&query), |b, ip| b.a(&name, *ip, 3600));
            Ok(builder.build())
        }
    }

    pub(crate) fn exchange(zone: &Arc<Zone>, via: &str) -> Arc<dyn DnsExchange> {
        Arc::new(ZoneExchange {
            zone: zone.clone(),
            via: via.into(),
        })
    }

    fn query(id: u16, name: &str, qtype: u16) -> DnsQuery {
        DnsQuery::parse(&DnsQuery::build(id, name, qtype)).unwrap()
    }

    fn rig() -> (Arc<Zone>, Arc<ManualClock>, ProfileResolver) {
        let zone = Arc::new(Zone::default());
        zone.answer("www.acme.example", &["198.51.100.14"]);
        let clock = Arc::new(ManualClock::new(UNIX_EPOCH + Duration::from_secs(1_000)));
        let resolver = ProfileResolver::new(
            vec!["10.66.0.53:53".parse().unwrap()],
            exchange(&zone, "if-1"),
            clock.clone(),
        );
        (zone, clock, resolver)
    }

    #[test]
    fn answers_from_the_upstream_then_from_the_cache_with_capped_ttls() {
        let (zone, clock, resolver) = rig();
        let first = resolver
            .resolve(&query(7, "www.acme.example", TYPE_A), 300)
            .unwrap();
        assert_eq!(first.source, AnswerSource::Upstream);
        assert_eq!(first.ips, [Ipv4Addr::new(198, 51, 100, 14)]);
        assert_eq!(first.response.answers()[0].ttl, 300);
        clock.advance(Duration::from_secs(100));
        let second = resolver
            .resolve(&query(8, "WWW.acme.example", TYPE_A), 300)
            .unwrap();
        assert_eq!(second.source, AnswerSource::Cache);
        assert_eq!(second.response.id(), 8);
        assert_eq!(second.response.answers()[0].ttl, 200);
        assert_eq!(zone.calls().len(), 1);
        assert!(resolver.health().rtt.is_some());
        assert_eq!(resolver.clear_cache(), 1);
        assert_eq!(resolver.cache_len(), 0);
    }

    #[test]
    fn aaaa_is_empty_without_asking() {
        let (zone, _, resolver) = rig();
        let answer = resolver
            .resolve(&query(1, "www.acme.example", TYPE_AAAA), 300)
            .unwrap();
        assert_eq!(answer.source, AnswerSource::NoIpv6);
        assert!(answer.response.answers().is_empty());
        assert_eq!(answer.response.rcode(), RCODE_NOERROR);
        assert!(zone.calls().is_empty());
    }

    #[test]
    fn failures_count_toward_unreachable_and_a_rebind_takes_the_new_exchange() {
        let (zone, _, resolver) = rig();
        *zone.down.lock().unwrap() = true;
        for id in 0..3 {
            assert!(
                resolver
                    .resolve(&query(id, "x.acme.example", TYPE_A), 300)
                    .is_err()
            );
        }
        assert!(resolver.unreachable());
        *zone.down.lock().unwrap() = false;
        resolver.rebind(exchange(&zone, "if-2"));
        resolver
            .resolve(&query(9, "y.acme.example", TYPE_A), 300)
            .unwrap();
        assert!(!resolver.unreachable());
        assert_eq!(zone.calls().last().unwrap().0, "if-2");
    }

    #[test]
    fn while_warming_up_a_failing_upstream_is_retried_within_the_budget() {
        let (zone, clock, resolver) = rig();
        let start = clock.now();
        *zone.failing.lock().unwrap() = 3;
        let answer = resolver
            .resolve(&query(1, "www.acme.example", TYPE_A), 300)
            .unwrap();
        assert_eq!(answer.ips, [Ipv4Addr::new(198, 51, 100, 14)]);
        assert!(answer.recovered, "the first answer since the Connect");
        assert_eq!(zone.calls().len(), 4);
        assert_eq!(clock.now(), start + UpstreamWarmup::PAUSE * 3);
        assert_eq!(resolver.health().consecutive_failures, 0);

        // Down for good: SERVFAIL once the budget is spent, one failure recorded.
        *zone.down.lock().unwrap() = true;
        let start = clock.now();
        assert!(
            resolver
                .resolve(&query(2, "x.acme.example", TYPE_A), 300)
                .is_err()
        );
        assert!(clock.now() <= start + UpstreamWarmup::BUDGET);
        assert_eq!(zone.calls().len(), 4 + 11);
        assert_eq!(resolver.health().consecutive_failures, 1);
        assert!(!resolver.unreachable());
    }

    #[test]
    fn warmed_up_a_failure_is_at_once_and_the_next_answer_recovers_once() {
        let (zone, clock, resolver) = rig();
        resolver.handshake(clock.now());
        let first = resolver
            .resolve(&query(1, "www.acme.example", TYPE_A), 300)
            .unwrap();
        assert!(first.recovered);
        clock.advance(UpstreamWarmup::GRACE);
        *zone.failing.lock().unwrap() = 1;
        let calls = zone.calls().len();
        assert!(
            resolver
                .resolve(&query(2, "x.acme.example", TYPE_A), 300)
                .is_err()
        );
        assert_eq!(zone.calls().len(), calls + 1, "not retried");
        let back = resolver
            .resolve(&query(3, "x.acme.example", TYPE_A), 300)
            .unwrap();
        assert!(back.recovered, "the first answer since the failure");
        let again = resolver
            .resolve(&query(4, "y.acme.example", TYPE_A), 300)
            .unwrap();
        assert!(!again.recovered);
    }

    #[test]
    fn a_probe_asks_once_and_only_its_answer_is_recorded() {
        let (zone, _, resolver) = rig();
        *zone.down.lock().unwrap() = true;
        let probe = DnsQuery::build(1, "acme.example", crate::dns::wire::TYPE_SOA);
        assert!(!resolver.probe(&probe));
        assert_eq!(zone.calls().len(), 1, "not retried");
        assert_eq!(resolver.health(), UpstreamHealthState::default());
        *zone.down.lock().unwrap() = false;
        assert!(resolver.probe(&probe));
        assert!(resolver.health().last_success.is_some());
        // The probe was the upstream's first answer: the next one flushes nothing.
        let answer = resolver
            .resolve(&query(2, "www.acme.example", TYPE_A), 300)
            .unwrap();
        assert!(!answer.recovered);
    }

    #[test]
    fn the_set_finds_resolvers_by_profile() {
        let (_, _, resolver) = rig();
        let set = ResolverSet::default();
        set.insert("AcmeCorp", Arc::new(resolver));
        assert!(set.get("AcmeCorp").is_some());
        assert_eq!(set.names(), ["AcmeCorp"]);
        assert!(set.remove("AcmeCorp").is_some());
        assert!(set.get("AcmeCorp").is_none());
    }
}
