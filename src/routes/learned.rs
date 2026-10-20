use std::collections::BTreeMap;
use std::net::Ipv4Addr;
use std::time::{Duration, SystemTime};

/// A /32 learned from a DNS answer for a listed name.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LearnedRoute {
    pub ip: Ipv4Addr,
    pub name: String,
    pub last_seen: SystemTime,
}

/// Learned hosts and when an answer last mentioned them; they expire after an idle period so
/// long-lived connections are not cut while the name keeps being resolved.
#[derive(Default)]
pub struct LearnedRoutes {
    routes: BTreeMap<Ipv4Addr, LearnedRoute>,
}

impl LearnedRoutes {
    /// Records that `ip` was seen in an answer for `name`; true when it is new.
    pub fn observe(&mut self, ip: Ipv4Addr, name: &str, now: SystemTime) -> bool {
        let route = LearnedRoute {
            ip,
            name: name.to_owned(),
            last_seen: now,
        };
        self.routes.insert(ip, route).is_none()
    }

    /// Removes and returns the routes not seen for at least `idle`.
    pub fn expire(&mut self, now: SystemTime, idle: Duration) -> Vec<LearnedRoute> {
        self.take_where(|route| {
            now.duration_since(route.last_seen)
                .is_ok_and(|age| age >= idle)
        })
    }

    /// Removes and returns the routes `drop` selects.
    pub fn take_where(&mut self, drop: impl Fn(&LearnedRoute) -> bool) -> Vec<LearnedRoute> {
        let ips: Vec<Ipv4Addr> = self
            .routes
            .values()
            .filter(|route| drop(route))
            .map(|route| route.ip)
            .collect();
        ips.iter().filter_map(|ip| self.routes.remove(ip)).collect()
    }

    pub fn clear(&mut self) -> usize {
        let count = self.routes.len();
        self.routes.clear();
        count
    }

    pub fn iter(&self) -> impl Iterator<Item = &LearnedRoute> {
        self.routes.values()
    }

    pub fn len(&self) -> usize {
        self.routes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.routes.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::UNIX_EPOCH;

    const HOUR: Duration = Duration::from_secs(3600);

    #[test]
    fn expires_only_after_the_idle_period_since_last_seen() {
        let start = UNIX_EPOCH + HOUR;
        let ip = Ipv4Addr::new(1, 2, 3, 4);
        let mut learned = LearnedRoutes::default();
        assert!(learned.observe(ip, "a.example.com", start));
        assert!(!learned.observe(ip, "a.example.com", start + 20 * HOUR));
        assert!(learned.expire(start + 24 * HOUR, 24 * HOUR).is_empty());
        let expired = learned.expire(start + 44 * HOUR, 24 * HOUR);
        assert_eq!(expired.len(), 1);
        assert!(learned.is_empty());
    }

    #[test]
    fn a_clock_going_backwards_expires_nothing() {
        let mut learned = LearnedRoutes::default();
        learned.observe(
            Ipv4Addr::new(1, 2, 3, 4),
            "a.example.com",
            UNIX_EPOCH + HOUR,
        );
        assert!(learned.expire(UNIX_EPOCH, Duration::ZERO).is_empty());
    }

    #[test]
    fn takes_selected_routes_and_clears() {
        let mut learned = LearnedRoutes::default();
        learned.observe(Ipv4Addr::new(1, 1, 1, 1), "a.one.com", UNIX_EPOCH);
        learned.observe(Ipv4Addr::new(2, 2, 2, 2), "b.two.com", UNIX_EPOCH);
        let taken = learned.take_where(|r| r.name.ends_with("one.com"));
        assert_eq!(taken[0].ip, Ipv4Addr::new(1, 1, 1, 1));
        assert_eq!(learned.len(), 1);
        assert_eq!(learned.clear(), 1);
    }
}
