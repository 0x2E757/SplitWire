use std::collections::HashMap;
use std::io;
use std::net::SocketAddrV4;
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};

use crate::clock::Clock;
use crate::logs::Log;
use crate::tunnel::{
    AdapterAddressing, AdapterSettings, AdapterState, DriverConfig, DriverVersion,
    EndpointResolver, HandshakeHealth, HandshakePolicy, Tunnel, TunnelHealth, TunnelLink,
    TunnelSpec, UpStage, WireGuardAdapter, WireGuardDriver,
};

/// `impl Tunnel` over wireguard-nt: one adapter per profile, created, addressed, configured and
/// brought up in that order; recreated when it vanished; its endpoint followed when the
/// handshake stalls. It knows only the boundaries, so it is tested with a fake driver.
pub struct WireGuardTunnel {
    driver: Arc<dyn WireGuardDriver>,
    addressing: Arc<dyn AdapterAddressing>,
    resolver: EndpointResolver,
    clock: Arc<dyn Clock>,
    log: Log,
    sessions: Mutex<HashMap<String, Arc<Mutex<Session>>>>,
}

/// One profile's adapter while it is connected. `adapter` is `None` after a failed recreate,
/// until a later refresh succeeds.
struct Session {
    adapter: Option<Box<dyn WireGuardAdapter>>,
    link: TunnelLink,
    endpoint: SocketAddrV4,
    handshakes: HandshakePolicy,
    last_resolve: Option<SystemTime>,
}

impl WireGuardTunnel {
    /// An endpoint is re-resolved at most this often.
    pub const RESOLVE_EVERY: Duration = Duration::from_secs(60);

    pub fn new(
        driver: Arc<dyn WireGuardDriver>,
        addressing: Arc<dyn AdapterAddressing>,
        resolver: EndpointResolver,
        clock: Arc<dyn Clock>,
        log: Log,
    ) -> Self {
        Self {
            driver,
            addressing,
            resolver,
            clock,
            log: log.with_source("tunnel"),
            sessions: Mutex::new(HashMap::new()),
        }
    }

    fn settings(spec: &TunnelSpec) -> AdapterSettings {
        AdapterSettings {
            address: spec.address,
            mtu: spec.mtu,
        }
    }

    /// Creates and starts the adapter. A failure drops the new adapter, which removes it; its
    /// error is marked with the stage that failed.
    fn open(&self, spec: &TunnelSpec) -> io::Result<Session> {
        let name = spec.adapter_name();
        self.resolver.addresses().record(&[*spec.endpoint.ip()]);
        let adapter = self
            .create(&name, spec)
            .map_err(|e| UpStage::Adapter.wrap(e))?;
        let luid = adapter.luid();
        let if_index = Self::start(adapter.as_ref(), spec)
            .and_then(|()| self.addressing.index(luid))
            .map_err(|e| UpStage::Peer.wrap(e))?;
        Ok(Session {
            adapter: Some(adapter),
            link: TunnelLink { luid, if_index },
            endpoint: spec.endpoint,
            handshakes: HandshakePolicy::new(self.clock.now()),
            last_resolve: None,
        })
    }

    /// The adapter, created with its address and MTU; another process's adapter is never taken.
    fn create(&self, name: &str, spec: &TunnelSpec) -> io::Result<Box<dyn WireGuardAdapter>> {
        if self.driver.open(name)?.is_some() {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                format!("{name} exists: another process holds it"),
            ));
        }
        let adapter = self
            .driver
            .create(name, TunnelSpec::TUNNEL_TYPE, spec.adapter_guid)?;
        adapter.set_logging(true)?;
        self.addressing
            .apply(adapter.luid(), &Self::settings(spec))?;
        Ok(adapter)
    }

    /// The peer's configuration, then the state up.
    fn start(adapter: &dyn WireGuardAdapter, spec: &TunnelSpec) -> io::Result<()> {
        adapter.set_config(&DriverConfig::Full(spec))?;
        adapter.set_state(AdapterState::Up)
    }

    fn session(&self, name: &str) -> io::Result<Arc<Mutex<Session>>> {
        self.sessions
            .lock()
            .unwrap()
            .get(name)
            .cloned()
            .ok_or_else(|| {
                io::Error::new(io::ErrorKind::NotFound, format!("{name} is not connected"))
            })
    }

    /// The adapter's index while it is the one this session created; `None` once it is gone or
    /// its LUID changed.
    fn current_index(&self, session: &Session) -> Option<u32> {
        let adapter = session.adapter.as_ref()?;
        if adapter.state().is_err() || adapter.luid() != session.link.luid {
            return None;
        }
        self.addressing.index(session.link.luid).ok()
    }

    /// Sets the state, address and MTU again when the adapter lost any of them.
    fn restore(
        &self,
        adapter: &dyn WireGuardAdapter,
        luid: u64,
        spec: &TunnelSpec,
    ) -> io::Result<()> {
        let settings = Self::settings(spec);
        let up = adapter.state()? == AdapterState::Up;
        if up && self.addressing.applied(luid, &settings)? {
            return Ok(());
        }
        self.log
            .with_profile(&spec.name)
            .warn("adapter was down or lost its address or MTU; setting them again");
        self.addressing.apply(luid, &settings)?;
        adapter.set_state(AdapterState::Up)
    }

    /// While the peer is handshaking or stale, re-resolves the endpoint (at most once a minute);
    /// when it moved, points the peer at the new address with `UPDATE_ONLY`. The resolver has
    /// already made the new address a never-route by then.
    fn follow_endpoint(&self, session: &mut Session, spec: &TunnelSpec) -> io::Result<()> {
        let now = self.clock.now();
        let adapter = session.adapter.as_ref().expect("checked by the caller");
        let health = adapter.config()?;
        if session.handshakes.judge(now, &health) == HandshakeHealth::Ok {
            return Ok(());
        }
        let recent = session
            .last_resolve
            .is_some_and(|last| now.duration_since(last).unwrap_or_default() < Self::RESOLVE_EVERY);
        if recent {
            return Ok(());
        }
        session.last_resolve = Some(now);
        let log = self.log.with_profile(&spec.name);
        let addresses = match self.resolver.resolve(&spec.endpoint_host) {
            Ok(addresses) => addresses,
            Err(e) => {
                log.warn(format!(
                    "endpoint {} did not resolve: {e}",
                    spec.endpoint_host
                ));
                return Ok(());
            }
        };
        if addresses.contains(session.endpoint.ip()) {
            return Ok(());
        }
        let endpoint = SocketAddrV4::new(addresses[0], session.endpoint.port());
        adapter.set_config(&DriverConfig::Endpoint {
            peer_public_key: spec.peer_public_key,
            endpoint,
        })?;
        log.info(format!(
            "endpoint {} moved from {} to {endpoint}",
            spec.endpoint_host, session.endpoint
        ));
        session.endpoint = endpoint;
        Ok(())
    }
}

impl Tunnel for WireGuardTunnel {
    fn up(&self, spec: &TunnelSpec) -> io::Result<TunnelLink> {
        if self.sessions.lock().unwrap().contains_key(&spec.name) {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                format!("{} is already up", spec.adapter_name()),
            ));
        }
        let session = self.open(spec)?;
        let link = session.link;
        self.sessions
            .lock()
            .unwrap()
            .insert(spec.name.clone(), Arc::new(Mutex::new(session)));
        Ok(link)
    }

    /// Takes the adapter down and closes it (which removes it); nothing to do when it is not up.
    fn down(&self, name: &str) -> io::Result<()> {
        let Some(session) = self.sessions.lock().unwrap().remove(name) else {
            return Ok(());
        };
        let adapter = session.lock().unwrap().adapter.take();
        match adapter {
            Some(adapter) => adapter.set_state(AdapterState::Down),
            None => Ok(()),
        }
    }

    fn health(&self, name: &str) -> io::Result<TunnelHealth> {
        let session = self.session(name)?;
        let session = session.lock().unwrap();
        match &session.adapter {
            Some(adapter) => adapter.config(),
            None => Err(io::Error::new(
                io::ErrorKind::NotConnected,
                format!("the adapter of {name} is gone"),
            )),
        }
    }

    fn refresh(&self, spec: &TunnelSpec) -> io::Result<TunnelLink> {
        let session = self.session(&spec.name)?;
        let mut session = session.lock().unwrap();
        let Some(if_index) = self.current_index(&session) else {
            self.log
                .with_profile(&spec.name)
                .warn("adapter is gone or changed; recreating it");
            // Closed first: the new adapter takes the same name.
            session.adapter = None;
            *session = self.open(spec)?;
            return Ok(session.link);
        };
        session.link.if_index = if_index;
        let adapter = session
            .adapter
            .as_deref()
            .expect("checked by current_index");
        self.restore(adapter, session.link.luid, spec)?;
        self.follow_endpoint(&mut session, spec)?;
        Ok(session.link)
    }

    fn driver_version(&self) -> Option<DriverVersion> {
        self.driver.running_version().ok().flatten()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::clock::ManualClock;
    use crate::net::Ipv4Net;
    use crate::profile::SecretKey;
    use crate::routes::RouteGuard;
    use crate::tunnel::{DriverVersion, FakeDns};
    use std::collections::HashSet;
    use std::net::Ipv4Addr;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::UNIX_EPOCH;

    /// Every call the fakes see, in order.
    #[derive(Default)]
    struct Journal(Mutex<Vec<String>>);

    impl Journal {
        fn push(&self, entry: impl Into<String>) {
            self.0.lock().unwrap().push(entry.into());
        }

        fn take(&self) -> Vec<String> {
            std::mem::take(&mut *self.0.lock().unwrap())
        }
    }

    /// What a test may change about one fake adapter.
    struct AdapterFacts {
        luid: u64,
        up: bool,
        gone: bool,
        fail_config: bool,
        health: TunnelHealth,
    }

    struct FakeAdapter {
        facts: Arc<Mutex<AdapterFacts>>,
        journal: Arc<Journal>,
    }

    fn gone() -> io::Error {
        io::Error::from(io::ErrorKind::NotFound)
    }

    impl WireGuardAdapter for FakeAdapter {
        fn luid(&self) -> u64 {
            self.facts.lock().unwrap().luid
        }

        fn set_config(&self, config: &DriverConfig) -> io::Result<()> {
            self.journal.push(match config {
                DriverConfig::Full(spec) => format!("config full {}", spec.endpoint),
                DriverConfig::Endpoint { endpoint, .. } => format!("config update-only {endpoint}"),
            });
            if self.facts.lock().unwrap().fail_config {
                return Err(io::Error::other("refused"));
            }
            Ok(())
        }

        fn config(&self) -> io::Result<TunnelHealth> {
            let facts = self.facts.lock().unwrap();
            if facts.gone {
                return Err(gone());
            }
            Ok(facts.health.clone())
        }

        fn set_state(&self, state: AdapterState) -> io::Result<()> {
            self.journal.push(format!("state {state:?}"));
            let mut facts = self.facts.lock().unwrap();
            if facts.gone {
                return Err(gone());
            }
            facts.up = state == AdapterState::Up;
            Ok(())
        }

        fn state(&self) -> io::Result<AdapterState> {
            let facts = self.facts.lock().unwrap();
            match (facts.gone, facts.up) {
                (true, _) => Err(gone()),
                (false, true) => Ok(AdapterState::Up),
                (false, false) => Ok(AdapterState::Down),
            }
        }

        fn set_logging(&self, on: bool) -> io::Result<()> {
            self.journal.push(format!("logging {on}"));
            Ok(())
        }
    }

    impl Drop for FakeAdapter {
        fn drop(&mut self) {
            self.journal.push("close");
        }
    }

    struct FakeDriver {
        journal: Arc<Journal>,
        adapters: Mutex<Vec<Arc<Mutex<AdapterFacts>>>>,
        next_luid: AtomicU64,
        foreign: Mutex<bool>,
        fail_config: Mutex<bool>,
    }

    impl FakeDriver {
        fn facts(&self, nth: usize) -> Arc<Mutex<AdapterFacts>> {
            self.adapters.lock().unwrap()[nth].clone()
        }

        fn adapter(&self, facts: Arc<Mutex<AdapterFacts>>) -> Box<dyn WireGuardAdapter> {
            Box::new(FakeAdapter {
                facts,
                journal: self.journal.clone(),
            })
        }
    }

    impl WireGuardDriver for FakeDriver {
        fn create(
            &self,
            name: &str,
            tunnel_type: &str,
            guid: u128,
        ) -> io::Result<Box<dyn WireGuardAdapter>> {
            self.journal
                .push(format!("create {name} {tunnel_type} {guid}"));
            let facts = Arc::new(Mutex::new(AdapterFacts {
                luid: self.next_luid.fetch_add(1, Ordering::SeqCst),
                up: false,
                gone: false,
                fail_config: *self.fail_config.lock().unwrap(),
                health: health(None, 0),
            }));
            self.adapters.lock().unwrap().push(facts.clone());
            Ok(self.adapter(facts))
        }

        fn open(&self, name: &str) -> io::Result<Option<Box<dyn WireGuardAdapter>>> {
            self.journal.push(format!("open {name}"));
            if !*self.foreign.lock().unwrap() {
                return Ok(None);
            }
            let facts = Arc::new(Mutex::new(AdapterFacts {
                luid: 1,
                up: true,
                gone: false,
                fail_config: false,
                health: health(None, 0),
            }));
            Ok(Some(self.adapter(facts)))
        }

        fn running_version(&self) -> io::Result<Option<DriverVersion>> {
            Ok(None)
        }

        fn delete_driver(&self) -> io::Result<()> {
            Ok(())
        }
    }

    /// Adapter LUID `n` has index `n + 100` until it is gone.
    struct FakeAddressing {
        journal: Arc<Journal>,
        applied: Mutex<bool>,
        gone: Mutex<HashSet<u64>>,
    }

    impl AdapterAddressing for FakeAddressing {
        fn apply(&self, luid: u64, settings: &AdapterSettings) -> io::Result<()> {
            self.journal.push(format!(
                "apply {luid} {}/32 mtu {}",
                settings.address, settings.mtu
            ));
            *self.applied.lock().unwrap() = true;
            Ok(())
        }

        fn applied(&self, _luid: u64, _settings: &AdapterSettings) -> io::Result<bool> {
            Ok(*self.applied.lock().unwrap())
        }

        fn index(&self, luid: u64) -> io::Result<u32> {
            if self.gone.lock().unwrap().contains(&luid) {
                return Err(gone());
            }
            Ok(luid as u32 + 100)
        }
    }

    fn health(handshake: Option<u64>, tx_bytes: u64) -> TunnelHealth {
        TunnelHealth {
            interface_public_key: [0x42; 32],
            last_handshake: handshake.map(|secs| UNIX_EPOCH + Duration::from_secs(secs)),
            rx_bytes: 0,
            tx_bytes,
            endpoint: None,
        }
    }

    fn spec() -> TunnelSpec {
        TunnelSpec {
            name: "AcmeCorp".into(),
            adapter_guid: 7,
            address: Ipv4Addr::new(10, 88, 4, 12),
            mtu: 1420,
            private_key: SecretKey::new([0xc3; 32]),
            peer_public_key: [0x11; 32],
            preshared_key: None,
            endpoint_host: "vpn.acme.example".into(),
            endpoint: "198.51.100.7:51820".parse().unwrap(),
            keepalive: 25,
        }
    }

    struct Rig {
        journal: Arc<Journal>,
        driver: Arc<FakeDriver>,
        addressing: Arc<FakeAddressing>,
        dns: Arc<FakeDns>,
        clock: Arc<ManualClock>,
        log: Log,
        tunnel: WireGuardTunnel,
    }

    fn rig() -> Rig {
        let journal = Arc::new(Journal::default());
        let driver = Arc::new(FakeDriver {
            journal: journal.clone(),
            adapters: Mutex::default(),
            next_luid: AtomicU64::new(1001),
            foreign: Mutex::new(false),
            fail_config: Mutex::new(false),
        });
        let addressing = Arc::new(FakeAddressing {
            journal: journal.clone(),
            applied: Mutex::new(false),
            gone: Mutex::default(),
        });
        let dns = Arc::new(FakeDns::default());
        dns.answer("vpn.acme.example", &["198.51.100.7"]);
        let clock = Arc::new(ManualClock::new(UNIX_EPOCH + Duration::from_secs(1_000)));
        let log = Log::new(100_000, clock.clone(), false);
        let tunnel = WireGuardTunnel::new(
            driver.clone(),
            addressing.clone(),
            dns.resolver(),
            clock.clone(),
            log.clone(),
        );
        Rig {
            journal,
            driver,
            addressing,
            dns,
            clock,
            log,
            tunnel,
        }
    }

    impl Rig {
        fn advance(&self, secs: u64) {
            self.clock.advance(Duration::from_secs(secs));
        }

        fn set_health(&self, nth: usize, health: TunnelHealth) {
            self.driver.facts(nth).lock().unwrap().health = health;
        }

        fn queries(&self) -> u32 {
            *self.dns.queries.lock().unwrap()
        }
    }

    #[test]
    fn up_creates_addresses_configures_and_starts_in_order() {
        let rig = rig();
        let link = rig.tunnel.up(&spec()).unwrap();
        assert_eq!(
            link,
            TunnelLink {
                luid: 1001,
                if_index: 1101
            }
        );
        assert_eq!(
            rig.journal.take(),
            [
                "open SplitWire-AcmeCorp",
                "create SplitWire-AcmeCorp SplitWire 7",
                "logging true",
                "apply 1001 10.88.4.12/32 mtu 1420",
                "config full 198.51.100.7:51820",
                "state Up",
            ]
        );
        assert!(rig.driver.facts(0).lock().unwrap().up);
        assert_eq!(
            rig.tunnel.health("AcmeCorp").unwrap().interface_public_key,
            [0x42; 32]
        );
        let again = rig.tunnel.up(&spec()).unwrap_err();
        assert_eq!(again.kind(), io::ErrorKind::AlreadyExists);
    }

    #[test]
    fn up_never_takes_over_an_adapter_another_process_holds() {
        let rig = rig();
        *rig.driver.foreign.lock().unwrap() = true;
        let error = rig.tunnel.up(&spec()).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::AlreadyExists);
        assert_eq!(UpStage::of(&error), Some(UpStage::Adapter));
        // Its handle is only closed again; nothing is created or configured.
        assert_eq!(rig.journal.take(), ["open SplitWire-AcmeCorp", "close"]);
        assert!(rig.tunnel.health("AcmeCorp").is_err());
    }

    #[test]
    fn a_failed_step_closes_the_new_adapter_and_leaves_nothing_up() {
        let rig = rig();
        *rig.driver.fail_config.lock().unwrap() = true;
        let error = rig.tunnel.up(&spec()).unwrap_err();
        assert_eq!(UpStage::of(&error), Some(UpStage::Peer));
        let journal = rig.journal.take();
        assert_eq!(journal.last().unwrap(), "close");
        assert!(!journal.iter().any(|entry| entry == "state Up"));
        assert_eq!(
            rig.tunnel.health("AcmeCorp").unwrap_err().kind(),
            io::ErrorKind::NotFound
        );
        *rig.driver.fail_config.lock().unwrap() = false;
        assert!(rig.tunnel.up(&spec()).is_ok());
    }

    #[test]
    fn down_stops_and_closes_the_adapter_once() {
        let rig = rig();
        rig.tunnel.up(&spec()).unwrap();
        rig.journal.take();
        rig.tunnel.down("AcmeCorp").unwrap();
        assert_eq!(rig.journal.take(), ["state Down", "close"]);
        rig.tunnel.down("AcmeCorp").unwrap();
        assert!(rig.journal.take().is_empty());
        assert!(rig.tunnel.refresh(&spec()).is_err());
    }

    #[test]
    fn refresh_recreates_a_vanished_adapter_with_a_new_link() {
        let rig = rig();
        rig.tunnel.up(&spec()).unwrap();
        rig.journal.take();
        rig.driver.facts(0).lock().unwrap().gone = true;
        rig.addressing.gone.lock().unwrap().insert(1001);
        let link = rig.tunnel.refresh(&spec()).unwrap();
        assert_eq!(
            link,
            TunnelLink {
                luid: 1002,
                if_index: 1102
            }
        );
        assert_eq!(
            rig.journal.take(),
            [
                "close",
                "open SplitWire-AcmeCorp",
                "create SplitWire-AcmeCorp SplitWire 7",
                "logging true",
                "apply 1002 10.88.4.12/32 mtu 1420",
                "config full 198.51.100.7:51820",
                "state Up",
            ]
        );
        assert!(rig.log.contains("recreating"));
    }

    #[test]
    fn refresh_recreates_an_adapter_whose_luid_changed() {
        let rig = rig();
        rig.tunnel.up(&spec()).unwrap();
        rig.driver.facts(0).lock().unwrap().luid = 4242;
        assert_eq!(rig.tunnel.refresh(&spec()).unwrap().luid, 1002);
    }

    #[test]
    fn a_failed_recreate_is_tried_again_at_the_next_refresh() {
        let rig = rig();
        rig.tunnel.up(&spec()).unwrap();
        rig.driver.facts(0).lock().unwrap().gone = true;
        *rig.driver.fail_config.lock().unwrap() = true;
        assert!(rig.tunnel.refresh(&spec()).is_err());
        assert_eq!(
            rig.tunnel.health("AcmeCorp").unwrap_err().kind(),
            io::ErrorKind::NotConnected
        );
        *rig.driver.fail_config.lock().unwrap() = false;
        assert_eq!(rig.tunnel.refresh(&spec()).unwrap().luid, 1003);
        assert!(rig.tunnel.health("AcmeCorp").is_ok());
    }

    #[test]
    fn refresh_restores_a_down_adapter_or_a_lost_address_in_place() {
        let rig = rig();
        let link = rig.tunnel.up(&spec()).unwrap();
        rig.journal.take();
        assert_eq!(rig.tunnel.refresh(&spec()).unwrap(), link);
        assert!(rig.journal.take().is_empty());
        rig.driver.facts(0).lock().unwrap().up = false;
        assert_eq!(rig.tunnel.refresh(&spec()).unwrap(), link);
        assert_eq!(
            rig.journal.take(),
            ["apply 1001 10.88.4.12/32 mtu 1420", "state Up"]
        );
        *rig.addressing.applied.lock().unwrap() = false;
        rig.tunnel.refresh(&spec()).unwrap();
        assert_eq!(
            rig.journal.take(),
            ["apply 1001 10.88.4.12/32 mtu 1420", "state Up"]
        );
    }

    #[test]
    fn a_stale_peer_follows_its_moved_endpoint_with_update_only() {
        let rig = rig();
        rig.tunnel.up(&spec()).unwrap();
        rig.set_health(0, health(Some(1_005), 100));
        rig.advance(10);
        rig.tunnel.refresh(&spec()).unwrap();
        assert_eq!(rig.queries(), 0);
        rig.journal.take();
        // The endpoint moved, the handshake is 200 s old, and the tunnel kept sending.
        rig.dns.answer("vpn.acme.example", &["203.0.113.9"]);
        rig.set_health(0, health(Some(1_005), 900));
        rig.advance(195);
        rig.tunnel.refresh(&spec()).unwrap();
        assert_eq!(rig.queries(), 1);
        assert_eq!(rig.journal.take(), ["config update-only 203.0.113.9:51820"]);
        assert!(
            rig.log
                .contains("moved from 198.51.100.7:51820 to 203.0.113.9:51820")
        );
        // At most one lookup a minute, while it keeps sending.
        rig.set_health(0, health(Some(1_005), 1_000));
        rig.advance(30);
        rig.tunnel.refresh(&spec()).unwrap();
        assert_eq!(rig.queries(), 1);
        rig.set_health(0, health(Some(1_005), 1_100));
        rig.advance(31);
        rig.tunnel.refresh(&spec()).unwrap();
        assert_eq!(rig.queries(), 2);
        assert!(rig.journal.take().is_empty());
    }

    #[test]
    fn every_endpoint_address_becomes_a_never_route() {
        let rig = rig();
        rig.tunnel.up(&spec()).unwrap();
        rig.dns
            .answer("vpn.acme.example", &["203.0.113.9", "203.0.113.10"]);
        rig.advance(31); // handshaking: no handshake within 30 s
        rig.tunnel.refresh(&spec()).unwrap();
        let endpoints = rig.tunnel.resolver.addresses().all();
        assert_eq!(
            endpoints,
            [
                Ipv4Addr::new(198, 51, 100, 7),
                Ipv4Addr::new(203, 0, 113, 9),
                Ipv4Addr::new(203, 0, 113, 10),
            ]
        );
        let guard = RouteGuard::new(&[]).forbid_hosts(&endpoints, "an endpoint");
        for net in ["198.51.100.7/32", "203.0.113.0/24", "203.0.113.10/32"] {
            let net: Ipv4Net = net.parse().unwrap();
            assert!(guard.check(&net).is_err(), "{net}");
        }
    }

    #[test]
    fn an_unchanged_or_unresolvable_endpoint_keeps_the_peer() {
        let rig = rig();
        rig.tunnel.up(&spec()).unwrap();
        rig.journal.take();
        rig.advance(31);
        rig.tunnel.refresh(&spec()).unwrap();
        assert_eq!(rig.queries(), 1);
        rig.dns.answers.lock().unwrap().clear();
        rig.advance(61);
        rig.tunnel.refresh(&spec()).unwrap();
        assert_eq!(rig.queries(), 2);
        assert!(rig.journal.take().is_empty());
        assert!(rig.log.contains("did not resolve"));
    }
}
