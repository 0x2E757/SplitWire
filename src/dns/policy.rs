use std::sync::{Arc, RwLock};

use crate::list::{DomainMatcher, DomainName};

/// The live DNS settings the proxy applies to every query: the names always sent to the
/// fallback (the `bypass` setting plus every profile's endpoint host) and the TTL cap.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DnsPolicy {
    pub bypass: DomainMatcher,
    pub max_ttl: u32,
}

impl DnsPolicy {
    pub fn new(bypass: &[DomainName], endpoint_hosts: &[DomainName], max_ttl: u32) -> Self {
        Self {
            bypass: DomainMatcher::new(bypass.iter().chain(endpoint_hosts)),
            max_ttl,
        }
    }
}

impl Default for DnsPolicy {
    fn default() -> Self {
        Self::new(&[], &[], 300)
    }
}

/// The current policy, swapped whole when a live setting or the profiles change.
#[derive(Default)]
pub struct SharedPolicy {
    current: RwLock<Arc<DnsPolicy>>,
}

impl SharedPolicy {
    pub fn current(&self) -> Arc<DnsPolicy> {
        self.current.read().unwrap().clone()
    }

    pub fn replace(&self, policy: DnsPolicy) {
        *self.current.write().unwrap() = Arc::new(policy);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bypasses_the_setting_and_every_endpoint_host() {
        let name = |n: &str| DomainName::parse(n).unwrap();
        let shared = SharedPolicy::default();
        assert!(!shared.current().bypass.matches("vpn.acme.example"));
        shared.replace(DnsPolicy::new(
            &[name("login.example.com")],
            &[name("vpn.acme.example")],
            60,
        ));
        let policy = shared.current();
        assert!(policy.bypass.matches("vpn.acme.example"));
        assert!(policy.bypass.matches("a.login.example.com"));
        assert!(!policy.bypass.matches("acme.example"));
        assert_eq!(policy.max_ttl, 60);
    }
}
