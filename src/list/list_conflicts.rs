use std::net::Ipv4Addr;

use super::{ListSet, ListTarget};
use crate::net::Ipv4Net;

/// What a profile's entries may not route: its endpoint addresses (never routed by any tunnel) and
/// its tunnel subnet (never routed by another tunnel).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProfileFootprint {
    pub profile: String,
    pub endpoints: Vec<Ipv4Addr>,
    pub tunnel: Option<Ipv4Net>,
}

/// The note a list line gets from the other lists: a network whose route will be refused, an entry
/// more or less specific than another profile's, or an entry inside one of its own.
pub struct ListConflicts<'a> {
    set: &'a ListSet,
    footprints: &'a [ProfileFootprint],
}

impl<'a> ListConflicts<'a> {
    pub fn new(set: &'a ListSet, footprints: &'a [ProfileFootprint]) -> Self {
        Self { set, footprints }
    }

    /// Every note of `profile`'s lines, `(line, note)`.
    pub fn notes(&self, profile: &str) -> Vec<(usize, String)> {
        let Some(list) = self.set.get(profile) else {
            return Vec::new();
        };
        list.entries
            .iter()
            .filter_map(|entry| {
                self.note(profile, &entry.target, Some(entry.line))
                    .map(|note| (entry.line, note))
            })
            .collect()
    }

    /// The note of `target` in `profile`; `line` is its own line, `None` while it is being typed.
    pub fn note(&self, profile: &str, target: &ListTarget, line: Option<usize>) -> Option<String> {
        self.refusal(profile, target)
            .or_else(|| self.override_note(profile, target))
            .or_else(|| self.inside_own(profile, target, line))
    }

    fn refusal(&self, profile: &str, target: &ListTarget) -> Option<String> {
        let ListTarget::Network(net) = target else {
            return None;
        };
        self.footprints.iter().find_map(|footprint| {
            let other = !footprint.profile.eq_ignore_ascii_case(profile);
            if footprint.endpoints.iter().any(|ip| net.contains(*ip)) {
                Some(format!("covers {}'s endpoint", footprint.profile))
            } else if other && footprint.tunnel.is_some_and(|tunnel| tunnel.overlaps(net)) {
                Some(format!("covers {}'s tunnel", footprint.profile))
            } else {
                None
            }
        })
    }

    fn override_note(&self, profile: &str, target: &ListTarget) -> Option<String> {
        self.set
            .lists()
            .iter()
            .filter(|l| !l.profile.eq_ignore_ascii_case(profile))
            .find_map(|other| {
                other.list.entries.iter().find_map(|entry| {
                    if Self::strictly_inside(target, &entry.target) {
                        Some(format!(
                            "overrides {} line {} while both are connected",
                            other.profile, entry.line
                        ))
                    } else if Self::strictly_inside(&entry.target, target) {
                        Some(format!(
                            "{} line {} ({}) overrides it while both are connected",
                            other.profile, entry.line, entry.target
                        ))
                    } else {
                        None
                    }
                })
            })
    }

    fn inside_own(
        &self,
        profile: &str,
        target: &ListTarget,
        line: Option<usize>,
    ) -> Option<String> {
        let entry = self
            .set
            .get(profile)?
            .entries
            .iter()
            .filter(|e| Some(e.line) != line && Self::strictly_inside(target, &e.target))
            .max_by_key(|e| Self::specificity(&e.target))?;
        Some(format!("inside {} (line {})", entry.target, entry.line))
    }

    /// Longer names and longer prefixes are more specific.
    fn specificity(target: &ListTarget) -> usize {
        match target {
            ListTarget::Domain(domain) => domain.as_str().len(),
            ListTarget::Network(net) => usize::from(net.prefix()),
        }
    }

    /// `inner` is a subdomain of `outer`, or a smaller network inside it.
    fn strictly_inside(inner: &ListTarget, outer: &ListTarget) -> bool {
        match (inner, outer) {
            (ListTarget::Domain(inner), ListTarget::Domain(outer)) => inner
                .as_str()
                .strip_suffix(outer.as_str())
                .is_some_and(|head| head.ends_with('.')),
            (ListTarget::Network(inner), ListTarget::Network(outer)) => {
                inner != outer && outer.covers(inner)
            }
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::list::RoutesList;

    fn target(text: &str) -> ListTarget {
        RoutesList::parse_target(text).unwrap()
    }

    fn footprints() -> Vec<ProfileFootprint> {
        vec![
            ProfileFootprint {
                profile: "AcmeCorp".into(),
                endpoints: vec![Ipv4Addr::new(198, 51, 100, 7)],
                tunnel: Some("10.88.4.0/24".parse().unwrap()),
            },
            ProfileFootprint {
                profile: "Contoso".into(),
                endpoints: vec![Ipv4Addr::new(203, 0, 113, 62)],
                tunnel: Some("10.99.0.46/32".parse().unwrap()),
            },
        ]
    }

    #[test]
    fn notes_overrides_in_both_directions_and_own_nesting() {
        let set = ListSet::load([
            ("AcmeCorp", "corp.com\na.corp.com\n172.16.0.0/12\n"),
            ("Contoso", "x.corp.com\ncontoso.example\n172.20.0.0/16\n"),
        ]);
        let footprints = footprints();
        let conflicts = ListConflicts::new(&set, &footprints);
        assert_eq!(
            conflicts.notes("AcmeCorp"),
            [
                (
                    1,
                    "Contoso line 1 (x.corp.com) overrides it while both are connected".into()
                ),
                (2, "inside corp.com (line 1)".into()),
                (
                    3,
                    "Contoso line 3 (172.20.0.0/16) overrides it while both are connected".into()
                ),
            ]
        );
        assert_eq!(
            conflicts.notes("Contoso"),
            [
                (
                    1,
                    "overrides AcmeCorp line 1 while both are connected".into()
                ),
                (
                    3,
                    "overrides AcmeCorp line 3 while both are connected".into()
                ),
            ]
        );
        assert_eq!(
            conflicts.note("Contoso", &target("notcorp.com"), None),
            None
        );
        assert_eq!(
            conflicts
                .note("AcmeCorp", &target("b.a.corp.com"), None)
                .unwrap(),
            "inside a.corp.com (line 2)"
        );
    }

    #[test]
    fn a_network_over_an_endpoint_or_another_tunnel_is_refused() {
        let set = ListSet::default();
        let footprints = footprints();
        let conflicts = ListConflicts::new(&set, &footprints);
        assert_eq!(
            conflicts
                .note("Contoso", &target("198.51.100.0/24"), None)
                .unwrap(),
            "covers AcmeCorp's endpoint"
        );
        assert_eq!(
            conflicts
                .note("AcmeCorp", &target("198.51.100.7"), None)
                .unwrap(),
            "covers AcmeCorp's endpoint"
        );
        assert_eq!(
            conflicts
                .note("Contoso", &target("10.88.0.0/16"), None)
                .unwrap(),
            "covers AcmeCorp's tunnel"
        );
        assert_eq!(
            conflicts.note("AcmeCorp", &target("10.88.0.0/16"), None),
            None
        );
        assert_eq!(conflicts.note("AcmeCorp", &target("corp.com"), None), None);
    }
}
