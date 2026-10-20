use std::collections::BTreeMap;
use std::net::{Ipv4Addr, SocketAddr};
use std::sync::{Arc, Mutex};

use crate::dns::ServerSet;
use crate::logs::Log;
use crate::net::{Ipv4Net, NetworkProbe, OnLinkNetwork};
use crate::routes::RouteGuard;
use crate::tunnel::EndpointAddresses;

/// What every profile's never-routes are made of: the machine's other networks and
/// the fallback DNS (refreshed from the probe), every endpoint address seen in this run, and the
/// tunnel subnets of the profiles connected or connecting. It also keeps the fallback servers.
pub struct GuardSource {
    probe: Arc<dyn NetworkProbe>,
    fallback: Arc<ServerSet>,
    configured: Mutex<Vec<Ipv4Addr>>,
    endpoints: Arc<EndpointAddresses>,
    on_link: Mutex<Vec<OnLinkNetwork>>,
    tunnels: Mutex<BTreeMap<String, Ipv4Net>>,
    log: Log,
}

impl GuardSource {
    pub fn new(
        probe: Arc<dyn NetworkProbe>,
        fallback: Arc<ServerSet>,
        endpoints: Arc<EndpointAddresses>,
        log: Log,
    ) -> Self {
        Self {
            probe,
            fallback,
            configured: Mutex::new(Vec::new()),
            endpoints,
            on_link: Mutex::new(Vec::new()),
            tunnels: Mutex::new(BTreeMap::new()),
            log: log.with_source("guard"),
        }
    }

    /// The `fallback` setting; empty follows the default-route interface's DNS.
    pub fn set_configured_fallback(&self, servers: Vec<Ipv4Addr>) {
        *self.configured.lock().unwrap() = servers;
    }

    /// Re-reads the fallback servers and the on-link networks. A failed read keeps the last ones.
    pub fn refresh(&self) {
        self.refresh_fallback();
        match self.probe.on_link_networks() {
            Ok(networks) => *self.on_link.lock().unwrap() = networks,
            Err(e) => self.log.error(format!(
                "listing the on-link networks failed (keeping the last ones): {e}"
            )),
        }
    }

    pub fn add_tunnel(&self, profile: &str, subnet: Ipv4Net) {
        self.tunnels
            .lock()
            .unwrap()
            .insert(profile.to_owned(), subnet);
    }

    pub fn remove_tunnel(&self, profile: &str) {
        self.tunnels.lock().unwrap().remove(profile);
    }

    /// The never-routes of `profile`.
    pub fn guard_for(&self, profile: &str) -> RouteGuard {
        let fallback: Vec<Ipv4Addr> = self.fallback_ips();
        let mut guard = RouteGuard::new(&self.on_link.lock().unwrap())
            .forbid_hosts(&fallback, "the fallback DNS")
            .forbid_hosts(&self.endpoints.all(), "an endpoint");
        for (other, subnet) in self.tunnels.lock().unwrap().iter() {
            if other != profile {
                guard = guard.forbid_networks(&[*subnet], &format!("the tunnel of {other}"));
            }
        }
        guard
    }

    pub fn fallback_servers(&self) -> Vec<SocketAddr> {
        self.fallback.get()
    }

    fn fallback_ips(&self) -> Vec<Ipv4Addr> {
        self.fallback
            .get()
            .iter()
            .filter_map(|server| match server {
                SocketAddr::V4(v4) => Some(*v4.ip()),
                SocketAddr::V6(_) => None,
            })
            .collect()
    }

    fn refresh_fallback(&self) {
        let configured = self.configured.lock().unwrap().clone();
        let ips = if configured.is_empty() {
            match self.probe.default_route_dns() {
                Ok(found) => found,
                Err(e) => {
                    self.log
                        .error(format!("finding the default-route DNS servers failed: {e}"));
                    return;
                }
            }
        } else {
            configured
        };
        let servers: Vec<SocketAddr> = ips.iter().map(|ip| SocketAddr::from((*ip, 53))).collect();
        if servers != self.fallback.get() {
            if servers.is_empty() {
                self.log.warn(
                    "no fallback DNS server known: endpoints, bypass and unlisted names fail",
                );
            } else {
                let shown: Vec<String> = servers.iter().map(ToString::to_string).collect();
                self.log.info(format!("fallback DNS: {}", shown.join(", ")));
            }
            self.fallback.replace(servers);
        }
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::clock::ManualClock;
    use std::io;
    use std::time::UNIX_EPOCH;

    /// A machine with one LAN and its router as DNS.
    pub(crate) struct FakeProbe;

    impl NetworkProbe for FakeProbe {
        fn on_link_networks(&self) -> io::Result<Vec<OnLinkNetwork>> {
            Ok(vec![OnLinkNetwork {
                net: "192.168.1.0/24".parse().unwrap(),
                interface: "Ethernet".into(),
            }])
        }

        fn default_route_dns(&self) -> io::Result<Vec<Ipv4Addr>> {
            Ok(vec![Ipv4Addr::new(192, 168, 1, 1)])
        }
    }

    fn net(text: &str) -> Ipv4Net {
        text.parse().unwrap()
    }

    #[test]
    fn every_profile_avoids_the_others_tunnels_and_every_endpoint() {
        let endpoints = Arc::new(EndpointAddresses::default());
        endpoints.record(&[Ipv4Addr::new(192, 0, 2, 93)]);
        let log = Log::new(1 << 16, Arc::new(ManualClock::new(UNIX_EPOCH)), false);
        let source = GuardSource::new(
            Arc::new(FakeProbe),
            Arc::new(ServerSet::default()),
            endpoints,
            log,
        );
        source.refresh();
        assert_eq!(
            source.fallback_servers(),
            ["192.168.1.1:53".parse().unwrap()]
        );
        source.add_tunnel("AcmeCorp", net("10.88.4.0/24"));
        source.add_tunnel("Contoso", net("10.77.0.0/16"));
        let acme = source.guard_for("AcmeCorp");
        assert!(acme.check(&net("10.88.4.20")).is_ok());
        assert!(
            acme.check(&net("10.77.0.9"))
                .unwrap_err()
                .contains("Contoso")
        );
        assert!(
            acme.check(&net("192.0.2.93"))
                .unwrap_err()
                .contains("endpoint")
        );
        assert!(acme.check(&net("192.168.1.1")).is_err());
        source.remove_tunnel("Contoso");
        assert!(
            source
                .guard_for("AcmeCorp")
                .check(&net("10.77.0.9"))
                .is_ok()
        );
        source.set_configured_fallback(vec![Ipv4Addr::new(9, 9, 9, 9)]);
        source.refresh();
        assert!(source.guard_for("AcmeCorp").check(&net("9.9.9.9")).is_err());
    }
}
