use std::collections::{BTreeMap, BTreeSet};
use std::net::Ipv4Addr;
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use super::{
    LearnedRoute, LearnedRoutes, RefusedRoute, RefusedRoutes, RouteGuard, RouteOrigin,
    RoutePlanner, RouteTable,
};
use crate::list::DomainMatcher;
use crate::logs::Log;
use crate::net::Ipv4Net;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StaticRoute {
    pub net: Ipv4Net,
    pub installed: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RoutesSnapshot {
    pub learned: Vec<LearnedRoute>,
    pub statics: Vec<StaticRoute>,
    pub upstreams: Vec<StaticRoute>,
    pub refused: Vec<RefusedRoute>,
    pub installed: usize,
}

/// Keeps one profile's routes on its adapter (`luid`) equal to what is wanted: the upstream DNS
/// hosts, the static entries and the learned hosts, minus whatever the guard refuses.
pub struct RouteManager {
    table: Arc<dyn RouteTable>,
    luid: u64,
    guard: RouteGuard,
    upstreams: BTreeSet<Ipv4Net>,
    statics: BTreeSet<Ipv4Net>,
    learned: LearnedRoutes,
    installed: BTreeSet<Ipv4Net>,
    refused: RefusedRoutes,
    idle_expiry: Duration,
    log: Log,
}

impl RouteManager {
    pub fn new(
        table: Arc<dyn RouteTable>,
        luid: u64,
        upstreams: &[Ipv4Addr],
        idle_expiry: Duration,
        log: Log,
    ) -> Self {
        Self {
            table,
            luid,
            guard: RouteGuard::new(&[]),
            upstreams: upstreams.iter().map(|ip| Ipv4Net::host(*ip)).collect(),
            statics: BTreeSet::new(),
            learned: LearnedRoutes::default(),
            installed: BTreeSet::new(),
            refused: RefusedRoutes::new(256),
            idle_expiry,
            log,
        }
    }

    /// A new guard forgets the refusals of the old one; the same guard again keeps them.
    pub fn set_guard(&mut self, guard: RouteGuard) {
        if guard == self.guard {
            return;
        }
        self.guard = guard;
        self.refused.clear();
    }

    pub fn set_statics(&mut self, nets: &[Ipv4Net]) {
        self.statics = nets.iter().copied().collect();
        self.refused.clear();
    }

    /// Routes the IPv4 answers for `name` into the tunnel at once, before the answer is sent.
    /// Returns the addresses learned; the guard refused the others.
    pub fn learn(&mut self, name: &str, ips: &[Ipv4Addr], now: SystemTime) -> Vec<Ipv4Addr> {
        let origin = RouteOrigin::Learned(name.to_owned());
        let mut learned = Vec::new();
        for ip in ips {
            let net = Ipv4Net::host(*ip);
            if let Err(reason) = self.guard.check(&net) {
                self.note_refused(net, &origin, reason, now);
                continue;
            }
            self.learned.observe(*ip, name, now);
            learned.push(*ip);
            if !self.installed.contains(&net) {
                self.install(net, &origin);
            }
        }
        learned
    }

    /// The learned hosts.
    pub fn learned_ips(&self) -> BTreeSet<Ipv4Addr> {
        self.learned.iter().map(|route| route.ip).collect()
    }

    pub fn set_idle_expiry(&mut self, idle_expiry: Duration) {
        self.idle_expiry = idle_expiry;
    }

    pub fn luid(&self) -> u64 {
        self.luid
    }

    /// Moves to a recreated adapter: the old routes went with the old adapter, so the wanted
    /// ones are installed on the new one.
    pub fn rebind(&mut self, luid: u64, now: SystemTime) {
        self.luid = luid;
        self.installed.clear();
        self.reconcile(now);
    }

    /// Drops learned hosts idle for longer than the expiry; returns how many.
    pub fn expire(&mut self, now: SystemTime) -> usize {
        let expired = self.learned.expire(now, self.idle_expiry);
        for route in &expired {
            self.log.info(format!(
                "learned route {} ({}) expired: not seen for {}s",
                route.ip,
                route.name,
                self.idle_expiry.as_secs()
            ));
        }
        self.sync(now);
        expired.len()
    }

    /// Drops learned hosts whose names are no longer listed (after a reload).
    pub fn forget_unlisted(&mut self, matcher: &DomainMatcher, now: SystemTime) -> usize {
        self.retain_names(|name| matcher.matches(name), "is no longer listed", now)
    }

    /// Drops learned hosts whose name `keep` refuses (no longer listed, or now another
    /// profile's); `why` ends the log line. Returns how many.
    pub fn retain_names(
        &mut self,
        keep: impl Fn(&str) -> bool,
        why: &str,
        now: SystemTime,
    ) -> usize {
        let dropped = self.learned.take_where(|route| !keep(&route.name));
        for route in &dropped {
            self.log.info(format!(
                "learned route {} dropped: {} {why}",
                route.ip, route.name
            ));
        }
        self.sync(now);
        dropped.len()
    }

    pub fn clear_learned(&mut self, now: SystemTime) -> usize {
        let count = self.learned.clear();
        self.sync(now);
        count
    }

    /// Re-reads the owned routes from the OS (they vanish with the adapter), then makes them
    /// equal to the wanted ones.
    pub fn reconcile(&mut self, now: SystemTime) {
        match self.table.list_owned(self.luid) {
            Ok(owned) => self.installed = owned.into_iter().collect(),
            Err(e) => self
                .log
                .error(format!("reading the route table failed: {e}")),
        }
        self.sync(now);
    }

    /// Removes every route SplitWire owns on this adapter (on disconnect).
    pub fn remove_all(&mut self) {
        let mut owned = self.installed.clone();
        match self.table.list_owned(self.luid) {
            Ok(listed) => owned.extend(listed),
            Err(e) => self
                .log
                .error(format!("reading the route table failed: {e}")),
        }
        for net in owned {
            self.uninstall(net);
        }
    }

    pub fn snapshot(&self) -> RoutesSnapshot {
        let rows = |nets: &BTreeSet<Ipv4Net>| {
            nets.iter()
                .map(|net| StaticRoute {
                    net: *net,
                    installed: self.installed.contains(net),
                })
                .collect()
        };
        RoutesSnapshot {
            learned: self.learned.iter().cloned().collect(),
            statics: rows(&self.statics),
            upstreams: rows(&self.upstreams),
            refused: self.refused.iter().cloned().collect(),
            installed: self.installed.len(),
        }
    }

    pub fn idle_expiry(&self) -> Duration {
        self.idle_expiry
    }

    fn sync(&mut self, now: SystemTime) {
        let desired = self.desired(now);
        let plan = RoutePlanner::plan(&desired, &self.installed);
        for net in plan.remove {
            self.uninstall(net);
        }
        for (net, origin) in plan.add {
            self.install(net, &origin);
        }
    }

    fn desired(&mut self, now: SystemTime) -> BTreeMap<Ipv4Net, RouteOrigin> {
        let wanted: Vec<(Ipv4Net, RouteOrigin)> = self
            .upstreams
            .iter()
            .map(|net| (*net, RouteOrigin::Upstream))
            .chain(self.statics.iter().map(|net| (*net, RouteOrigin::Static)))
            .chain(self.learned.iter().map(|route| {
                (
                    Ipv4Net::host(route.ip),
                    RouteOrigin::Learned(route.name.clone()),
                )
            }))
            .collect();
        let mut desired = BTreeMap::new();
        for (net, origin) in wanted {
            match self.guard.check(&net) {
                Ok(()) => {
                    desired.entry(net).or_insert(origin);
                }
                Err(reason) => self.note_refused(net, &origin, reason, now),
            }
        }
        desired
    }

    fn install(&mut self, net: Ipv4Net, origin: &RouteOrigin) {
        match self.table.add(self.luid, net) {
            Ok(()) => {
                self.installed.insert(net);
                self.log.info(format!("route added {net} ({origin})"));
            }
            Err(e) => self
                .log
                .error(format!("route add {net} ({origin}) failed: {e}")),
        }
    }

    fn uninstall(&mut self, net: Ipv4Net) {
        match self.table.remove(self.luid, net) {
            Ok(()) => self.log.info(format!("route removed {net}")),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => {
                self.log.error(format!("route remove {net} failed: {e}"));
                return;
            }
        }
        self.installed.remove(&net);
    }

    fn note_refused(&mut self, net: Ipv4Net, origin: &RouteOrigin, reason: String, at: SystemTime) {
        let refused = RefusedRoute {
            net,
            origin: origin.to_string(),
            reason: reason.clone(),
            at,
        };
        if self.refused.record(refused) {
            self.log
                .warn(format!("route refused {net} ({origin}): {reason}"));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::clock::ManualClock;
    use crate::list::DomainName;
    use crate::net::OnLinkNetwork;
    use std::io;
    use std::sync::Mutex;
    use std::time::UNIX_EPOCH;

    /// Routes per adapter, as the OS keeps them.
    #[derive(Default)]
    struct FakeTable {
        routes: Mutex<BTreeSet<(u64, Ipv4Net)>>,
    }

    impl FakeTable {
        fn routes(&self, luid: u64) -> Vec<String> {
            self.routes
                .lock()
                .unwrap()
                .iter()
                .filter(|(on, _)| *on == luid)
                .map(|(_, net)| net.to_string())
                .collect()
        }
    }

    impl RouteTable for FakeTable {
        fn list_owned(&self, luid: u64) -> io::Result<Vec<Ipv4Net>> {
            let routes = self.routes.lock().unwrap();
            Ok(routes
                .iter()
                .filter(|(on, _)| *on == luid)
                .map(|(_, net)| *net)
                .collect())
        }
        fn add(&self, luid: u64, net: Ipv4Net) -> io::Result<()> {
            self.routes.lock().unwrap().insert((luid, net));
            Ok(())
        }
        fn remove(&self, luid: u64, net: Ipv4Net) -> io::Result<()> {
            if self.routes.lock().unwrap().remove(&(luid, net)) {
                Ok(())
            } else {
                Err(io::ErrorKind::NotFound.into())
            }
        }
    }

    const HOUR: Duration = Duration::from_secs(3600);
    const LUID: u64 = 7;

    fn log() -> Log {
        Log::new(1 << 20, Arc::new(ManualClock::new(UNIX_EPOCH)), false)
    }

    fn manager_on(table: &Arc<FakeTable>, luid: u64, log: &Log) -> RouteManager {
        RouteManager::new(
            table.clone(),
            luid,
            &[Ipv4Addr::new(10, 66, 0, 53)],
            24 * HOUR,
            log.clone(),
        )
    }

    fn setup() -> (Arc<FakeTable>, RouteManager, Log) {
        let table = Arc::new(FakeTable::default());
        let log = log();
        let manager = manager_on(&table, LUID, &log);
        (table, manager, log)
    }

    fn net(text: &str) -> Ipv4Net {
        text.parse().unwrap()
    }

    fn ip(text: &str) -> Ipv4Addr {
        text.parse().unwrap()
    }

    #[test]
    fn reconcile_installs_upstream_and_statics_and_removes_stale_routes() {
        let (table, mut manager, _) = setup();
        table.add(LUID, net("9.9.9.9")).unwrap(); // left over on this adapter
        manager.set_statics(&[net("10.0.0.0/8"), net("192.0.2.93")]);
        manager.reconcile(UNIX_EPOCH);
        assert_eq!(
            table.routes(LUID),
            ["10.0.0.0/8", "10.66.0.53/32", "192.0.2.93/32"]
        );
    }

    #[test]
    fn learning_adds_host_routes_at_once() {
        let (table, mut manager, log) = setup();
        manager.learn("www.contoso.example", &[ip("34.1.2.3")], UNIX_EPOCH);
        assert_eq!(table.routes(LUID), ["34.1.2.3/32"]);
        assert!(log.contains("route added 34.1.2.3/32 (learned from www.contoso.example)"));
        assert_eq!(manager.snapshot().learned.len(), 1);
    }

    #[test]
    fn learned_routes_expire_after_idle_but_not_while_seen() {
        let (table, mut manager, _) = setup();
        let start = UNIX_EPOCH;
        manager.learn("a.example.com", &[ip("34.1.2.3")], start);
        manager.learn("a.example.com", &[ip("34.1.2.3")], start + 20 * HOUR);
        assert_eq!(manager.expire(start + 30 * HOUR), 0);
        assert_eq!(table.routes(LUID), ["10.66.0.53/32", "34.1.2.3/32"]);
        assert_eq!(manager.expire(start + 44 * HOUR), 1);
        assert_eq!(table.routes(LUID), ["10.66.0.53/32"]);
    }

    #[test]
    fn rebinding_installs_the_routes_on_the_new_adapter() {
        let (table, mut manager, log) = setup();
        manager.learn("a.example.com", &[ip("34.1.2.3")], UNIX_EPOCH);
        table.routes.lock().unwrap().clear();
        manager.rebind(LUID + 1, UNIX_EPOCH);
        assert_eq!(manager.luid(), LUID + 1);
        assert_eq!(table.routes(LUID + 1), ["10.66.0.53/32", "34.1.2.3/32"]);
        let moved = manager.retain_names(|name| name != "a.example.com", "moved", UNIX_EPOCH);
        assert_eq!(moved, 1);
        assert_eq!(table.routes(LUID + 1), ["10.66.0.53/32"]);
        assert!(log.contains("learned route 34.1.2.3 dropped: a.example.com moved"));
    }

    #[test]
    fn reload_syncs_statics_and_forgets_unlisted_learned_routes() {
        let (table, mut manager, _) = setup();
        manager.set_statics(&[net("10.0.0.0/8")]);
        manager.reconcile(UNIX_EPOCH);
        manager.learn("x.old.com", &[ip("34.1.1.1")], UNIX_EPOCH);
        manager.learn("x.kept.com", &[ip("34.2.2.2")], UNIX_EPOCH);

        manager.set_statics(&[net("172.16.0.0/12")]);
        let kept = DomainMatcher::new(&[DomainName::parse("kept.com").unwrap()]);
        assert_eq!(manager.forget_unlisted(&kept, UNIX_EPOCH), 1);
        assert_eq!(
            table.routes(LUID),
            ["10.66.0.53/32", "34.2.2.2/32", "172.16.0.0/12"]
        );
    }

    #[test]
    fn refused_routes_are_never_installed_and_are_reported() {
        let (table, mut manager, log) = setup();
        manager.set_guard(
            RouteGuard::new(&[OnLinkNetwork {
                net: net("192.168.1.0/24"),
                interface: "Ethernet".into(),
            }])
            .forbid_hosts(&[ip("192.0.2.93")], "the endpoint of Contoso"),
        );
        manager.set_statics(&[net("0.0.0.0/0"), net("192.168.0.0/16")]);
        manager.reconcile(UNIX_EPOCH);
        let learned = manager.learn(
            "vpn.example.com",
            &[ip("192.0.2.93"), ip("127.0.0.1"), ip("34.5.6.7")],
            UNIX_EPOCH,
        );
        assert_eq!(learned, [ip("34.5.6.7")]);
        assert_eq!(manager.learned_ips(), BTreeSet::from([ip("34.5.6.7")]));
        assert_eq!(table.routes(LUID), ["10.66.0.53/32", "34.5.6.7/32"]);
        let refused: Vec<String> = manager
            .snapshot()
            .refused
            .iter()
            .map(|r| r.net.to_string())
            .collect();
        assert_eq!(
            refused,
            [
                "0.0.0.0/0",
                "127.0.0.1/32",
                "192.0.2.93/32",
                "192.168.0.0/16"
            ]
        );
        assert!(log.contains("route refused 192.0.2.93/32"));
    }

    #[test]
    fn clearing_learned_keeps_statics_and_remove_all_empties_the_adapter() {
        let (table, mut manager, _) = setup();
        manager.set_statics(&[net("10.0.0.0/8")]);
        manager.reconcile(UNIX_EPOCH);
        manager.learn("a.example.com", &[ip("34.1.2.3")], UNIX_EPOCH);
        assert_eq!(manager.clear_learned(UNIX_EPOCH), 1);
        assert_eq!(table.routes(LUID), ["10.0.0.0/8", "10.66.0.53/32"]);
        manager.remove_all();
        assert!(table.routes(LUID).is_empty());
        assert_eq!(manager.snapshot().installed, 0);
    }

    #[test]
    fn reconcile_restores_routes_lost_with_the_adapter() {
        let (table, mut manager, _) = setup();
        manager.learn("a.example.com", &[ip("34.1.2.3")], UNIX_EPOCH);
        table.routes.lock().unwrap().clear();
        manager.reconcile(UNIX_EPOCH);
        assert_eq!(table.routes(LUID), ["10.66.0.53/32", "34.1.2.3/32"]);
    }

    #[test]
    fn two_managers_keep_the_same_upstream_on_their_own_adapters() {
        let table = Arc::new(FakeTable::default());
        let log = log();
        let mut first = manager_on(&table, 1, &log);
        let mut second = manager_on(&table, 2, &log);
        first.reconcile(UNIX_EPOCH);
        second.reconcile(UNIX_EPOCH);
        second.learn("b.example.com", &[ip("34.9.9.9")], UNIX_EPOCH);
        assert_eq!(table.routes(1), ["10.66.0.53/32"]);
        assert_eq!(table.routes(2), ["10.66.0.53/32", "34.9.9.9/32"]);
        first.remove_all();
        assert!(table.routes(1).is_empty());
        assert_eq!(table.routes(2), ["10.66.0.53/32", "34.9.9.9/32"]);
    }
}
