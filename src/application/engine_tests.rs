//! The coordinator over fakes it can break at will: every Connect step failing with its exact
//! rollback while another profile stays untouched, boot and crash restore, settings
//! applied live and by reconnect, tunnel recovery, re-homing, the first handshake and the
//! upstream's warm-up, and the redaction of keys.

use std::collections::HashMap;
use std::fs;
use std::io;
use std::net::{Ipv4Addr, UdpSocket};
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use super::data_folder::tests::scratch;
use super::guard_source::tests::FakeProbe;
use super::{Coordinator, DataFolder, Engine, EngineConfig, EngineParts, ProfileSet};
use crate::alerts::AlertKind;
use crate::clock::{Clock, ManualClock};
use crate::config::{SettingChange, SettingsScope};
use crate::dns::wire::{DnsQuery, DnsResponse, RCODE_NOERROR, RCODE_SERVFAIL, TYPE_A};
use crate::dns::{
    DnsExchange, DnsExchangeFactory, NrptPolicyProbe, NrptRules, UpstreamWarmup, Zone,
    zone_exchange,
};
use crate::list::DomainName;
use crate::logs::Log;
use crate::net::Ipv4Net;
use crate::platform::composition::PlatformSet;
use crate::platform::dry_run::{
    DryRunAddressing, DryRunDnsFlush, DryRunFolderAccess, DryRunNrpt, DryRunRouteTable,
};
use crate::profile::fixtures::{FakeStore, OTHER_PRIVATE, PRIVATE, conf};
use crate::profile::{Pin, ProfileState};
use crate::routes::RouteTable;
use crate::tunnel::{
    AdapterState, ConnectStep, DriverConfig, DriverVersion, StepResult, TunnelHealth,
    WireGuardAdapter, WireGuardDriver,
};

/// When the fake peers handshake: at the `after`-th health read since `after` was set (the
/// first by default), never while it is `None`.
struct Handshakes {
    after: Mutex<Option<u32>>,
    reads: AtomicU32,
}

impl Default for Handshakes {
    fn default() -> Self {
        Self {
            after: Mutex::new(Some(1)),
            reads: AtomicU32::new(0),
        }
    }
}

impl Handshakes {
    fn at() -> SystemTime {
        UNIX_EPOCH + Duration::from_secs(1_792_400_000)
    }

    fn after(&self, reads: Option<u32>) {
        *self.after.lock().unwrap() = reads;
        self.reads.store(0, Ordering::SeqCst);
    }

    fn read(&self) -> Option<SystemTime> {
        let reads = self.reads.fetch_add(1, Ordering::SeqCst) + 1;
        let after = *self.after.lock().unwrap();
        after.is_some_and(|after| reads >= after).then(Self::at)
    }
}

struct FakeAdapter {
    luid: u64,
    gone: Arc<AtomicBool>,
    fail_config: bool,
    handshakes: Arc<Handshakes>,
}

impl WireGuardAdapter for FakeAdapter {
    fn luid(&self) -> u64 {
        self.luid
    }
    fn set_config(&self, _config: &DriverConfig) -> io::Result<()> {
        if self.fail_config {
            return Err(io::Error::other("the driver refused the configuration"));
        }
        Ok(())
    }
    fn config(&self) -> io::Result<TunnelHealth> {
        if self.gone.load(Ordering::SeqCst) {
            return Err(io::ErrorKind::NotFound.into());
        }
        Ok(TunnelHealth {
            interface_public_key: [0; 32],
            last_handshake: self.handshakes.read(),
            rx_bytes: 0,
            tx_bytes: 0,
            endpoint: None,
        })
    }
    fn set_state(&self, _state: AdapterState) -> io::Result<()> {
        Ok(())
    }
    fn state(&self) -> io::Result<AdapterState> {
        if self.gone.load(Ordering::SeqCst) {
            return Err(io::ErrorKind::NotFound.into());
        }
        Ok(AdapterState::Up)
    }
    fn set_logging(&self, _on: bool) -> io::Result<()> {
        Ok(())
    }
}

/// Adapters that can fail to be created or configured, or vanish, and whose peers handshake
/// when told.
#[derive(Default)]
struct FakeDriver {
    next: AtomicU64,
    fail_create: AtomicBool,
    fail_config: AtomicBool,
    gone: Mutex<HashMap<u64, Arc<AtomicBool>>>,
    handshakes: Arc<Handshakes>,
}

impl FakeDriver {
    fn vanish(&self, luid: u64) {
        self.gone.lock().unwrap()[&luid].store(true, Ordering::SeqCst);
    }
}

impl WireGuardDriver for FakeDriver {
    fn create(
        &self,
        _name: &str,
        _kind: &str,
        _guid: u128,
    ) -> io::Result<Box<dyn WireGuardAdapter>> {
        if self.fail_create.load(Ordering::SeqCst) {
            return Err(io::Error::other("the driver could not be installed"));
        }
        let luid = 0x100 + self.next.fetch_add(1, Ordering::SeqCst);
        let gone = Arc::new(AtomicBool::new(false));
        self.gone.lock().unwrap().insert(luid, gone.clone());
        Ok(Box::new(FakeAdapter {
            luid,
            gone,
            fail_config: self.fail_config.load(Ordering::SeqCst),
            handshakes: self.handshakes.clone(),
        }))
    }
    fn open(&self, _name: &str) -> io::Result<Option<Box<dyn WireGuardAdapter>>> {
        Ok(None)
    }
    fn running_version(&self) -> io::Result<Option<DriverVersion>> {
        Ok(None)
    }
    fn delete_driver(&self) -> io::Result<()> {
        Ok(())
    }
}

struct FlakyTable {
    inner: DryRunRouteTable,
    fail_list: AtomicBool,
}

impl RouteTable for FlakyTable {
    fn list_owned(&self, luid: u64) -> io::Result<Vec<Ipv4Net>> {
        if self.fail_list.load(Ordering::SeqCst) {
            return Err(io::Error::from(io::ErrorKind::PermissionDenied));
        }
        self.inner.list_owned(luid)
    }
    fn add(&self, luid: u64, net: Ipv4Net) -> io::Result<()> {
        self.inner.add(luid, net)
    }
    fn remove(&self, luid: u64, net: Ipv4Net) -> io::Result<()> {
        self.inner.remove(luid, net)
    }
}

struct FlakyNrpt {
    inner: DryRunNrpt,
    fail_add: AtomicBool,
}

impl NrptRules for FlakyNrpt {
    fn list(&self) -> io::Result<Vec<DomainName>> {
        self.inner.list()
    }
    fn add(&self, domain: &DomainName) -> io::Result<()> {
        if self.fail_add.load(Ordering::SeqCst) {
            return Err(io::Error::from(io::ErrorKind::PermissionDenied));
        }
        self.inner.add(domain)
    }
    fn remove(&self, domain: &DomainName) -> io::Result<()> {
        self.inner.remove(domain)
    }
}

#[derive(Default)]
struct FakePolicy(AtomicBool);

impl NrptPolicyProbe for FakePolicy {
    fn policy_rules_exist(&self) -> io::Result<bool> {
        Ok(self.0.load(Ordering::SeqCst))
    }
}

/// Bound exchanges of the zone, named `<source>#<index>`.
struct ZoneFactory {
    zone: Arc<Zone>,
}

impl DnsExchangeFactory for ZoneFactory {
    fn bound(&self, source: Ipv4Addr, if_index: u32) -> Arc<dyn DnsExchange> {
        zone_exchange(&self.zone, &format!("{source}#{if_index}"))
    }
}

struct Rig {
    folder: DataFolder,
    clock: Arc<ManualClock>,
    log: Log,
    zone: Arc<Zone>,
    driver: Arc<FakeDriver>,
    table: Arc<FlakyTable>,
    nrpt: Arc<FlakyNrpt>,
    policy: Arc<FakePolicy>,
    flusher: Arc<DryRunDnsFlush>,
}

const ACME_LIST: &str = "acme.example\ncorp.example\n172.20.0.0/16\n";
const CONTOSO_LIST: &str = "contoso.example\n";

/// AcmeCorp (endpoint by name), Contoso (an IP literal) and HomeLab (an endpoint that does
/// not resolve), with their lists.
fn rig(name: &str) -> Rig {
    let folder = scratch(name);
    let clock = Arc::new(ManualClock::new(
        UNIX_EPOCH + Duration::from_secs(1_792_400_000),
    ));
    let log = Log::new(1 << 22, clock.clone(), false);
    let mut profiles =
        ProfileSet::load(folder.clone(), Arc::new(FakeStore), clock.clone()).unwrap();
    for (n, (profile, key, address, endpoint, list)) in [
        (
            "AcmeCorp",
            PRIVATE,
            "10.88.4.12/24",
            "vpn.acme.example:51820",
            ACME_LIST,
        ),
        (
            "Contoso",
            OTHER_PRIVATE,
            "10.77.0.5/32",
            "198.51.100.8:51820",
            CONTOSO_LIST,
        ),
        (
            "HomeLab",
            PRIVATE,
            "10.66.0.2/32",
            "home.example.net:51820",
            "lab.home.example\n",
        ),
    ]
    .into_iter()
    .enumerate()
    {
        let text = conf(key, address, "10.66.0.53", endpoint);
        profiles
            .import(profile, &text, Pin::new(n as u8 + 1).unwrap(), false)
            .unwrap();
        let stored = profiles.find(profile).unwrap().name;
        fs::write(folder.list(&stored), list).unwrap();
    }
    let zone = Arc::new(Zone::default());
    zone.answer("vpn.acme.example", &["203.0.113.9"]);
    zone.answer("www.acme.example", &["10.1.0.5"]);
    zone.answer("x.b.corp.example", &["10.2.0.7"]);
    zone.answer("cdn.acme.example", &["192.0.2.93"]);
    zone.answer("www.contoso.example", &["192.0.2.93", "192.0.2.94"]);
    Rig {
        folder,
        log: log.clone(),
        clock,
        zone,
        driver: Arc::default(),
        table: Arc::new(FlakyTable {
            inner: DryRunRouteTable::new(log.clone()),
            fail_list: AtomicBool::new(false),
        }),
        nrpt: Arc::new(FlakyNrpt {
            inner: DryRunNrpt::new(log.clone()),
            fail_add: AtomicBool::new(false),
        }),
        policy: Arc::default(),
        flusher: Arc::new(DryRunDnsFlush::new(log)),
    }
}

impl Rig {
    fn platform(&self) -> PlatformSet {
        PlatformSet {
            driver: self.driver.clone(),
            addressing: Arc::new(DryRunAddressing::new(self.log.clone())),
            routes: self.table.clone(),
            nrpt: self.nrpt.clone(),
            nrpt_policy: self.policy.clone(),
            flusher: self.flusher.clone(),
            probe: Arc::new(FakeProbe),
            exchange: zone_exchange(&self.zone, "fallback"),
            bound: Arc::new(ZoneFactory {
                zone: self.zone.clone(),
            }),
            secrets: Arc::new(FakeStore),
            folder: Arc::new(DryRunFolderAccess::new(self.log.clone())),
            dry_run: true,
        }
    }

    fn start_on(&self, listen: &str) -> Arc<Coordinator> {
        let config = EngineConfig {
            folder: self.folder.clone(),
            listen: Some(listen.parse().unwrap()),
            connect: Vec::new(),
            restore_all: false,
        };
        Engine::prepare(config, self.platform(), self.clock.clone(), &self.log).unwrap()
    }

    fn start(&self) -> Arc<Coordinator> {
        self.start_on("127.0.0.1:0")
    }

    fn write(&self, profile: &str, file: &str, text: &str) {
        let path = self.folder.profiles().join(profile).join(file);
        fs::write(path, text).unwrap();
    }

    /// KeepLab: a peer with a keepalive, which handshakes on its own once up, listing
    /// `keep.example`.
    fn keep_lab(&self, coordinator: &Coordinator) {
        let text = conf(
            OTHER_PRIVATE,
            "10.55.0.2/32",
            "10.66.0.53",
            "198.51.100.20:51820",
        );
        let text = format!("{text}PersistentKeepalive = 25\n");
        coordinator
            .import("KeepLab", &text, Pin::new(4).unwrap(), false)
            .unwrap();
        self.write("KeepLab", "routes.txt", "keep.example\n");
        coordinator.reload_lists();
        self.zone.answer("www.keep.example", &["10.9.0.5"]);
        self.zone.answer("db.keep.example", &["10.9.0.6"]);
    }
}

impl Drop for Rig {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(self.folder.root());
    }
}

/// What a failed Connect of another profile must leave as it was.
#[derive(Debug, PartialEq)]
struct World {
    routes: Vec<(u64, Ipv4Net)>,
    rules: Vec<String>,
    connected: Vec<String>,
    claims: usize,
    acme_listed: bool,
}

fn world(rig: &Rig, parts: &EngineParts) -> World {
    World {
        routes: rig.table.inner.all(),
        rules: rig.nrpt.inner.rules(),
        connected: parts.active.names(),
        claims: parts.claims.lock().unwrap().len(),
        acme_listed: parts.matcher.current().owner("www.acme.example").is_some(),
    }
}

fn ask(parts: &EngineParts, name: &str) -> Vec<Ipv4Addr> {
    let outcome = parts.dns.answer(&DnsQuery::build(7, name, TYPE_A)).unwrap();
    outcome.listed.map(|l| l.learn).unwrap_or_default()
}

fn rcode(parts: &EngineParts, name: &str) -> u8 {
    let outcome = parts.dns.answer(&DnsQuery::build(7, name, TYPE_A)).unwrap();
    DnsResponse::parse(outcome.response).unwrap().rcode()
}

fn detail(coordinator: &Coordinator, name: &str, step: ConnectStep) -> String {
    let attempt = coordinator.parts().states.get(name).last_attempt.unwrap();
    attempt.steps[step.number() - 1].detail.clone()
}

/// The names asked through the bound exchanges of the adapter address `source`.
fn asked_from(rig: &Rig, source: &str) -> Vec<String> {
    let prefix = format!("{source}#");
    let calls = rig.zone.calls().into_iter();
    calls
        .filter(|(via, _, _)| via.starts_with(&prefix))
        .map(|(_, _, name)| name)
        .collect()
}

fn alert(parts: &EngineParts, id: &str) -> Option<crate::alerts::Alert> {
    parts.alerts.active().into_iter().find(|a| a.id == id)
}

fn results(coordinator: &Coordinator, name: &str) -> Vec<StepResult> {
    let attempt = coordinator.parts().states.get(name).last_attempt.unwrap();
    attempt.steps.iter().map(|s| s.result).collect()
}

use StepResult::{Done, Failed, NotRun, Skipped, Undone};

#[test]
fn every_failing_step_undoes_exactly_what_was_done_and_nothing_else() {
    let rig = rig("engine-steps");
    let coordinator = rig.start();
    let parts = coordinator.parts().clone();
    coordinator.connect("AcmeCorp").unwrap();
    ask(&parts, "www.acme.example");
    let before = world(&rig, &parts);
    let acme_link = *parts.active.get("AcmeCorp").unwrap().link.lock().unwrap();

    let expect =
        |step: ConnectStep, expected: [StepResult; 10], break_it: &dyn Fn(), fix: &dyn Fn()| {
            break_it();
            let error = coordinator.connect("Contoso").unwrap_err();
            fix();
            assert!(error.to_string().contains(&format!("({step})")), "{error}");
            assert_eq!(results(&coordinator, "Contoso"), expected, "{step}");
            assert_eq!(parts.states.state("Contoso"), ProfileState::Failed);
            assert_eq!(world(&rig, &parts), before, "{step}");
            assert_eq!(
                *parts.active.get("AcmeCorp").unwrap().link.lock().unwrap(),
                acme_link
            );
            assert!(parts.resolvers.get("Contoso").is_none());
            let acme_guard = parts.guards.guard_for("AcmeCorp");
            assert!(
                acme_guard
                    .check(&Ipv4Net::host(Ipv4Addr::new(10, 77, 0, 5)))
                    .is_ok()
            );
            assert!(
                parts
                    .alerts
                    .active()
                    .iter()
                    .any(|a| a.id == "connect_failed:Contoso:")
            );
        };

    expect(
        ConnectStep::Profile,
        [
            Failed, NotRun, NotRun, NotRun, NotRun, NotRun, NotRun, NotRun, NotRun, NotRun,
        ],
        &|| rig.write("Contoso", "profile.conf", "mtu=9000\n"),
        &|| rig.write("Contoso", "profile.conf", "connect_at_boot=false\n"),
    );
    expect(
        ConnectStep::Adapter,
        [
            Undone, Skipped, Undone, Failed, NotRun, NotRun, NotRun, NotRun, NotRun, NotRun,
        ],
        &|| rig.driver.fail_create.store(true, Ordering::SeqCst),
        &|| rig.driver.fail_create.store(false, Ordering::SeqCst),
    );
    expect(
        ConnectStep::Peer,
        [
            Undone, Skipped, Undone, Undone, Failed, NotRun, NotRun, NotRun, NotRun, NotRun,
        ],
        &|| rig.driver.fail_config.store(true, Ordering::SeqCst),
        &|| rig.driver.fail_config.store(false, Ordering::SeqCst),
    );
    expect(
        ConnectStep::Routes,
        [
            Undone, Skipped, Undone, Undone, Undone, Failed, NotRun, NotRun, NotRun, NotRun,
        ],
        &|| rig.table.fail_list.store(true, Ordering::SeqCst),
        &|| rig.table.fail_list.store(false, Ordering::SeqCst),
    );
    expect(
        ConnectStep::Nrpt,
        [
            Undone, Skipped, Undone, Undone, Undone, Undone, Undone, Undone, Failed, NotRun,
        ],
        &|| rig.nrpt.fail_add.store(true, Ordering::SeqCst),
        &|| rig.nrpt.fail_add.store(false, Ordering::SeqCst),
    );
    let state = rig.folder.state();
    expect(
        ConnectStep::State,
        [
            Undone, Skipped, Undone, Undone, Undone, Undone, Undone, Undone, Undone, Failed,
        ],
        &|| {
            fs::remove_file(&state).unwrap();
            fs::create_dir(&state).unwrap();
        },
        &|| {
            fs::remove_dir(&state).unwrap();
            fs::write(&state, "connected=AcmeCorp\n").unwrap();
        },
    );

    // The endpoint that does not resolve fails at the guard.
    let error = coordinator.connect("HomeLab").unwrap_err();
    assert!(
        error
            .to_string()
            .contains("home.example.net did not resolve"),
        "{error}"
    );
    assert_eq!(
        results(&coordinator, "HomeLab"),
        [
            Undone, Skipped, Failed, NotRun, NotRun, NotRun, NotRun, NotRun, NotRun, NotRun
        ]
    );

    // Fixed, it connects, and the next attempt cleared the alert.
    coordinator.connect("Contoso").unwrap();
    assert_eq!(
        results(&coordinator, "Contoso"),
        [
            Done, Skipped, Done, Done, Done, Done, Done, Done, Done, Done
        ]
    );
    assert!(
        parts
            .alerts
            .active()
            .iter()
            .all(|a| a.id != "connect_failed:Contoso:")
    );
    assert_eq!(
        fs::read_to_string(rig.folder.state()).unwrap(),
        "connected=AcmeCorp,Contoso\n"
    );
}

#[test]
fn the_first_connect_starts_the_proxy_and_a_busy_port_fails_at_step_2() {
    let rig = rig("engine-proxy");
    let busy = UdpSocket::bind("127.0.0.1:0").unwrap();
    let coordinator = rig.start_on(&busy.local_addr().unwrap().to_string());
    let error = coordinator.connect("AcmeCorp").unwrap_err();
    assert!(error.to_string().contains("(proxy)"), "{error}");
    assert_eq!(
        results(&coordinator, "AcmeCorp"),
        [
            Undone, Failed, NotRun, NotRun, NotRun, NotRun, NotRun, NotRun, NotRun, NotRun
        ]
    );
    drop(busy);
    let coordinator = rig.start();
    coordinator.connect("AcmeCorp").unwrap();
    assert_eq!(results(&coordinator, "AcmeCorp")[1], Done);
    coordinator.connect("Contoso").unwrap();
    assert_eq!(results(&coordinator, "Contoso")[1], Skipped);
    coordinator.disconnect("AcmeCorp").unwrap();
    assert!(
        !rig.nrpt.inner.rules().is_empty(),
        "Contoso keeps the proxy and its rules"
    );
    coordinator.disconnect("Contoso").unwrap();
    assert!(rig.nrpt.inner.rules().is_empty());
    assert!(rig.table.inner.all().is_empty());
}

#[test]
fn a_connect_leaves_a_connected_profile_as_it_is() {
    let rig = rig("engine-connect-twice");
    let coordinator = rig.start();
    let parts = coordinator.parts().clone();
    coordinator.connect("AcmeCorp").unwrap();
    let since = parts.active.get("AcmeCorp").unwrap().since;
    rig.clock.advance(Duration::from_secs(60));
    coordinator.connect("acmecorp").unwrap();
    assert_eq!(parts.active.get("AcmeCorp").unwrap().since, since);
    assert_eq!(
        fs::read_to_string(rig.folder.state()).unwrap(),
        "connected=AcmeCorp\n"
    );
}

#[test]
fn boot_restores_connect_at_boot_a_crash_restores_all_and_bad_settings_nothing() {
    let rig = rig("engine-boot");
    rig.write("AcmeCorp", "profile.conf", "connect_at_boot=true\n");
    fs::write(rig.folder.state(), "connected=AcmeCorp,Contoso\n").unwrap();
    let clean = rig.start();
    assert_eq!(clean.parts().active.names(), ["AcmeCorp"]);
    // The marker is still there: the next start sees a crash.
    clean.disconnect_all(true);
    let crashed = rig.start();
    assert_eq!(crashed.parts().active.names(), ["AcmeCorp", "Contoso"]);
    crashed.disconnect_all(true);
    fs::write(rig.folder.settings(), "max_ttl=5\n").unwrap();
    let held = rig.start();
    assert!(held.parts().active.is_empty());
    assert_eq!(held.parts().states.state("AcmeCorp"), ProfileState::HeldOff);
    assert!(
        held.parts()
            .alerts
            .active()
            .iter()
            .any(|a| a.kind() == AlertKind::SettingsInvalid)
    );
    assert_eq!(
        fs::read_to_string(rig.folder.state()).unwrap(),
        "connected=AcmeCorp,Contoso\n"
    );
    assert!(
        held.connect("AcmeCorp")
            .unwrap_err()
            .to_string()
            .contains("splitwire.conf is invalid")
    );
    fs::write(rig.folder.settings(), "max_ttl=60\n").unwrap();
    held.reload_settings();
    assert_eq!(held.parts().states.state("AcmeCorp"), ProfileState::Failed);
    assert_eq!(held.parts().states.state("Contoso"), ProfileState::Off);
    assert!(
        held.parts()
            .alerts
            .active()
            .iter()
            .all(|a| a.kind() != AlertKind::SettingsInvalid)
    );
}

#[test]
fn settings_apply_live_block_one_profile_or_reconnect_it() {
    let rig = rig("engine-settings");
    let coordinator = rig.start();
    let parts = coordinator.parts().clone();
    coordinator.connect("AcmeCorp").unwrap();
    rig.write("HomeLab", "profile.conf", "mtu=9000\n");
    coordinator.reload_settings();
    assert_eq!(parts.states.state("HomeLab"), ProfileState::Blocked);
    assert!(
        parts
            .alerts
            .active()
            .iter()
            .any(|a| a.id == "profile_settings_invalid:HomeLab:")
    );
    rig.write("HomeLab", "profile.conf", "\n");
    coordinator.reload_settings();
    assert_eq!(parts.states.state("HomeLab"), ProfileState::Off);

    let rev = coordinator.settings_rev();
    let changes = [
        SettingChange {
            scope: SettingsScope::Profile("AcmeCorp".into()),
            key: "mtu".into(),
            value: Some("1380".into()),
        },
        SettingChange {
            scope: SettingsScope::Global,
            key: "bypass".into(),
            value: Some("www.acme.example".into()),
        },
    ];
    coordinator.save_settings(rev, &changes).unwrap();
    assert_eq!(
        parts.active.get("AcmeCorp").unwrap().spec.mtu,
        1380,
        "reconnected"
    );
    assert!(parts.policy.current().bypass.matches("www.acme.example"));
    assert!(
        parts.policy.current().bypass.matches("vpn.acme.example"),
        "an endpoint host"
    );
    assert!(matches!(
        coordinator.save_settings(rev, &changes),
        Err(super::SaveRefused::Stale)
    ));
}

#[test]
fn a_vanished_adapter_is_recreated_and_routes_and_dns_follow_it() {
    let rig = rig("engine-recover");
    let coordinator = rig.start();
    let parts = coordinator.parts().clone();
    coordinator.connect("AcmeCorp").unwrap();
    ask(&parts, "www.acme.example");
    let old = *parts.active.get("AcmeCorp").unwrap().link.lock().unwrap();
    rig.driver.vanish(old.luid);
    rig.driver.fail_create.store(true, Ordering::SeqCst);
    coordinator.watch_tunnels();
    assert_eq!(parts.states.state("AcmeCorp"), ProfileState::Failed);
    assert!(
        parts
            .alerts
            .active()
            .iter()
            .any(|a| a.id == "recover_failed:AcmeCorp:")
    );
    rig.driver.fail_create.store(false, Ordering::SeqCst);
    coordinator.watch_tunnels();
    let new = *parts.active.get("AcmeCorp").unwrap().link.lock().unwrap();
    assert_ne!(new.luid, old.luid);
    assert!(matches!(
        parts.states.state("AcmeCorp"),
        ProfileState::Connected { .. }
    ));
    assert!(
        parts
            .alerts
            .active()
            .iter()
            .all(|a| a.kind() != AlertKind::RecoverFailed)
    );
    let routes = rig.table.inner.routes(new.luid);
    assert!(routes.contains(&Ipv4Net::host(Ipv4Addr::new(10, 1, 0, 5))));
    ask(&parts, "new.acme.example");
    let via = rig.zone.calls().last().unwrap().0.clone();
    assert_eq!(via, format!("10.88.4.12#{}", new.if_index));
}

#[test]
fn list_edits_rehome_learned_routes_and_follow_nrpt() {
    let rig = rig("engine-lists");
    let coordinator = rig.start();
    let parts = coordinator.parts().clone();
    coordinator.connect("AcmeCorp").unwrap();
    coordinator.connect("Contoso").unwrap();
    assert_eq!(
        ask(&parts, "x.b.corp.example"),
        [Ipv4Addr::new(10, 2, 0, 7)]
    );
    let acme = parts
        .active
        .get("AcmeCorp")
        .unwrap()
        .link
        .lock()
        .unwrap()
        .luid;
    let contoso = parts
        .active
        .get("Contoso")
        .unwrap()
        .link
        .lock()
        .unwrap()
        .luid;
    let learned = Ipv4Net::host(Ipv4Addr::new(10, 2, 0, 7));
    assert!(rig.table.inner.routes(acme).contains(&learned));
    let rev = coordinator.lists_rev().unwrap();
    coordinator
        .edit_list(rev, "Contoso", Some("b.corp.example"), None)
        .unwrap();
    assert!(!rig.table.inner.routes(acme).contains(&learned), "re-homed");
    assert!(
        rig.nrpt
            .inner
            .rules()
            .contains(&"b.corp.example".to_owned())
    );
    assert_eq!(
        ask(&parts, "x.b.corp.example"),
        [Ipv4Addr::new(10, 2, 0, 7)]
    );
    assert!(rig.table.inner.routes(contoso).contains(&learned));
    let stale = coordinator.edit_list(rev, "AcmeCorp", Some("new.example"), None);
    assert!(matches!(stale, Err(super::ListRefusal::Stale)));
    // A hand edit: a line that is not an entry becomes a list problem.
    rig.write("AcmeCorp", "routes.txt", "acme.example\nintranet\n");
    coordinator.reload_lists();
    assert!(
        parts
            .alerts
            .active()
            .iter()
            .any(|a| a.id == "list_problem:AcmeCorp:")
    );
    assert!(!rig.nrpt.inner.rules().contains(&"corp.example".to_owned()));
}

#[test]
fn shared_addresses_stay_with_the_first_learner_until_its_route_expires() {
    let rig = rig("engine-claims");
    let coordinator = rig.start();
    let parts = coordinator.parts().clone();
    coordinator.connect("AcmeCorp").unwrap();
    coordinator.connect("Contoso").unwrap();
    ask(&parts, "cdn.acme.example");
    assert_eq!(
        ask(&parts, "www.contoso.example"),
        [Ipv4Addr::new(192, 0, 2, 94)]
    );
    assert!(
        parts
            .alerts
            .active()
            .iter()
            .any(|a| a.id == "address_held:Contoso:192.0.2.93")
    );
    rig.clock.advance(Duration::from_secs(86_400));
    coordinator.maintain();
    assert!(
        parts
            .alerts
            .active()
            .iter()
            .all(|a| a.kind() != AlertKind::AddressHeld)
    );
    assert_eq!(
        ask(&parts, "www.contoso.example"),
        [Ipv4Addr::new(192, 0, 2, 93), Ipv4Addr::new(192, 0, 2, 94)]
    );
    rig.policy.0.store(true, Ordering::SeqCst);
    coordinator.maintain();
    assert!(
        parts
            .alerts
            .active()
            .iter()
            .any(|a| a.kind() == AlertKind::GpNrpt)
    );
}

#[test]
fn imports_replaces_and_removes_through_the_coordinator() {
    let rig = rig("engine-profiles");
    let coordinator = rig.start();
    let parts = coordinator.parts().clone();
    coordinator.connect("Contoso").unwrap();
    let text = conf(
        OTHER_PRIVATE,
        "10.77.0.5/32",
        "10.66.0.59",
        "198.51.100.8:51820",
    );
    let preview = coordinator.preview_import(&crate::profile::ImportRequest {
        name: "Contoso",
        file_name: "Contoso.conf",
        conf_text: &text,
    });
    assert!(preview.replace);
    coordinator
        .import("Contoso", &text, Pin::new(2).unwrap(), true)
        .unwrap();
    assert_eq!(
        parts.active.get("Contoso").unwrap().upstream[0].ip(),
        &Ipv4Addr::new(10, 66, 0, 59)
    );
    coordinator.remove("Contoso").unwrap();
    assert!(parts.active.is_empty());
    assert!(
        coordinator
            .profiles()
            .iter()
            .all(|(name, _)| name != "Contoso")
    );
}

#[test]
fn traffic_samples_follow_the_byte_counters_once_a_second() {
    let rig = rig("engine-traffic");
    let coordinator = rig.start();
    let parts = coordinator.parts().clone();
    coordinator.connect("AcmeCorp").unwrap();
    let sampler = super::TrafficSampler::new(
        parts.active.clone(),
        parts.tunnel.clone(),
        rig.clock.clone(),
    );
    sampler.tick();
    rig.clock.advance(Duration::from_secs(1));
    sampler.tick();
    let acme = parts.active.get("AcmeCorp").unwrap();
    let samples = acme.traffic.lock().unwrap().after(0);
    assert_eq!(samples.len(), 1);
    assert_eq!(samples[0].seq, 2);
    assert_eq!(sampler.seq(), 2);
}

#[test]
fn no_key_reaches_the_log_the_alerts_or_the_attempts() {
    let rig = rig("engine-redaction");
    let coordinator = rig.start();
    coordinator.connect("AcmeCorp").unwrap();
    coordinator.connect("HomeLab").unwrap_err();
    coordinator.disconnect_all(false);
    let mut text: Vec<String> = rig
        .log
        .since(0, usize::MAX)
        .into_iter()
        .map(|e| e.message)
        .collect();
    text.extend(
        coordinator
            .parts()
            .alerts
            .active()
            .iter()
            .map(|a| format!("{a:?}")),
    );
    for name in ["AcmeCorp", "HomeLab"] {
        text.push(format!("{:?}", coordinator.parts().states.get(name)));
    }
    let all = text.join("\n");
    for key in [PRIVATE, OTHER_PRIVATE] {
        assert!(!all.contains(key));
        assert!(!all.contains(&key[..12]));
    }
}

#[test]
fn connect_waits_for_the_first_handshake_before_nrpt() {
    let rig = rig("engine-handshake");
    let coordinator = rig.start();
    let parts = coordinator.parts().clone();
    rig.keep_lab(&coordinator);
    rig.driver.handshakes.after(Some(3));
    let start = rig.clock.now();
    coordinator.connect("KeepLab").unwrap();
    // Three reads, two polls apart.
    assert_eq!(rig.clock.now(), start + Duration::from_millis(500));
    let dns = detail(&coordinator, "KeepLab", ConnectStep::Dns);
    assert!(
        dns.ends_with("; first handshake after 0.5 s") && !dns.contains("probe"),
        "{dns}"
    );
    assert!(rig.nrpt.inner.rules().contains(&"keep.example".to_owned()));
    assert!(alert(&parts, "handshake_stale:KeepLab:").is_none());
    // A keepalive peer handshakes on its own: no probe.
    assert!(asked_from(&rig, "10.55.0.2").is_empty());
}

#[test]
fn a_peer_without_a_keepalive_is_probed_once_then_awaited() {
    let rig = rig("engine-probe");
    let coordinator = rig.start();
    let parts = coordinator.parts().clone();
    rig.driver.handshakes.after(Some(4));
    let start = rig.clock.now();
    coordinator.connect("Contoso").unwrap();
    // One read before the probe, then three more, two polls apart.
    assert_eq!(rig.clock.now(), start + Duration::from_millis(500));
    assert_eq!(asked_from(&rig, "10.77.0.5"), ["contoso.example"]);
    let dns = detail(&coordinator, "Contoso", ConnectStep::Dns);
    assert!(
        dns.ends_with("; probe sent; first handshake after 0.5 s"),
        "{dns}"
    );
    assert!(
        rig.nrpt
            .inner
            .rules()
            .contains(&"contoso.example".to_owned())
    );
    // The probe's answer was the upstream's first: the first query flushes nothing more.
    let resolver = parts.resolvers.get("Contoso").unwrap();
    assert!(resolver.health().last_success.is_some());
    let flushes = rig.flusher.flushes();
    assert_eq!(
        ask(&parts, "www.contoso.example").len(),
        2,
        "answered through the tunnel"
    );
    assert_eq!(rig.flusher.flushes(), flushes);
}

#[test]
fn a_failed_probe_and_no_handshake_still_connect_with_an_alert() {
    let rig = rig("engine-probe-fails");
    let coordinator = rig.start();
    let parts = coordinator.parts().clone();
    rig.driver.handshakes.after(None);
    *rig.zone.down.lock().unwrap() = true;
    let start = rig.clock.now();
    coordinator.connect("Contoso").unwrap();
    assert_eq!(rig.clock.now(), start + Duration::from_secs(10));
    assert_eq!(asked_from(&rig, "10.77.0.5"), ["contoso.example"]);
    assert_eq!(
        results(&coordinator, "Contoso"),
        [Done, Done, Done, Done, Done, Done, Done, Done, Done, Done]
    );
    let dns = detail(&coordinator, "Contoso", ConnectStep::Dns);
    assert!(dns.ends_with("; probe sent; no handshake within 10 s; connecting anyway"));
    assert!(alert(&parts, "handshake_stale:Contoso:").is_some());
    // The failed probe is not a failure of the upstream.
    let resolver = parts.resolvers.get("Contoso").unwrap();
    assert_eq!(resolver.health().consecutive_failures, 0);
}

#[test]
fn a_connect_without_a_handshake_succeeds_with_an_alert_until_one_comes() {
    let rig = rig("engine-no-handshake");
    let coordinator = rig.start();
    let parts = coordinator.parts().clone();
    rig.keep_lab(&coordinator);
    rig.driver.handshakes.after(None);
    let start = rig.clock.now();
    coordinator.connect("KeepLab").unwrap();
    assert_eq!(rig.clock.now(), start + Duration::from_secs(10));
    assert_eq!(
        results(&coordinator, "KeepLab"),
        [Done, Done, Done, Done, Done, Done, Done, Done, Done, Done]
    );
    let dns = detail(&coordinator, "KeepLab", ConnectStep::Dns);
    assert!(dns.ends_with("; no handshake within 10 s; connecting anyway"));
    assert!(rig.nrpt.inner.rules().contains(&"keep.example".to_owned()));
    assert!(matches!(
        parts.states.state("KeepLab"),
        ProfileState::Connected { .. }
    ));
    let raised = alert(&parts, "handshake_stale:KeepLab:").unwrap();
    assert_eq!(raised.raised.title, "No handshake yet");

    rig.driver.handshakes.after(Some(1));
    coordinator.watch_tunnels();
    assert!(alert(&parts, "handshake_stale:KeepLab:").is_none());
}

#[test]
fn the_first_answers_after_connect_are_retried_flushed_once_and_raise_no_alert() {
    let rig = rig("engine-warmup");
    let coordinator = rig.start();
    let parts = coordinator.parts().clone();
    rig.keep_lab(&coordinator);
    rig.driver.handshakes.after(Some(1));
    coordinator.connect("KeepLab").unwrap();
    let flushes = rig.flusher.flushes();

    // The upstream times out three times, then answers within the budget: no SERVFAIL.
    *rig.zone.failing.lock().unwrap() = 3;
    assert_eq!(
        ask(&parts, "www.keep.example"),
        [Ipv4Addr::new(10, 9, 0, 5)]
    );
    assert_eq!(
        rig.flusher.flushes(),
        flushes + 1,
        "the first answer flushes"
    );
    *rig.zone.failing.lock().unwrap() = 2;
    assert_eq!(rcode(&parts, "db.keep.example"), RCODE_NOERROR);
    assert_eq!(rig.flusher.flushes(), flushes + 1, "once");
    coordinator.watch_tunnels();
    assert!(alert(&parts, "upstream_unreachable:KeepLab:").is_none());

    // Warmed up, a failure is SERVFAIL at once; three in a row raise the alert.
    rig.clock.advance(UpstreamWarmup::GRACE);
    *rig.zone.down.lock().unwrap() = true;
    for name in ["a.keep.example", "b.keep.example", "c.keep.example"] {
        let calls = rig.zone.calls().len();
        assert_eq!(rcode(&parts, name), RCODE_SERVFAIL);
        assert_eq!(rig.zone.calls().len(), calls + 1);
    }
    coordinator.watch_tunnels();
    let raised = alert(&parts, "upstream_unreachable:KeepLab:")
        .unwrap()
        .raised;
    assert_eq!(raised.message.matches("10.66.0.53:53").count(), 1);

    // The first answer after the failures flushes the cache once more, and clears the alert.
    *rig.zone.down.lock().unwrap() = false;
    assert_eq!(rcode(&parts, "d.keep.example"), RCODE_NOERROR);
    assert_eq!(rig.flusher.flushes(), flushes + 2);
    coordinator.watch_tunnels();
    assert!(alert(&parts, "upstream_unreachable:KeepLab:").is_none());
}

#[test]
fn inside_the_grace_a_spent_budget_counts_once_toward_upstream_unreachable() {
    let rig = rig("engine-grace");
    let coordinator = rig.start();
    let parts = coordinator.parts().clone();
    rig.keep_lab(&coordinator);
    rig.driver.handshakes.after(Some(1));
    coordinator.connect("KeepLab").unwrap();
    *rig.zone.down.lock().unwrap() = true;
    // The query is retried for its budget before its SERVFAIL, and that raises nothing.
    let start = rig.clock.now();
    assert_eq!(rcode(&parts, "a.keep.example"), RCODE_SERVFAIL);
    assert!(rig.clock.now() >= start + UpstreamWarmup::BUDGET - UpstreamWarmup::PAUSE);
    coordinator.watch_tunnels();
    assert!(alert(&parts, "upstream_unreachable:KeepLab:").is_none());
    // Two more in a row, retried until the grace ends and then at once: the alert is due.
    for name in ["b.keep.example", "c.keep.example"] {
        assert_eq!(rcode(&parts, name), RCODE_SERVFAIL);
    }
    coordinator.watch_tunnels();
    assert!(alert(&parts, "upstream_unreachable:KeepLab:").is_some());
}

#[test]
fn a_tunnel_idle_past_its_handshake_warms_up_again_instead_of_failing() {
    let rig = rig("engine-cold");
    let coordinator = rig.start();
    let parts = coordinator.parts().clone();
    rig.keep_lab(&coordinator);
    rig.driver.handshakes.after(Some(1));
    coordinator.connect("KeepLab").unwrap();
    assert_eq!(
        ask(&parts, "www.keep.example"),
        [Ipv4Addr::new(10, 9, 0, 5)]
    );
    let flushes = rig.flusher.flushes();

    // Idle: the last handshake grows older than WireGuard keeps its keys, as the watch reports.
    rig.clock
        .advance(UpstreamWarmup::COLD_AFTER + UpstreamWarmup::GRACE);
    coordinator.watch_tunnels();
    // The peer's new handshake takes three timeouts: the name is answered, nothing is raised.
    *rig.zone.failing.lock().unwrap() = 3;
    assert_eq!(rcode(&parts, "db.keep.example"), RCODE_NOERROR);
    assert_eq!(rig.flusher.flushes(), flushes, "no failure to forget");
    coordinator.watch_tunnels();
    assert!(alert(&parts, "upstream_unreachable:KeepLab:").is_none());
}
