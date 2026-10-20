use std::collections::BTreeMap;
use std::time::SystemTime;

use crate::net::Ipv4Net;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RefusedRoute {
    pub net: Ipv4Net,
    pub origin: String,
    pub reason: String,
    pub at: SystemTime,
}

/// The routes the guard refused, latest per network, bounded so a flood cannot grow it.
pub struct RefusedRoutes {
    routes: BTreeMap<Ipv4Net, RefusedRoute>,
    capacity: usize,
}

impl RefusedRoutes {
    pub fn new(capacity: usize) -> Self {
        Self {
            routes: BTreeMap::new(),
            capacity,
        }
    }

    /// Records a refusal; true when it is news (a new network or a different reason).
    pub fn record(&mut self, refused: RefusedRoute) -> bool {
        let news = self
            .routes
            .get(&refused.net)
            .is_none_or(|old| old.reason != refused.reason);
        if !self.routes.contains_key(&refused.net) && self.routes.len() >= self.capacity {
            self.drop_oldest();
        }
        self.routes.insert(refused.net, refused);
        news
    }

    pub fn clear(&mut self) {
        self.routes.clear();
    }

    pub fn iter(&self) -> impl Iterator<Item = &RefusedRoute> {
        self.routes.values()
    }

    pub fn len(&self) -> usize {
        self.routes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.routes.is_empty()
    }

    fn drop_oldest(&mut self) {
        if let Some(oldest) = self.routes.values().min_by_key(|r| r.at).map(|r| r.net) {
            self.routes.remove(&oldest);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, UNIX_EPOCH};

    fn refused(net: &str, reason: &str, secs: u64) -> RefusedRoute {
        RefusedRoute {
            net: net.parse().unwrap(),
            origin: "static".into(),
            reason: reason.into(),
            at: UNIX_EPOCH + Duration::from_secs(secs),
        }
    }

    #[test]
    fn reports_news_only_once_per_reason() {
        let mut routes = RefusedRoutes::new(10);
        assert!(routes.record(refused("1.1.1.1", "a", 1)));
        assert!(!routes.record(refused("1.1.1.1", "a", 2)));
        assert!(routes.record(refused("1.1.1.1", "b", 3)));
        assert_eq!(routes.len(), 1);
    }

    #[test]
    fn drops_the_oldest_when_full() {
        let mut routes = RefusedRoutes::new(2);
        routes.record(refused("1.1.1.1", "a", 5));
        routes.record(refused("2.2.2.2", "a", 1));
        routes.record(refused("3.3.3.3", "a", 9));
        let nets: Vec<String> = routes.iter().map(|r| r.net.to_string()).collect();
        assert_eq!(nets, ["1.1.1.1/32", "3.3.3.3/32"]);
    }
}
