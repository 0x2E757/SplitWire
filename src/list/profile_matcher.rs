use std::collections::BTreeMap;
use std::sync::{Arc, RwLock};

use super::{DomainName, ListSet};

/// Which connected profile a name goes to: the one listing the most specific domain covering it
/// (`a.corp.com` in A beats `corp.com` in B). Only connected profiles are in it, so a name of a
/// disconnected profile falls through to the next connected one.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ProfileMatcher {
    owners: BTreeMap<DomainName, String>,
}

impl ProfileMatcher {
    /// The domains of the `connected` profiles' lists. `ListSet` has already dropped exact
    /// duplicates across profiles, so every domain has one owner.
    pub fn build(lists: &ListSet, connected: &[String]) -> Self {
        let mut owners = BTreeMap::new();
        for list in lists.lists() {
            if !connected
                .iter()
                .any(|name| name.eq_ignore_ascii_case(&list.profile))
            {
                continue;
            }
            for domain in list.list.domains() {
                owners
                    .entry(domain.clone())
                    .or_insert_with(|| list.profile.clone());
            }
        }
        Self { owners }
    }

    /// `(profile, listed domain)` for `name`, the longest matching suffix first.
    pub fn owner(&self, name: &str) -> Option<(&str, &str)> {
        let name = name.strip_suffix('.').unwrap_or(name).to_ascii_lowercase();
        let mut candidate = name.as_str();
        loop {
            if let Ok(domain) = DomainName::parse(candidate)
                && let Some((domain, profile)) = self.owners.get_key_value(&domain)
            {
                return Some((profile, domain.as_str()));
            }
            candidate = candidate.split_once('.')?.1;
        }
    }

    /// True when `name` goes to `profile`.
    pub fn routes_to(&self, name: &str, profile: &str) -> bool {
        self.owner(name)
            .is_some_and(|(owner, _)| owner.eq_ignore_ascii_case(profile))
    }

    /// The union of the connected profiles' domains: exactly the NRPT rules.
    pub fn domains(&self) -> Vec<DomainName> {
        self.owners.keys().cloned().collect()
    }

    pub fn is_empty(&self) -> bool {
        self.owners.is_empty()
    }
}

/// The current matcher, swapped as a whole when the connected profiles or the lists change.
#[derive(Default)]
pub struct SharedProfileMatcher {
    current: RwLock<Arc<ProfileMatcher>>,
}

impl SharedProfileMatcher {
    pub fn current(&self) -> Arc<ProfileMatcher> {
        self.current.read().unwrap().clone()
    }

    pub fn replace(&self, matcher: ProfileMatcher) {
        *self.current.write().unwrap() = Arc::new(matcher);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lists() -> ListSet {
        ListSet::load([
            (
                "AcmeCorp",
                "acme.example\npartners.contoso.example\n10.0.0.0/8\n",
            ),
            ("Contoso", "contoso.example\n"),
            ("HomeLab", "lab.home.example\n"),
        ])
    }

    fn names(connected: &[&str]) -> Vec<String> {
        connected.iter().map(|n| (*n).to_owned()).collect()
    }

    #[test]
    fn the_most_specific_connected_profile_wins() {
        let matcher = ProfileMatcher::build(&lists(), &names(&["AcmeCorp", "Contoso"]));
        assert_eq!(
            matcher.owner("x.Partners.Contoso.example."),
            Some(("AcmeCorp", "partners.contoso.example"))
        );
        assert_eq!(
            matcher.owner("www.contoso.example"),
            Some(("Contoso", "contoso.example"))
        );
        assert_eq!(matcher.owner("contoso.example.evil.net"), None);
        assert_eq!(matcher.owner("lab.home.example"), None);
        assert!(matcher.routes_to("acme.example", "acmecorp"));
    }

    #[test]
    fn a_disconnected_profiles_names_fall_through() {
        let matcher = ProfileMatcher::build(&lists(), &names(&["Contoso"]));
        assert_eq!(
            matcher.owner("x.partners.contoso.example"),
            Some(("Contoso", "contoso.example"))
        );
        assert_eq!(matcher.owner("acme.example"), None);
        let none = ProfileMatcher::build(&lists(), &[]);
        assert!(none.is_empty());
    }

    #[test]
    fn the_union_of_connected_domains_is_the_nrpt_set() {
        let matcher = ProfileMatcher::build(&lists(), &names(&["AcmeCorp", "HomeLab"]));
        let domains: Vec<String> = matcher.domains().iter().map(ToString::to_string).collect();
        assert_eq!(
            domains,
            [
                "acme.example",
                "lab.home.example",
                "partners.contoso.example"
            ]
        );
    }

    #[test]
    fn the_shared_matcher_swaps_whole() {
        let shared = SharedProfileMatcher::default();
        assert!(shared.current().owner("acme.example").is_none());
        shared.replace(ProfileMatcher::build(&lists(), &names(&["AcmeCorp"])));
        assert!(shared.current().owner("acme.example").is_some());
    }
}
