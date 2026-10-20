use std::net::Ipv4Addr;

use super::{ActiveProfile, ActiveProfiles};
use crate::ipc::{RouteGroup, RouteGroupKey, RouteRow};
use crate::list::{ListTarget, RoutesList};
use crate::net::Ipv4Net;

/// A connected profile's routes grouped by the list line that wants them: a learned
/// /32 under the most specific domain line its name matches, a static under its own line, an
/// answer another profile holds under the line of its name, then the upstream DNS /32s and the
/// refused routes. A disconnected profile has no routes.
pub struct RouteGroups<'a> {
    active: &'a ActiveProfiles,
}

impl<'a> RouteGroups<'a> {
    pub fn new(active: &'a ActiveProfiles) -> Self {
        Self { active }
    }

    pub fn of(&self, profile: &str, list: Option<&RoutesList>) -> Vec<RouteGroup> {
        let Some(active) = self.active.get(profile) else {
            return Vec::new();
        };
        let empty = RoutesList::default();
        let list = list.unwrap_or(&empty);
        let mut groups: Vec<RouteGroup> = list
            .entries
            .iter()
            .map(|entry| RouteGroup {
                key: RouteGroupKey::Line {
                    line: entry.line,
                    text: entry.target.to_string(),
                },
                rows: Vec::new(),
            })
            .collect();
        let snapshot = active.routes.lock().unwrap().snapshot();
        let idle = active.routes.lock().unwrap().idle_expiry();
        for learned in &snapshot.learned {
            if let Some(at) = Self::line_of_name(list, &learned.name) {
                groups[at].rows.push(RouteRow::Learned {
                    ip: learned.ip,
                    name: learned.name.clone(),
                    last_seen: learned.last_seen,
                    expires: learned.last_seen + idle,
                });
            }
        }
        for route in &snapshot.statics {
            if let Some(at) = Self::line_of_net(list, route.net) {
                groups[at].rows.push(RouteRow::Static { net: route.net });
            }
        }
        for (ip, name, by, by_name) in self.held(&active) {
            if let Some(at) = Self::line_of_name(list, &name) {
                groups[at].rows.push(RouteRow::Held {
                    ip,
                    name,
                    by,
                    by_name,
                });
            }
        }
        let upstream: Vec<RouteRow> = snapshot
            .upstreams
            .iter()
            .map(|route| RouteRow::Upstream {
                ip: route.net.addr(),
            })
            .collect();
        if !upstream.is_empty() {
            groups.push(RouteGroup {
                key: RouteGroupKey::Upstream,
                rows: upstream,
            });
        }
        let refused: Vec<RouteRow> = snapshot
            .refused
            .iter()
            .map(|route| RouteRow::Refused {
                net: route.net,
                name: route
                    .origin
                    .strip_prefix("learned from ")
                    .map(str::to_owned),
                reason: route.reason.clone(),
            })
            .collect();
        if !refused.is_empty() {
            groups.push(RouteGroup {
                key: RouteGroupKey::Refused,
                rows: refused,
            });
        }
        groups
    }

    /// The held answers with the name the holder learned the address for.
    fn held(&self, active: &ActiveProfile) -> Vec<(Ipv4Addr, String, String, String)> {
        let held = active.held.lock().unwrap().clone();
        held.into_iter()
            .map(|(ip, (name, by))| {
                let by_name = self
                    .active
                    .get(&by)
                    .and_then(|holder| {
                        let snapshot = holder.routes.lock().unwrap().snapshot();
                        snapshot
                            .learned
                            .into_iter()
                            .find(|learned| learned.ip == ip)
                            .map(|learned| learned.name)
                    })
                    .unwrap_or_default();
                (ip, name, by, by_name)
            })
            .collect()
    }

    /// The index of the most specific domain entry that `name` falls under.
    fn line_of_name(list: &RoutesList, name: &str) -> Option<usize> {
        let name = name.trim_end_matches('.').to_ascii_lowercase();
        list.entries
            .iter()
            .enumerate()
            .filter_map(|(at, entry)| match &entry.target {
                ListTarget::Domain(domain) => {
                    let domain = domain.as_str();
                    let under = name == domain || name.ends_with(&format!(".{domain}"));
                    under.then_some((at, domain.len()))
                }
                ListTarget::Network(_) => None,
            })
            .max_by_key(|(_, len)| *len)
            .map(|(at, _)| at)
    }

    fn line_of_net(list: &RoutesList, net: Ipv4Net) -> Option<usize> {
        list.entries
            .iter()
            .position(|entry| entry.target == ListTarget::Network(net))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_name_goes_to_its_most_specific_line() {
        let list = RoutesList::parse("corp.com\n10.1.0.0/16\na.corp.com\n");
        assert_eq!(RouteGroups::line_of_name(&list, "x.corp.com"), Some(0));
        assert_eq!(RouteGroups::line_of_name(&list, "b.a.corp.com."), Some(2));
        assert_eq!(RouteGroups::line_of_name(&list, "CORP.com"), Some(0));
        assert_eq!(RouteGroups::line_of_name(&list, "notcorp.com"), None);
        assert_eq!(
            RouteGroups::line_of_net(&list, "10.1.0.0/16".parse().unwrap()),
            Some(1)
        );
    }
}
