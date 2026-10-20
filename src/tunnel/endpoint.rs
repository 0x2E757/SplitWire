use std::collections::BTreeSet;
use std::hash::{BuildHasher, RandomState};
use std::io;
use std::net::Ipv4Addr;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use crate::dns::Forwarder;
use crate::dns::wire::{DnsQuery, DnsResponse, TYPE_A};

/// Every address any profile's endpoint resolved to in this run. Each is a never-route in every
/// tunnel: the encrypted traffic to an endpoint must never enter a tunnel itself.
#[derive(Debug, Default)]
pub struct EndpointAddresses {
    seen: Mutex<BTreeSet<Ipv4Addr>>,
}

impl EndpointAddresses {
    pub fn record(&self, addresses: &[Ipv4Addr]) {
        self.seen.lock().unwrap().extend(addresses);
    }

    pub fn all(&self) -> Vec<Ipv4Addr> {
        self.seen.lock().unwrap().iter().copied().collect()
    }
}

/// Resolves an endpoint host through the fallback DNS only, never the OS resolver or the proxy:
/// the host may lie under a listed domain, whose answers would come through a tunnel.
pub struct EndpointResolver {
    fallback: Forwarder,
    addresses: Arc<EndpointAddresses>,
}

impl EndpointResolver {
    pub fn new(fallback: Forwarder, addresses: Arc<EndpointAddresses>) -> Self {
        Self {
            fallback,
            addresses,
        }
    }

    pub fn addresses(&self) -> &Arc<EndpointAddresses> {
        &self.addresses
    }

    /// The IPv4 addresses of `host`: an IPv4 literal as is, a name by an A query to the fallback
    /// servers. Every address is recorded as a never-route before it is returned.
    pub fn resolve(&self, host: &str) -> io::Result<Vec<Ipv4Addr>> {
        let addresses = self.lookup(host)?;
        self.addresses.record(&addresses);
        Ok(addresses)
    }

    /// The addresses of `host` the same way, without recording them: an endpoint that is only
    /// being previewed for an import.
    pub fn lookup(&self, host: &str) -> io::Result<Vec<Ipv4Addr>> {
        match host.parse::<Ipv4Addr>() {
            Ok(literal) => Ok(vec![literal]),
            Err(_) => self.query(host),
        }
    }

    fn query(&self, host: &str) -> io::Result<Vec<Ipv4Addr>> {
        let id = RandomState::new().hash_one(Instant::now()) as u16;
        let reply = self.fallback.forward(&DnsQuery::build(id, host, TYPE_A))?;
        let reply =
            DnsResponse::parse(reply).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        if reply.id() != id {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("the answer for {host} has another query's ID"),
            ));
        }
        let addresses = reply.ipv4_chain(host);
        if addresses.is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                format!("{host} has no IPv4 address (rcode {})", reply.rcode()),
            ));
        }
        Ok(addresses)
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::dns::{DnsExchange, ResponseBuilder, ServerSet};
    use std::collections::HashMap;
    use std::net::SocketAddr;

    /// Answers A queries from a table of names; counts the queries.
    #[derive(Default)]
    pub(crate) struct FakeDns {
        pub(crate) answers: Mutex<HashMap<String, Vec<Ipv4Addr>>>,
        pub(crate) queries: Mutex<u32>,
    }

    impl FakeDns {
        pub(crate) fn answer(&self, name: &str, addresses: &[&str]) {
            let addresses = addresses.iter().map(|a| a.parse().unwrap()).collect();
            self.answers.lock().unwrap().insert(name.into(), addresses);
        }

        pub(crate) fn resolver(self: &Arc<Self>) -> EndpointResolver {
            let servers = Arc::new(ServerSet::new(vec!["192.0.2.53:53".parse().unwrap()]));
            let forwarder = Forwarder::new(self.clone(), servers);
            EndpointResolver::new(forwarder, Arc::default())
        }
    }

    impl DnsExchange for FakeDns {
        fn exchange(&self, _server: SocketAddr, query: &[u8]) -> io::Result<Vec<u8>> {
            *self.queries.lock().unwrap() += 1;
            let query = DnsQuery::parse(query).unwrap();
            let answers = self.answers.lock().unwrap();
            let Some(addresses) = answers.get(&query.question.name) else {
                return Err(io::ErrorKind::TimedOut.into());
            };
            let builder = addresses
                .iter()
                .fold(ResponseBuilder::new(&query), |b, ip| {
                    b.a(&query.question.name, *ip, 60)
                });
            Ok(builder.build())
        }
    }

    #[test]
    fn an_ip_literal_needs_no_query() {
        let dns = Arc::new(FakeDns::default());
        let resolver = dns.resolver();
        assert_eq!(
            resolver.resolve("198.51.100.7").unwrap(),
            [Ipv4Addr::new(198, 51, 100, 7)]
        );
        assert_eq!(*dns.queries.lock().unwrap(), 0);
        assert_eq!(resolver.addresses().all(), [Ipv4Addr::new(198, 51, 100, 7)]);
    }

    #[test]
    fn a_name_is_asked_of_the_fallback_and_every_address_is_kept() {
        let dns = Arc::new(FakeDns::default());
        dns.answer("vpn.example.com", &["203.0.113.9", "203.0.113.10"]);
        let resolver = dns.resolver();
        assert_eq!(resolver.resolve("vpn.example.com").unwrap().len(), 2);
        dns.answer("vpn.example.com", &["203.0.113.11"]);
        resolver.resolve("vpn.example.com").unwrap();
        assert_eq!(resolver.addresses().all().len(), 3);
        assert_eq!(*dns.queries.lock().unwrap(), 2);
    }

    #[test]
    fn no_answer_is_an_error_and_records_nothing() {
        let dns = Arc::new(FakeDns::default());
        dns.answer("empty.example.com", &[]);
        let resolver = dns.resolver();
        let error = resolver.resolve("empty.example.com").unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::NotFound);
        dns.answer("preview.example.com", &["203.0.113.40"]);
        assert_eq!(resolver.lookup("preview.example.com").unwrap().len(), 1);
        assert!(resolver.resolve("down.example.com").is_err());
        assert!(resolver.addresses().all().is_empty());
    }
}
