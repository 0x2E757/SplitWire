use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use crate::net::Ipv4Net;

/// Why a route is wanted; the first origin wins when several want the same network.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RouteOrigin {
    Upstream,
    Static,
    Learned(String),
}

impl fmt::Display for RouteOrigin {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RouteOrigin::Upstream => f.write_str("upstream DNS"),
            RouteOrigin::Static => f.write_str("static"),
            RouteOrigin::Learned(name) => write!(f, "learned from {name}"),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RoutePlan {
    pub add: Vec<(Ipv4Net, RouteOrigin)>,
    pub remove: Vec<Ipv4Net>,
}

impl RoutePlan {
    pub fn is_empty(&self) -> bool {
        self.add.is_empty() && self.remove.is_empty()
    }
}

/// Turns the wanted routes and the installed ones into the changes that make them equal.
pub struct RoutePlanner;

impl RoutePlanner {
    pub fn plan(
        desired: &BTreeMap<Ipv4Net, RouteOrigin>,
        installed: &BTreeSet<Ipv4Net>,
    ) -> RoutePlan {
        RoutePlan {
            add: desired
                .iter()
                .filter(|(net, _)| !installed.contains(net))
                .map(|(net, origin)| (*net, origin.clone()))
                .collect(),
            remove: installed
                .iter()
                .filter(|net| !desired.contains_key(net))
                .copied()
                .collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn net(text: &str) -> Ipv4Net {
        text.parse().unwrap()
    }

    #[test]
    fn adds_missing_and_removes_unwanted() {
        let desired = BTreeMap::from([
            (net("10.66.0.53"), RouteOrigin::Upstream),
            (net("10.0.0.0/8"), RouteOrigin::Static),
        ]);
        let installed = BTreeSet::from([net("10.66.0.53"), net("1.2.3.4")]);
        let plan = RoutePlanner::plan(&desired, &installed);
        assert_eq!(plan.add, [(net("10.0.0.0/8"), RouteOrigin::Static)]);
        assert_eq!(plan.remove, [net("1.2.3.4")]);
    }

    #[test]
    fn nothing_to_do_when_in_sync() {
        let desired = BTreeMap::from([(net("1.2.3.4"), RouteOrigin::Learned("a.b.com".into()))]);
        let installed = BTreeSet::from([net("1.2.3.4")]);
        assert!(RoutePlanner::plan(&desired, &installed).is_empty());
    }
}
