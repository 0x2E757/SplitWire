use std::collections::BTreeSet;
use std::sync::{Arc, RwLock};

use super::DomainName;

/// Matches a queried name against a set of domains: the domain itself and all its subdomains.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DomainMatcher {
    domains: BTreeSet<String>,
}

impl DomainMatcher {
    pub fn new<'a>(domains: impl IntoIterator<Item = &'a DomainName>) -> Self {
        Self {
            domains: domains.into_iter().map(|d| d.as_str().to_owned()).collect(),
        }
    }

    /// The listed domain that covers `name`, if any (the most specific one).
    pub fn matching_domain(&self, name: &str) -> Option<&str> {
        let name = name.strip_suffix('.').unwrap_or(name).to_ascii_lowercase();
        let mut candidate = name.as_str();
        loop {
            if let Some(found) = self.domains.get(candidate) {
                return Some(found);
            }
            candidate = candidate.split_once('.')?.1;
        }
    }

    pub fn matches(&self, name: &str) -> bool {
        self.matching_domain(name).is_some()
    }

    pub fn domains(&self) -> impl Iterator<Item = &str> {
        self.domains.iter().map(String::as_str)
    }

    pub fn len(&self) -> usize {
        self.domains.len()
    }

    pub fn is_empty(&self) -> bool {
        self.domains.is_empty()
    }
}

/// The current listed-domain matcher, swapped as a whole on reload.
#[derive(Default)]
pub struct SharedMatcher {
    current: RwLock<Arc<DomainMatcher>>,
}

impl SharedMatcher {
    pub fn current(&self) -> Arc<DomainMatcher> {
        self.current.read().unwrap().clone()
    }

    pub fn replace(&self, matcher: DomainMatcher) {
        *self.current.write().unwrap() = Arc::new(matcher);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn matcher(names: &[&str]) -> DomainMatcher {
        let domains: Vec<DomainName> = names
            .iter()
            .map(|n| DomainName::parse(n).unwrap())
            .collect();
        DomainMatcher::new(&domains)
    }

    #[test]
    fn matches_apex_and_subdomains() {
        let m = matcher(&["contoso.example"]);
        assert_eq!(
            m.matching_domain("contoso.example"),
            Some("contoso.example")
        );
        assert_eq!(
            m.matching_domain("a.b.contoso.example"),
            Some("contoso.example")
        );
    }

    #[test]
    fn ignores_case_and_trailing_dot() {
        let m = matcher(&["contoso.example"]);
        assert!(m.matches("WWW.Contoso.Example."));
    }

    #[test]
    fn does_not_match_lookalikes_or_parents() {
        let m = matcher(&["contoso.example"]);
        assert!(!m.matches("notcontoso.example"));
        assert!(!m.matches("contoso.example.evil.net"));
        assert!(!m.matches("com"));
        assert!(!m.matches(""));
    }

    #[test]
    fn picks_the_most_specific_listed_domain() {
        let m = matcher(&["example.com", "eu.example.com"]);
        assert_eq!(
            m.matching_domain("x.eu.example.com"),
            Some("eu.example.com")
        );
        assert_eq!(m.matching_domain("us.example.com"), Some("example.com"));
    }

    #[test]
    fn shared_matcher_swaps_atomically() {
        let shared = SharedMatcher::default();
        assert!(!shared.current().matches("a.example.com"));
        shared.replace(matcher(&["example.com"]));
        assert!(shared.current().matches("a.example.com"));
    }
}
