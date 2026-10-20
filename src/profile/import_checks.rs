use std::fmt;
use std::net::Ipv4Addr;

use super::{EndpointHost, ProfileFacts};
use crate::list::{ListSet, ListTarget};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum CheckLevel {
    Ok,
    Info,
    Warn,
    /// Blocks the import.
    Bad,
}

impl CheckLevel {
    pub fn as_str(self) -> &'static str {
        match self {
            CheckLevel::Ok => "ok",
            CheckLevel::Info => "info",
            CheckLevel::Warn => "warn",
            CheckLevel::Bad => "bad",
        }
    }
}

/// One line of "Checks against your other profiles".
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ImportCheck {
    pub level: CheckLevel,
    pub title: String,
    pub detail: String,
}

impl fmt::Display for ImportCheck {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.title, self.detail)
    }
}

/// Another stored profile, as the checks see it.
pub struct OtherProfile<'a> {
    pub name: &'a str,
    pub facts: &'a ProfileFacts,
}

/// Checks a profile about to be imported against the other profiles: tunnel overlap (blocks), an
/// endpoint inside another profile's networks or answers, a shared DNS server.
pub struct ImportChecks<'a> {
    pub others: Vec<OtherProfile<'a>>,
    pub lists: &'a ListSet,
    /// Addresses other profiles' DNS answered recently, `(profile, address)`.
    pub answers: &'a [(String, Ipv4Addr)],
}

impl ImportChecks<'_> {
    /// `endpoint_addresses` are what the endpoint resolved to; an IP literal needs none.
    pub fn run(
        &self,
        name: &str,
        facts: &ProfileFacts,
        endpoint_addresses: &[Ipv4Addr],
    ) -> Vec<ImportCheck> {
        if self.others.is_empty() {
            let mut checks = vec![Self::check(
                CheckLevel::Ok,
                format!("Name {name}"),
                "No other profile to compare with yet".into(),
            )];
            checks.extend(
                self.dns(facts)
                    .into_iter()
                    .filter(|c| c.level != CheckLevel::Ok),
            );
            return checks;
        }
        let mut checks = self.tunnel(facts);
        checks.extend(self.endpoint(facts, endpoint_addresses));
        checks.extend(self.dns(facts));
        checks
    }

    fn tunnel(&self, facts: &ProfileFacts) -> Vec<ImportCheck> {
        let title = format!("Tunnel {}", facts.address);
        let subnet = facts.address.subnet();
        let overlaps: Vec<ImportCheck> = self
            .others
            .iter()
            .filter(|other| other.facts.address.subnet().overlaps(&subnet))
            .map(|other| {
                Self::check(
                    CheckLevel::Bad,
                    title.clone(),
                    format!(
                        "Overlaps {}'s tunnel {}; tunnels must not overlap",
                        other.name,
                        other.facts.address.subnet()
                    ),
                )
            })
            .collect();
        if !overlaps.is_empty() {
            return overlaps;
        }
        let names: Vec<&str> = self.others.iter().map(|o| o.name).collect();
        vec![Self::check(
            CheckLevel::Ok,
            title,
            format!("No overlap with the tunnels of {}", names.join(", ")),
        )]
    }

    fn endpoint(&self, facts: &ProfileFacts, resolved: &[Ipv4Addr]) -> Vec<ImportCheck> {
        let addresses: Vec<Ipv4Addr> = facts
            .endpoint
            .literal()
            .into_iter()
            .chain(resolved.iter().copied())
            .collect();
        let shown: Vec<String> = addresses.iter().map(ToString::to_string).collect();
        let title = match (&facts.endpoint.host, shown.is_empty()) {
            (EndpointHost::Name(name), false) => format!("Endpoint {name} → {}", shown.join(", ")),
            (host, _) => format!("Endpoint {host}"),
        };
        let mut found = Vec::new();
        for other in &self.others {
            for entry in self
                .lists
                .get(other.name)
                .map_or(&[][..], |l| &l.entries[..])
            {
                match &entry.target {
                    ListTarget::Network(net) if addresses.iter().any(|ip| net.contains(*ip)) => {
                        found.push(Self::check(
                            CheckLevel::Warn,
                            title.clone(),
                            format!(
                                "Inside {} line {} ({net}): that route is refused while this profile is connected",
                                other.name, entry.line
                            ),
                        ));
                    }
                    ListTarget::Domain(domain)
                        if Self::host_under(&facts.endpoint.host, domain.as_str()) =>
                    {
                        found.push(Self::check(
                            CheckLevel::Info,
                            title.clone(),
                            format!(
                                "Listed by {} line {} ({domain}); the endpoint is still resolved through the fallback DNS",
                                other.name, entry.line
                            ),
                        ));
                    }
                    _ => {}
                }
            }
        }
        for (profile, address) in self.answers {
            if addresses.contains(address) && self.others.iter().any(|o| o.name == profile) {
                found.push(Self::check(
                    CheckLevel::Warn,
                    title.clone(),
                    format!("{profile}'s DNS answered {address}: that route is refused while this profile is connected"),
                ));
            }
        }
        if found.is_empty() {
            found.push(Self::check(
                CheckLevel::Ok,
                title,
                "Outside every other profile's networks and answers".into(),
            ));
        }
        found
    }

    fn dns(&self, facts: &ProfileFacts) -> Vec<ImportCheck> {
        if facts.dns.is_empty() {
            return vec![Self::check(
                CheckLevel::Info,
                "No DNS server".into(),
                "Set Upstream DNS in the profile settings before connecting".into(),
            )];
        }
        let shared: Vec<ImportCheck> = facts
            .dns
            .iter()
            .filter_map(|server| {
                let users: Vec<&str> = self
                    .others
                    .iter()
                    .filter(|o| o.facts.dns.contains(server))
                    .map(|o| o.name)
                    .collect();
                (!users.is_empty()).then(|| {
                    Self::check(
                        CheckLevel::Info,
                        format!("DNS {server}"),
                        format!(
                            "Also used by {}; each profile asks it through its own adapter",
                            users.join(", ")
                        ),
                    )
                })
            })
            .collect();
        if !shared.is_empty() {
            return shared;
        }
        let servers: Vec<String> = facts.dns.iter().map(ToString::to_string).collect();
        vec![Self::check(
            CheckLevel::Ok,
            format!("DNS {}", servers.join(", ")),
            "Not used by another profile".into(),
        )]
    }

    fn host_under(host: &EndpointHost, domain: &str) -> bool {
        let EndpointHost::Name(name) = host else {
            return false;
        };
        let name = name.as_str();
        name == domain
            || name
                .strip_suffix(domain)
                .is_some_and(|head| head.ends_with('.'))
    }

    fn check(level: CheckLevel, title: String, detail: String) -> ImportCheck {
        ImportCheck {
            level,
            title,
            detail,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profile::fixtures::sample_profile;
    use crate::profile::{Endpoint, TunnelAddress};

    fn facts(address: &str, dns: &[&str], endpoint: &str) -> ProfileFacts {
        let mut facts = sample_profile().facts;
        facts.address = TunnelAddress::parse(address).unwrap();
        facts.dns = dns.iter().map(|d| d.parse().unwrap()).collect();
        facts.endpoint = Endpoint::parse(endpoint).unwrap();
        facts
    }

    fn levels(checks: &[ImportCheck]) -> Vec<(CheckLevel, String)> {
        checks.iter().map(|c| (c.level, c.to_string())).collect()
    }

    #[test]
    fn the_first_profile_has_nothing_to_compare_with() {
        let lists = ListSet::default();
        let checks = ImportChecks {
            others: Vec::new(),
            lists: &lists,
            answers: &[],
        };
        let new = facts("10.77.0.5/32", &[], "vpn.partner.example:51820");
        assert_eq!(
            levels(&checks.run("Partner", &new, &[])),
            [
                (
                    CheckLevel::Ok,
                    "Name Partner: No other profile to compare with yet".into()
                ),
                (
                    CheckLevel::Info,
                    "No DNS server: Set Upstream DNS in the profile settings before connecting"
                        .into()
                ),
            ]
        );
    }

    #[test]
    fn a_clean_profile_passes_every_check() {
        let acme = facts("10.88.4.12/24", &["10.66.0.53"], "198.51.100.7:51820");
        let lists = ListSet::load([("AcmeCorp", "corp.com\n10.0.0.0/8\n")]);
        let checks = ImportChecks {
            others: vec![OtherProfile {
                name: "AcmeCorp",
                facts: &acme,
            }],
            lists: &lists,
            answers: &[],
        };
        let new = facts("10.77.0.5/32", &["10.77.0.53"], "vpn.partner.example:51820");
        let ip = Ipv4Addr::new(203, 0, 113, 80);
        assert_eq!(
            levels(&checks.run("Partner", &new, &[ip])),
            [
                (CheckLevel::Ok, "Tunnel 10.77.0.5/32: No overlap with the tunnels of AcmeCorp".into()),
                (
                    CheckLevel::Ok,
                    "Endpoint vpn.partner.example → 203.0.113.80: Outside every other profile's networks and answers".into()
                ),
                (CheckLevel::Ok, "DNS 10.77.0.53: Not used by another profile".into()),
            ]
        );
    }

    #[test]
    fn overlaps_block_and_shared_resources_are_reported() {
        let acme = facts("10.88.4.12/24", &["10.66.0.53"], "198.51.100.7:51820");
        let lists = ListSet::load([("AcmeCorp", "example.net\n203.0.113.0/24\n")]);
        let answers = [("AcmeCorp".to_owned(), Ipv4Addr::new(203, 0, 113, 80))];
        let checks = ImportChecks {
            others: vec![OtherProfile {
                name: "AcmeCorp",
                facts: &acme,
            }],
            lists: &lists,
            answers: &answers,
        };
        let new = facts("10.88.4.200/32", &["10.66.0.53"], "vpn.example.net:51820");
        let found = checks.run("Partner", &new, &[Ipv4Addr::new(203, 0, 113, 80)]);
        assert_eq!(
            levels(&found),
            [
                (
                    CheckLevel::Bad,
                    "Tunnel 10.88.4.200/32: Overlaps AcmeCorp's tunnel 10.88.4.0/24; tunnels must not overlap".into()
                ),
                (
                    CheckLevel::Info,
                    "Endpoint vpn.example.net → 203.0.113.80: Listed by AcmeCorp line 1 (example.net); the endpoint is still resolved through the fallback DNS".into()
                ),
                (
                    CheckLevel::Warn,
                    "Endpoint vpn.example.net → 203.0.113.80: Inside AcmeCorp line 2 (203.0.113.0/24): that route is refused while this profile is connected".into()
                ),
                (
                    CheckLevel::Warn,
                    "Endpoint vpn.example.net → 203.0.113.80: AcmeCorp's DNS answered 203.0.113.80: that route is refused while this profile is connected".into()
                ),
                (
                    CheckLevel::Info,
                    "DNS 10.66.0.53: Also used by AcmeCorp; each profile asks it through its own adapter".into()
                ),
            ]
        );
    }
}
