//! The demo's three sample profiles, with made-up public keys.

use std::net::Ipv4Addr;
use std::time::{Duration, SystemTime};

use super::Scenario;
use super::model::{DemoChart, DemoProfile, DemoRoute};
use crate::ipc::{ProfileState, RouteRow};
use crate::net::Ipv4Net;
use crate::profile::{Endpoint, Pin, TunnelAddress};
use crate::tunnel::HandshakeHealth;

pub struct DemoProfiles;

impl DemoProfiles {
    fn ip(text: &str) -> Ipv4Addr {
        text.parse().expect("a sample address")
    }

    fn net(text: &str) -> Ipv4Net {
        text.parse().expect("a sample network")
    }

    fn ago(secs: u64) -> SystemTime {
        Scenario::now() - Duration::from_secs(secs)
    }

    fn ahead(secs: u64) -> SystemTime {
        Scenario::now() + Duration::from_secs(secs)
    }

    fn learned(line: usize, ip: &str, name: &str, ago: u64, expires: u64) -> DemoRoute {
        DemoRoute {
            line: Some(line),
            row: RouteRow::Learned {
                ip: Self::ip(ip),
                name: name.into(),
                last_seen: Self::ago(ago),
                expires: Self::ahead(expires),
            },
        }
    }

    fn statics(line: usize, net: &str) -> DemoRoute {
        DemoRoute {
            line: Some(line),
            row: RouteRow::Static {
                net: Self::net(net),
            },
        }
    }

    fn upstream(ip: &str) -> DemoRoute {
        DemoRoute {
            line: None,
            row: RouteRow::Upstream { ip: Self::ip(ip) },
        }
    }

    const H: u64 = 3600;
    const M: u64 = 60;

    pub fn contoso() -> DemoProfile {
        let day = 24 * Self::H;
        let learned = [
            (2, "192.0.2.93", "contoso.example", 12),
            (2, "192.0.2.94", "www.contoso.example", 12),
            (2, "203.0.113.7", "jira.contoso.example", Self::M),
            (2, "10.66.4.15", "intranet.contoso.example", 3 * Self::M),
            (2, "10.66.4.31", "git.contoso.example", 8 * Self::M),
            (
                2,
                "203.0.113.17",
                "confluence.contoso.example",
                14 * Self::M,
            ),
            (2, "10.66.6.8", "vault.contoso.example", 26 * Self::M),
            (
                2,
                "10.66.4.77",
                "grafana.contoso.example",
                2 * Self::H + 5 * Self::M,
            ),
            (3, "198.51.100.46", "shop-contoso.example", 22),
            (3, "198.51.100.71", "www.shop-contoso.example", 22),
            (3, "203.0.113.140", "api.shop-contoso.example", 2 * Self::M),
            (
                3,
                "198.51.100.201",
                "admin.shop-contoso.example",
                41 * Self::M,
            ),
            (
                3,
                "198.51.100.109",
                "cdn.shop-contoso.example",
                Self::H + 2 * Self::M,
            ),
            (
                3,
                "198.51.100.66",
                "mail.shop-contoso.example",
                Self::H + 40 * Self::M,
            ),
        ];
        let mut routes: Vec<DemoRoute> = learned
            .iter()
            .map(|(line, ip, name, ago)| Self::learned(*line, ip, name, *ago, day - ago))
            .collect();
        routes.push(DemoRoute {
            line: None,
            row: RouteRow::Refused {
                net: Self::net("192.168.1.20/32"),
                name: Some("printer.contoso.example".into()),
                reason: "on-link network of Ethernet 192.168.1.0/24".into(),
            },
        });
        routes.push(Self::statics(4, "10.66.0.0/16"));
        routes.push(Self::statics(5, "172.16.40.12/32"));
        routes.push(Self::upstream("10.66.0.53"));
        DemoProfile {
            name: "Contoso".into(),
            colour: Pin::new(3).expect("a pin"),
            state: ProfileState::Connected(HandshakeHealth::Ok),
            since: Some(Scenario::at(9, 14, 10)),
            uptime_secs: Some(2 * Self::H + 14 * Self::M),
            last_connected: Some(Scenario::at(9, 14, 10)),
            handshake_age_secs: Some(42),
            rx_rate: 38_000,
            tx_rate: 6_100,
            rx_total: 1_240_000_000,
            tx_total: 318_600_000,
            dns_rtt_ms: Some(18),
            address: TunnelAddress::parse("10.99.0.46/32").expect("an address"),
            mtu: 1420,
            endpoint: Endpoint::parse("vpn.contoso.example:51820").expect("an endpoint"),
            endpoint_addresses: vec![Self::ip("203.0.113.62")],
            interface_key: "Zt4bW8yC1eG5iK9mO2qS6uX0zB3dF7hJ1lN5pR9tV2w=".into(),
            peer_key: "hR7mK0vN3pQ8sT1wY4zB6cE9fH2jL5nP8rU1xA4dG7k=".into(),
            dns: vec!["10.66.0.53:53".parse().expect("a server")],
            keepalive: 25,
            imported: Scenario::at(0, 0, 0) - Duration::from_secs(5 * 86_400 - 10 * Self::H),
            list: "# Contoso\ncontoso.example\nshop-contoso.example\n10.66.0.0/16\n172.16.40.12\n"
                .into(),
            routes,
            settings: "connect_at_boot = true\n".into(),
            attempt: None,
            chart: DemoChart {
                level: 34.0,
                seed: 7,
            },
            connecting_since: None,
            traffic_from: 0,
        }
    }

    pub fn acme_corp() -> DemoProfile {
        let day = 24 * Self::H;
        let learned = [
            (1, "198.51.100.14", "acme.example", 30),
            (1, "198.51.100.15", "www.acme.example", 30),
            (1, "10.88.20.4", "jira.acme.example", 4 * Self::M),
            (1, "198.51.100.80", "sso.acme.example", 9 * Self::M),
            (2, "10.88.12.5", "intranet.acme.example", Self::M),
            (2, "10.88.12.9", "wiki.intranet.acme.example", 6 * Self::M),
            (2, "10.88.12.31", "git.intranet.acme.example", 22 * Self::M),
            (4, "192.0.2.140", "partners.contoso.example", 12 * Self::M),
        ];
        let mut routes: Vec<DemoRoute> = learned
            .iter()
            .map(|(line, ip, name, ago)| Self::learned(*line, ip, name, *ago, day - ago))
            .collect();
        routes.insert(
            4,
            DemoRoute {
                line: Some(1),
                row: RouteRow::Held {
                    ip: Self::ip("192.0.2.93"),
                    name: "cdn.acme.example".into(),
                    by: "Contoso".into(),
                    by_name: "contoso.example".into(),
                },
            },
        );
        routes.push(Self::statics(3, "172.20.0.0/16"));
        routes.push(Self::upstream("10.88.0.53"));
        DemoProfile {
            name: "AcmeCorp".into(),
            colour: Pin::new(2).expect("a pin"),
            state: ProfileState::Connected(HandshakeHealth::Ok),
            since: Some(Scenario::at(10, 41, 2)),
            uptime_secs: Some(47 * Self::M),
            last_connected: Some(Scenario::at(10, 41, 2)),
            handshake_age_secs: Some(78),
            rx_rate: 12_000,
            tx_rate: 1_700,
            rx_total: 86_400_000,
            tx_total: 12_900_000,
            dns_rtt_ms: Some(24),
            address: TunnelAddress::parse("10.88.4.12/32").expect("an address"),
            mtu: 1380,
            endpoint: Endpoint::parse("vpn.acme.example:51820").expect("an endpoint"),
            endpoint_addresses: vec![Self::ip("203.0.113.25")],
            interface_key: "Lp8sD2fH6jK0mN4qR8tV2xZ6bC0eG4iJ8lN2pR6tW0y=".into(),
            peer_key: "Qm3vT8xA2cF6hJ0lN4pR7tW1yB5dG9kM2oS6uV0zX3e=".into(),
            dns: vec!["10.88.0.53:53".parse().expect("a server")],
            keepalive: 25,
            imported: Scenario::at(0, 0, 0) - Duration::from_secs(3 * 86_400 - 9 * Self::H),
            list: "acme.example\nintranet.acme.example\n172.20.0.0/16\npartners.contoso.example\n"
                .into(),
            routes,
            settings: "connect_at_boot = false\n".into(),
            attempt: None,
            chart: DemoChart {
                level: 10.0,
                seed: 3,
            },
            connecting_since: None,
            traffic_from: 0,
        }
    }

    pub fn home_lab() -> DemoProfile {
        DemoProfile {
            name: "HomeLab".into(),
            colour: Pin::new(1).expect("a pin"),
            state: ProfileState::Blocked,
            since: None,
            uptime_secs: None,
            last_connected: Some(
                Scenario::at(0, 0, 0) - Duration::from_secs(Self::H + 50 * Self::M),
            ),
            handshake_age_secs: None,
            rx_rate: 0,
            tx_rate: 0,
            rx_total: 0,
            tx_total: 0,
            dns_rtt_ms: None,
            address: TunnelAddress::parse("10.9.0.2/32").expect("an address"),
            mtu: 1420,
            endpoint: Endpoint::parse("home.example.net:51820").expect("an endpoint"),
            endpoint_addresses: vec![Self::ip("192.0.2.40")],
            interface_key: "Ub2eH6kM0pS4vY8bD2gJ6mP0rT4wZ8cF2hK6nQ0sV4y=".into(),
            peer_key: "Vn5rX9zB3dF7hK1mP5sU9wY3aC7eG1iL5nQ9tW3xZ7c=".into(),
            dns: vec!["10.9.0.1:53".parse().expect("a server")],
            keepalive: 0,
            imported: Scenario::at(0, 0, 0) - Duration::from_secs(47 * 86_400),
            list: "lab.home.example\n".into(),
            routes: Vec::new(),
            settings: "connect_at_boot = false\nmtu = 9000\n# upstream = 10.9.0.1\n".into(),
            attempt: None,
            chart: DemoChart {
                level: 8.0,
                seed: 11,
            },
            connecting_since: None,
            traffic_from: 0,
        }
    }
}
