use std::net::Ipv4Addr;

use crate::net::{Ipv4Net, OnLinkNetwork};

/// The never-routes of one profile: anything wider than /8 (the default route included),
/// loopback, link-local, multicast, broadcast and `0/8`, the networks attached to other
/// interfaces, plus what the caller adds (the fallback DNS, every endpoint, the other profiles'
/// tunnel subnets).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RouteGuard {
    forbidden: Vec<(Ipv4Net, String)>,
}

impl RouteGuard {
    pub const MIN_PREFIX: u8 = 8;

    pub fn new(on_link: &[OnLinkNetwork]) -> Self {
        let mut forbidden: Vec<(Ipv4Net, String)> = [
            ("0.0.0.0/8", "the 'this network' block"),
            ("127.0.0.0/8", "loopback"),
            ("169.254.0.0/16", "link-local"),
            ("224.0.0.0/4", "multicast"),
            ("255.255.255.255/32", "broadcast"),
        ]
        .iter()
        .map(|(net, why)| (net.parse().expect("valid constant"), (*why).to_owned()))
        .collect();
        forbidden.extend(on_link.iter().map(|link| {
            (
                link.net,
                format!("on-link network {} of '{}'", link.net, link.interface),
            )
        }));
        Self { forbidden }
    }

    /// Also refuses routes covering `hosts` (the fallback DNS servers, the endpoints), which
    /// must stay reachable outside every tunnel.
    pub fn forbid_hosts(self, hosts: &[Ipv4Addr], why: &str) -> Self {
        let nets: Vec<Ipv4Net> = hosts.iter().map(|ip| Ipv4Net::host(*ip)).collect();
        self.forbid_networks(&nets, why)
    }

    /// Also refuses routes overlapping `nets` (another profile's tunnel subnet).
    pub fn forbid_networks(mut self, nets: &[Ipv4Net], why: &str) -> Self {
        self.forbidden
            .extend(nets.iter().map(|net| (*net, why.to_owned())));
        self
    }

    /// `Err(reason)` when `net` must not go into the tunnel.
    pub fn check(&self, net: &Ipv4Net) -> Result<(), String> {
        if net.prefix() < Self::MIN_PREFIX {
            return Err(format!(
                "wider than /{} (default route or too broad)",
                Self::MIN_PREFIX
            ));
        }
        match self.forbidden.iter().find(|(bad, _)| bad.overlaps(net)) {
            Some((_, why)) => Err(format!("overlaps {why}")),
            None => Ok(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn net(text: &str) -> Ipv4Net {
        text.parse().unwrap()
    }

    fn guard() -> RouteGuard {
        RouteGuard::new(&[OnLinkNetwork {
            net: net("192.168.1.0/24"),
            interface: "Ethernet".into(),
        }])
        .forbid_hosts(&[Ipv4Addr::new(192, 0, 2, 93)], "the endpoint of Contoso")
        .forbid_networks(&[net("10.88.0.0/16")], "the tunnel of AcmeCorp")
    }

    fn refusal(text: &str) -> String {
        guard().check(&net(text)).unwrap_err()
    }

    #[test]
    fn allows_ordinary_hosts_and_networks() {
        let guard = guard();
        assert!(guard.check(&net("10.66.0.53")).is_ok());
        assert!(guard.check(&net("10.66.0.0/16")).is_ok());
        assert!(guard.check(&net("34.1.2.3")).is_ok());
    }

    #[test]
    fn refuses_default_and_too_broad_routes() {
        assert!(refusal("0.0.0.0/0").contains("wider than /8"));
        assert!(refusal("128.0.0.0/1").contains("wider than /8"));
        assert!(refusal("10.0.0.0/7").contains("wider than /8"));
    }

    #[test]
    fn refuses_an_endpoint_and_networks_covering_it() {
        assert!(refusal("192.0.2.93").contains("endpoint of Contoso"));
        assert!(refusal("192.0.0.0/16").contains("endpoint of Contoso"));
    }

    #[test]
    fn refuses_another_profiles_tunnel_both_inside_and_covering() {
        assert!(refusal("10.88.4.12").contains("tunnel of AcmeCorp"));
        assert!(refusal("10.0.0.0/8").contains("tunnel of AcmeCorp"));
    }

    #[test]
    fn refuses_special_ranges() {
        assert!(refusal("127.0.0.53").contains("loopback"));
        assert!(refusal("169.254.10.1").contains("link-local"));
        assert!(refusal("239.1.2.3").contains("multicast"));
        assert!(refusal("255.255.255.255").contains("broadcast"));
        assert!(refusal("0.1.2.3").contains("this network"));
    }

    #[test]
    fn refuses_extra_hosts_such_as_the_fallback_dns() {
        let guard = guard().forbid_hosts(&[Ipv4Addr::new(8, 8, 8, 8)], "the fallback DNS");
        assert!(
            guard
                .check(&net("8.8.8.8"))
                .unwrap_err()
                .contains("fallback DNS")
        );
        assert!(guard.check(&net("8.0.0.0/8")).is_err());
        assert!(guard.check(&net("8.8.4.4")).is_ok());
    }

    #[test]
    fn refuses_on_link_networks_both_inside_and_covering() {
        assert!(refusal("192.168.1.20").contains("'Ethernet'"));
        assert!(refusal("192.168.0.0/16").contains("on-link"));
        assert!(guard().check(&net("192.168.2.1")).is_ok());
    }
}
