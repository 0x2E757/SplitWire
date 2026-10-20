use std::collections::{BTreeMap, BTreeSet};
use std::net::Ipv4Addr;

/// Which profile holds each learned /32. The first profile to learn an address
/// holds it until its route expires, it disconnects or its list drops the name; another profile
/// answered the same address does not take it over (no preemption).
#[derive(Debug, Default)]
pub struct HostClaims {
    holders: BTreeMap<Ipv4Addr, String>,
}

/// The addresses of one answer, split by who holds them.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ClaimSplit {
    /// Free or already this profile's: claimed for it now.
    pub mine: Vec<Ipv4Addr>,
    /// Held by another profile, with that profile.
    pub held: Vec<(Ipv4Addr, String)>,
}

impl HostClaims {
    /// Claims every free address of `ips` for `profile`.
    pub fn claim(&mut self, profile: &str, ips: &[Ipv4Addr]) -> ClaimSplit {
        let mut split = ClaimSplit::default();
        for ip in ips {
            match self.holders.get(ip) {
                Some(holder) if holder != profile => split.held.push((*ip, holder.clone())),
                _ => {
                    self.holders.insert(*ip, profile.to_owned());
                    split.mine.push(*ip);
                }
            }
        }
        split
    }

    pub fn release(&mut self, profile: &str, ips: &[Ipv4Addr]) {
        for ip in ips {
            if self.holders.get(ip).is_some_and(|h| h == profile) {
                self.holders.remove(ip);
            }
        }
    }

    /// Releases every claim of `profile` (it disconnected); returns how many.
    pub fn release_profile(&mut self, profile: &str) -> usize {
        let before = self.holders.len();
        self.holders.retain(|_, holder| holder != profile);
        before - self.holders.len()
    }

    /// Keeps only the claims of `profile` it still has a learned route for.
    pub fn retain(&mut self, profile: &str, learned: &BTreeSet<Ipv4Addr>) {
        self.holders
            .retain(|ip, holder| holder != profile || learned.contains(ip));
    }

    pub fn holder(&self, ip: Ipv4Addr) -> Option<&str> {
        self.holders.get(&ip).map(String::as_str)
    }

    pub fn len(&self) -> usize {
        self.holders.len()
    }

    pub fn is_empty(&self) -> bool {
        self.holders.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ip(text: &str) -> Ipv4Addr {
        text.parse().unwrap()
    }

    #[test]
    fn the_first_learner_holds_and_another_is_refused() {
        let mut claims = HostClaims::default();
        let first = claims.claim("Contoso", &[ip("192.0.2.93")]);
        assert_eq!(first.mine, [ip("192.0.2.93")]);
        let second = claims.claim("AcmeCorp", &[ip("192.0.2.93"), ip("198.51.100.14")]);
        assert_eq!(second.mine, [ip("198.51.100.14")]);
        assert_eq!(second.held, [(ip("192.0.2.93"), "Contoso".to_owned())]);
        // The holder learning it again keeps it.
        assert!(claims.claim("Contoso", &[ip("192.0.2.93")]).held.is_empty());
        assert_eq!(claims.holder(ip("192.0.2.93")), Some("Contoso"));
    }

    #[test]
    fn claims_are_released_by_expiry_disconnect_or_hand() {
        let mut claims = HostClaims::default();
        claims.claim("A", &[ip("1.1.1.1"), ip("2.2.2.2"), ip("3.3.3.3")]);
        claims.claim("B", &[ip("4.4.4.4")]);
        claims.retain("A", &BTreeSet::from([ip("1.1.1.1"), ip("2.2.2.2")]));
        assert_eq!(claims.holder(ip("3.3.3.3")), None);
        claims.release("B", &[ip("1.1.1.1")]);
        assert_eq!(claims.holder(ip("1.1.1.1")), Some("A"));
        claims.release("A", &[ip("1.1.1.1")]);
        assert_eq!(claims.release_profile("A"), 1);
        assert_eq!(claims.len(), 1);
        // Released: the other profile may learn it now.
        assert_eq!(claims.claim("B", &[ip("2.2.2.2")]).mine.len(), 1);
    }
}
