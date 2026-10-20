use std::net::Ipv4Addr;
use std::sync::{Arc, Mutex};

use super::wire::{DnsQuery, DnsResponse, RCODE_SERVFAIL, TYPE_A, TYPE_AAAA, TYPE_CNAME};
use super::{AnswerSource, Forwarder, ResolverSet, SharedPolicy};
use crate::list::SharedProfileMatcher;
use crate::logs::Log;
use crate::routes::HostClaims;
use crate::stats::QueryStats;

/// What a listed answer asks of its profile before it is sent.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ListedAnswer {
    pub profile: String,
    /// The addresses to route on the profile's adapter (claimed for it).
    pub learn: Vec<Ipv4Addr>,
    /// Addresses another profile holds, with that profile: never routed here.
    pub held: Vec<(Ipv4Addr, String)>,
    /// The profile's upstream answered for the first time since the Connect or since it failed.
    pub recovered: bool,
}

/// The reply to one query, plus what to route before it leaves.
#[derive(Debug)]
pub struct QueryOutcome {
    pub query: DnsQuery,
    pub response: Vec<u8>,
    /// The queried name, lower-case, without the trailing dot.
    pub name: String,
    pub listed: Option<ListedAnswer>,
}

impl QueryOutcome {
    /// The reply for a UDP client: an empty truncated one when it does not fit, so the client
    /// retries over TCP.
    pub fn udp_payload(&self) -> Vec<u8> {
        if self.response.len() > self.query.udp_limit() {
            self.query.truncated_reply()
        } else {
            self.response.clone()
        }
    }
}

/// Where a query goes: bypass names and endpoint hosts to the fallback; a listed
/// name to its profile's resolver, then the claims check; any other name to the fallback.
pub struct QueryHandler {
    matcher: Arc<SharedProfileMatcher>,
    resolvers: Arc<ResolverSet>,
    policy: Arc<SharedPolicy>,
    fallback: Forwarder,
    claims: Arc<Mutex<HostClaims>>,
    stats: Arc<QueryStats>,
    log: Log,
}

/// The shared parts a handler reads.
pub struct QueryHandlerParts {
    pub matcher: Arc<SharedProfileMatcher>,
    pub resolvers: Arc<ResolverSet>,
    pub policy: Arc<SharedPolicy>,
    pub fallback: Forwarder,
    pub claims: Arc<Mutex<HostClaims>>,
    pub stats: Arc<QueryStats>,
}

impl QueryHandler {
    pub fn new(parts: QueryHandlerParts, log: Log) -> Self {
        Self {
            matcher: parts.matcher,
            resolvers: parts.resolvers,
            policy: parts.policy,
            fallback: parts.fallback,
            claims: parts.claims,
            stats: parts.stats,
            log: log.with_source("dns"),
        }
    }

    /// `None` for a message that is not a query (it is dropped).
    pub fn handle(&self, raw: &[u8]) -> Option<QueryOutcome> {
        let query = DnsQuery::parse(raw).ok()?;
        let name = query
            .question
            .name
            .trim_end_matches('.')
            .to_ascii_lowercase();
        let label = format!("{} {name}", type_name(query.question.qtype));
        let policy = self.policy.current();
        if policy.bypass.matches(&name) {
            self.stats.count_bypassed();
            let response = self.via_fallback(&query, &label, "bypass");
            return Some(Self::outcome(query, response, name, None));
        }
        let matcher = self.matcher.current();
        let Some((profile, domain)) = matcher.owner(&name) else {
            self.stats.count_other();
            let response = self.via_fallback(&query, &label, "not listed");
            return Some(Self::outcome(query, response, name, None));
        };
        self.stats.count_listed();
        let log = self.log.with_profile(profile);
        let Some(resolver) = self.resolvers.get(profile) else {
            self.stats.count_failed();
            log.warn(format!(
                "{label}: listed ({domain}), but no resolver; SERVFAIL"
            ));
            let response = query.reply(RCODE_SERVFAIL);
            return Some(Self::outcome(query, response, name, None));
        };
        let resolution = match resolver.resolve(&query, policy.max_ttl) {
            Ok(resolution) => resolution,
            Err(e) => {
                self.stats.count_failed();
                log.warn(format!(
                    "{label}: listed ({domain}), upstream failed: {e}; SERVFAIL"
                ));
                let response = query.reply(RCODE_SERVFAIL);
                return Some(Self::outcome(query, response, name, None));
            }
        };
        if resolution.source == AnswerSource::Cache {
            self.stats.count_cache_hit();
        }
        let split = self.claims.lock().unwrap().claim(profile, &resolution.ips);
        let response = Self::without_held(resolution.response, &split.mine, &split.held);
        log.info(format!(
            "{label}: listed ({domain}) -> {}",
            Self::describe(&split.mine, &split.held, resolution.source)
        ));
        let listed = ListedAnswer {
            profile: profile.to_owned(),
            learn: split.mine,
            held: split.held,
            recovered: resolution.recovered,
        };
        Some(Self::outcome(query, response, name, Some(listed)))
    }

    /// A held address leaves the answer when other addresses remain; otherwise the
    /// answer is kept and that traffic goes through the holder.
    fn without_held(
        response: DnsResponse,
        mine: &[Ipv4Addr],
        held: &[(Ipv4Addr, String)],
    ) -> Vec<u8> {
        if held.is_empty() || mine.is_empty() {
            return response.into_bytes();
        }
        let dropped: Vec<Ipv4Addr> = held.iter().map(|(ip, _)| *ip).collect();
        match response.without_addresses(&dropped) {
            Ok(filtered) => filtered.into_bytes(),
            Err(_) => response.into_bytes(),
        }
    }

    fn via_fallback(&self, query: &DnsQuery, label: &str, why: &str) -> Vec<u8> {
        match self.fallback.forward(query.raw()) {
            Ok(response) => {
                self.log
                    .info(format!("{label}: {why}, sent to the fallback DNS"));
                response
            }
            Err(e) => {
                self.stats.count_failed();
                self.log.warn(format!(
                    "{label}: {why}, fallback DNS failed: {e}; SERVFAIL"
                ));
                query.reply(RCODE_SERVFAIL)
            }
        }
    }

    fn outcome(
        query: DnsQuery,
        response: Vec<u8>,
        name: String,
        listed: Option<ListedAnswer>,
    ) -> QueryOutcome {
        QueryOutcome {
            query,
            response,
            name,
            listed,
        }
    }

    fn describe(mine: &[Ipv4Addr], held: &[(Ipv4Addr, String)], source: AnswerSource) -> String {
        if source == AnswerSource::NoIpv6 {
            return "empty answer (no IPv6 in the tunnels)".into();
        }
        let mut parts: Vec<String> = mine.iter().map(ToString::to_string).collect();
        parts.extend(held.iter().map(|(ip, by)| format!("{ip} (held by {by})")));
        let text = if parts.is_empty() {
            "no IPv4".to_owned()
        } else {
            parts.join(", ")
        };
        match source {
            AnswerSource::Cache => format!("{text} (cache)"),
            _ => text,
        }
    }
}

fn type_name(qtype: u16) -> String {
    match qtype {
        TYPE_A => "A".into(),
        TYPE_AAAA => "AAAA".into(),
        TYPE_CNAME => "CNAME".into(),
        other => format!("TYPE{other}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::clock::ManualClock;
    use crate::dns::profile_resolver::tests::{Zone, exchange};
    use crate::dns::{DnsPolicy, ProfileResolver, ServerSet};
    use crate::list::{DomainName, ListSet, ProfileMatcher};
    use std::time::{Duration, UNIX_EPOCH};

    const UPSTREAM: &str = "10.66.0.53:53";

    struct Rig {
        zone: Arc<Zone>,
        handler: QueryHandler,
        claims: Arc<Mutex<HostClaims>>,
        stats: Arc<QueryStats>,
        matcher: Arc<SharedProfileMatcher>,
    }

    /// Two connected profiles with the same upstream address, each through its own adapter.
    fn rig() -> Rig {
        let zone = Arc::new(Zone::default());
        zone.answer("www.acme.example", &["198.51.100.14"]);
        zone.answer("cdn.acme.example", &["192.0.2.93", "192.0.2.94"]);
        zone.answer("solo.acme.example", &["192.0.2.93"]);
        zone.answer("www.contoso.example", &["192.0.2.93"]);
        zone.answer("x.partners.contoso.example", &["192.0.2.140"]);
        zone.answer("vpn.acme.example", &["203.0.113.9"]);
        zone.answer("example.org", &["203.0.113.14"]);
        let clock = Arc::new(ManualClock::new(UNIX_EPOCH + Duration::from_secs(1_000)));
        let resolvers = Arc::new(ResolverSet::default());
        for (profile, via) in [
            ("AcmeCorp", "SplitWire-AcmeCorp"),
            ("Contoso", "SplitWire-Contoso"),
        ] {
            let resolver = ProfileResolver::new(
                vec![UPSTREAM.parse().unwrap()],
                exchange(&zone, via),
                clock.clone(),
            );
            resolvers.insert(profile, Arc::new(resolver));
        }
        let lists = ListSet::load([
            ("AcmeCorp", "acme.example\npartners.contoso.example\n"),
            ("Contoso", "contoso.example\n"),
        ]);
        let matcher = Arc::new(SharedProfileMatcher::default());
        matcher.replace(ProfileMatcher::build(
            &lists,
            &["AcmeCorp".into(), "Contoso".into()],
        ));
        let policy = Arc::new(SharedPolicy::default());
        policy.replace(DnsPolicy::new(
            &[],
            &[DomainName::parse("vpn.acme.example").unwrap()],
            300,
        ));
        let fallback = Forwarder::new(
            exchange(&zone, "fallback"),
            Arc::new(ServerSet::new(vec!["192.168.1.1:53".parse().unwrap()])),
        );
        let claims = Arc::new(Mutex::new(HostClaims::default()));
        let stats = Arc::new(QueryStats::default());
        let handler = QueryHandler::new(
            QueryHandlerParts {
                matcher: matcher.clone(),
                resolvers,
                policy,
                fallback,
                claims: claims.clone(),
                stats: stats.clone(),
            },
            Log::new(1 << 20, clock, false),
        );
        Rig {
            zone,
            handler,
            claims,
            stats,
            matcher,
        }
    }

    fn ask(rig: &Rig, name: &str, qtype: u16) -> (QueryOutcome, DnsResponse) {
        let outcome = rig
            .handler
            .handle(&DnsQuery::build(77, name, qtype))
            .unwrap();
        let response = DnsResponse::parse(outcome.response.clone()).unwrap();
        assert_eq!(response.id(), 77);
        (outcome, response)
    }

    fn ip(text: &str) -> Ipv4Addr {
        text.parse().unwrap()
    }

    #[test]
    fn a_listed_name_is_asked_through_its_profiles_bound_exchange() {
        let rig = rig();
        let (outcome, _) = ask(&rig, "WWW.Acme.example.", TYPE_A);
        let listed = outcome.listed.unwrap();
        assert_eq!(listed.profile, "AcmeCorp");
        assert_eq!(listed.learn, [ip("198.51.100.14")]);
        let (outcome, _) = ask(&rig, "x.partners.contoso.example", TYPE_A);
        assert_eq!(outcome.listed.unwrap().profile, "AcmeCorp");
        let (outcome, _) = ask(&rig, "www.contoso.example", TYPE_A);
        assert_eq!(outcome.listed.unwrap().profile, "Contoso");
        let vias: Vec<(String, String)> = rig
            .zone
            .calls()
            .into_iter()
            .map(|(via, _, name)| (via, name))
            .collect();
        assert_eq!(
            vias,
            [
                ("SplitWire-AcmeCorp".into(), "www.acme.example".into()),
                (
                    "SplitWire-AcmeCorp".into(),
                    "x.partners.contoso.example".into()
                ),
                ("SplitWire-Contoso".into(), "www.contoso.example".into()),
            ]
        );
        assert_eq!(rig.stats.snapshot().listed, 3);
    }

    #[test]
    fn a_held_address_leaves_the_answer_when_others_remain() {
        let rig = rig();
        ask(&rig, "www.contoso.example", TYPE_A);
        let (outcome, response) = ask(&rig, "cdn.acme.example", TYPE_A);
        let listed = outcome.listed.unwrap();
        assert_eq!(listed.learn, [ip("192.0.2.94")]);
        assert_eq!(listed.held, [(ip("192.0.2.93"), "Contoso".to_owned())]);
        assert_eq!(response.ipv4_chain("cdn.acme.example"), [ip("192.0.2.94")]);
        assert_eq!(
            rig.claims.lock().unwrap().holder(ip("192.0.2.93")),
            Some("Contoso")
        );
    }

    #[test]
    fn an_answer_of_only_held_addresses_is_kept_and_routes_nothing() {
        let rig = rig();
        ask(&rig, "www.contoso.example", TYPE_A);
        let (outcome, response) = ask(&rig, "solo.acme.example", TYPE_A);
        let listed = outcome.listed.unwrap();
        assert!(listed.learn.is_empty());
        assert_eq!(listed.held.len(), 1);
        assert_eq!(response.ipv4_chain("solo.acme.example"), [ip("192.0.2.93")]);
    }

    #[test]
    fn bypass_endpoint_and_unlisted_names_go_to_the_fallback() {
        let rig = rig();
        let (outcome, _) = ask(&rig, "vpn.acme.example", TYPE_A);
        assert!(outcome.listed.is_none());
        let (outcome, _) = ask(&rig, "example.org", TYPE_A);
        assert!(outcome.listed.is_none());
        let vias: Vec<String> = rig
            .zone
            .calls()
            .into_iter()
            .map(|(via, _, _)| via)
            .collect();
        assert_eq!(vias, ["fallback", "fallback"]);
        let counters = rig.stats.snapshot();
        assert_eq!((counters.bypassed, counters.other), (1, 1));
    }

    #[test]
    fn a_disconnected_profiles_names_fall_through_to_the_next() {
        let rig = rig();
        let lists = ListSet::load([
            ("AcmeCorp", "acme.example\npartners.contoso.example\n"),
            ("Contoso", "contoso.example\n"),
        ]);
        rig.matcher
            .replace(ProfileMatcher::build(&lists, &["Contoso".into()]));
        let (outcome, _) = ask(&rig, "x.partners.contoso.example", TYPE_A);
        assert_eq!(outcome.listed.unwrap().profile, "Contoso");
        let (outcome, _) = ask(&rig, "www.acme.example", TYPE_A);
        assert!(outcome.listed.is_none());
    }

    #[test]
    fn aaaa_is_empty_failures_are_servfail_and_garbage_is_dropped() {
        let rig = rig();
        let (outcome, response) = ask(&rig, "www.acme.example", TYPE_AAAA);
        assert!(response.answers().is_empty());
        assert!(outcome.listed.unwrap().learn.is_empty());
        *rig.zone.down.lock().unwrap() = true;
        let (outcome, response) = ask(&rig, "new.acme.example", TYPE_A);
        assert_eq!(response.rcode(), RCODE_SERVFAIL);
        assert!(outcome.listed.is_none());
        assert!(rig.handler.handle(&[0, 1, 2]).is_none());
        let (mut outcome, _) = ask(&rig, "www.contoso.example", TYPE_AAAA);
        outcome.response.resize(600, 0);
        assert!(
            DnsResponse::parse(outcome.udp_payload())
                .unwrap()
                .truncated()
        );
    }
}
