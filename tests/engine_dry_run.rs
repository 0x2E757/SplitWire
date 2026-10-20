//! The engine integration test: `PlatformSet::dry_run()` with the proxy on
//! 127.0.0.1:15353 and two made-up profiles sharing one upstream DNS address (a fake DNS server
//! on loopback). Nothing on the machine changes: adapters, routes and NRPT are logging fakes.

use std::collections::{BTreeSet, HashMap};
use std::fs;
use std::io;
use std::net::{Ipv4Addr, SocketAddr, UdpSocket};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use splitwire::alerts::AlertKind;
use splitwire::application::{DataFolder, Engine, EngineConfig, ProfileSet};
use splitwire::clock::{Clock, SystemClock};
use splitwire::dns::wire::{DnsQuery, DnsResponse, TYPE_A};
use splitwire::dns::{NrptRules, ResponseBuilder};
use splitwire::list::DomainName;
use splitwire::logs::Log;
use splitwire::net::Ipv4Net;
use splitwire::platform::composition::PlatformSet;
use splitwire::platform::dry_run::{DryRunNrpt, DryRunRouteTable};
use splitwire::profile::{Pin, ProfileState};
use splitwire::tunnel::{ConnectStep, StepResult};

// Made-up keys; none of them is a real key.
const ACME_PRIVATE: &str = "yAnz5TF+lXXJte14tji3zlMNq+hd2rYUIgJBgB3fBmk=";
const CONTOSO_PRIVATE: &str = "cGFydG5lci1wcml2YXRlLWtleS1tYWRlLXVwLTAwMDA=";
const HOME_PRIVATE: &str = "aG9tZWxhYi1wcml2YXRlLWtleS1tYWRlLXVwLTAwMDA=";
const PEER_PUBLIC: &str = "xTIBA5rboUvnH4htodjb6e697QjLERt1NAB4mZqp8Dg=";
const LISTEN: &str = "127.0.0.1:15353";

fn conf(private_key: &str, address: &str, endpoint: &str) -> String {
    format!(
        "[Interface]\nPrivateKey = {private_key}\nAddress = {address}\nDNS = 10.66.0.53\n\
         [Peer]\nPublicKey = {PEER_PUBLIC}\nEndpoint = {endpoint}\n"
    )
}

/// A DNS server on loopback answering A queries from a table; the upstream of both profiles.
struct FakeUpstream {
    address: SocketAddr,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl FakeUpstream {
    fn start(zone: &[(&str, &str)]) -> Self {
        let zone: HashMap<String, Ipv4Addr> = zone
            .iter()
            .map(|(name, ip)| ((*name).to_owned(), ip.parse().unwrap()))
            .collect();
        let socket = UdpSocket::bind("127.0.0.1:0").unwrap();
        socket
            .set_read_timeout(Some(Duration::from_millis(100)))
            .unwrap();
        let address = socket.local_addr().unwrap();
        let stop = Arc::new(AtomicBool::new(false));
        let stopped = stop.clone();
        let thread = thread::spawn(move || {
            let mut buffer = [0u8; 512];
            while !stopped.load(Ordering::SeqCst) {
                let Ok((len, from)) = socket.recv_from(&mut buffer) else {
                    continue;
                };
                let query = DnsQuery::parse(&buffer[..len]).unwrap();
                let name = query.question.name.to_ascii_lowercase();
                let reply = zone
                    .get(&name)
                    .into_iter()
                    .fold(ResponseBuilder::new(&query), |b, ip| b.a(&name, *ip, 600))
                    .build();
                let _ = socket.send_to(&reply, from);
            }
        });
        Self {
            address,
            stop,
            thread: Some(thread),
        }
    }
}

impl Drop for FakeUpstream {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

/// The dry run's NRPT, refusing the rules of one domain (a failing step 9).
struct RefusingNrpt {
    inner: DryRunNrpt,
    refused: DomainName,
}

impl NrptRules for RefusingNrpt {
    fn list(&self) -> io::Result<Vec<DomainName>> {
        self.inner.list()
    }

    fn add(&self, domain: &DomainName) -> io::Result<()> {
        if *domain == self.refused {
            return Err(io::Error::from(io::ErrorKind::PermissionDenied));
        }
        self.inner.add(domain)
    }

    fn remove(&self, domain: &DomainName) -> io::Result<()> {
        self.inner.remove(domain)
    }
}

struct ScratchFolder(PathBuf);

impl Drop for ScratchFolder {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// Writes the three profiles, their lists and their `profile.conf` (upstream on loopback).
fn prepare(
    folder: &DataFolder,
    platform: &PlatformSet,
    clock: Arc<dyn Clock>,
    upstream: SocketAddr,
) {
    let mut profiles = ProfileSet::load(folder.clone(), platform.secrets.clone(), clock).unwrap();
    let profiles_to_write = [
        (
            "AcmeCorp",
            ACME_PRIVATE,
            "10.88.4.12/24",
            "198.51.100.7:51820",
            "acme.example\ncorp.example\n172.20.0.0/16\n",
        ),
        (
            "Contoso",
            CONTOSO_PRIVATE,
            "10.77.0.5/32",
            "198.51.100.8:51820",
            "contoso.example\nb.corp.example\n",
        ),
        (
            "HomeLab",
            HOME_PRIVATE,
            "10.66.0.2/32",
            "198.51.100.9:51820",
            "lab.home.example\n",
        ),
    ];
    for (n, (name, key, address, endpoint, list)) in profiles_to_write.iter().enumerate() {
        let pin = Pin::new(n as u8 + 1).unwrap();
        profiles
            .import(name, &conf(key, address, endpoint), pin, false)
            .unwrap();
        let stored = profiles.find(name).unwrap().name;
        fs::write(folder.list(&stored), list).unwrap();
        fs::write(
            folder.profile_settings(&stored),
            format!("connect_at_boot=false\nupstream={upstream}\n"),
        )
        .unwrap();
    }
}

/// Asks the proxy for `name`'s A record, as a client would.
fn ask(name: &str) -> Vec<Ipv4Addr> {
    let socket = UdpSocket::bind("127.0.0.1:0").unwrap();
    socket
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    socket
        .send_to(&DnsQuery::build(0x5157, name, TYPE_A), LISTEN)
        .unwrap();
    let mut buffer = [0u8; 4096];
    let len = socket.recv(&mut buffer).unwrap();
    let response = DnsResponse::parse(buffer[..len].to_vec()).unwrap();
    assert_eq!(response.id(), 0x5157);
    response.ipv4_chain(name)
}

fn host(ip: &str) -> Ipv4Net {
    Ipv4Net::host(ip.parse().unwrap())
}

#[test]
fn two_profiles_route_their_names_on_their_own_fake_adapters() {
    let root = std::env::temp_dir().join(format!("splitwire-engine-it-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    let _cleanup = ScratchFolder(root.clone());
    let folder = DataFolder::new(root);
    let clock: Arc<dyn Clock> = Arc::new(SystemClock);
    let log = Log::new(Log::DEFAULT_BYTES, clock.clone(), false);
    let upstream = FakeUpstream::start(&[
        ("www.acme.example", "10.1.0.5"),
        ("cdn.acme.example", "192.0.2.93"),
        ("www.contoso.example", "192.0.2.93"),
        ("x.b.corp.example", "10.2.0.7"),
        ("y.lab.home.example", "10.3.0.1"),
    ]);
    let table = Arc::new(DryRunRouteTable::new(log.clone()));
    let nrpt = Arc::new(RefusingNrpt {
        inner: DryRunNrpt::new(log.clone()),
        refused: DomainName::parse("lab.home.example").unwrap(),
    });
    let platform = PlatformSet {
        routes: table.clone(),
        nrpt: nrpt.clone(),
        ..PlatformSet::dry_run(&log, clock.clone())
    };
    prepare(&folder, &platform, clock.clone(), upstream.address);
    let config = EngineConfig {
        folder: folder.clone(),
        listen: Some(LISTEN.parse().unwrap()),
        connect: Vec::new(),
        restore_all: false,
    };
    let engine = Engine::start(config, platform, clock, log.clone()).unwrap();
    let coordinator = engine.coordinator().clone();
    let parts = coordinator.parts().clone();

    // Connect both.
    coordinator.connect("AcmeCorp").unwrap();
    coordinator.connect("contoso").unwrap();
    let luid = |name: &str| parts.active.get(name).unwrap().link.lock().unwrap().luid;
    let (acme, contoso) = (luid("AcmeCorp"), luid("Contoso"));
    assert_ne!(acme, contoso);
    assert!(
        table
            .routes(acme)
            .contains(&"172.20.0.0/16".parse().unwrap())
    );
    assert_eq!(
        nrpt.inner.rules(),
        [
            "acme.example",
            "b.corp.example",
            "contoso.example",
            "corp.example"
        ]
    );

    // Listed names route on the right fake adapter, before the answer arrives.
    assert_eq!(ask("www.acme.example"), [Ipv4Addr::new(10, 1, 0, 5)]);
    assert!(table.routes(acme).contains(&host("10.1.0.5")));
    assert_eq!(ask("x.b.corp.example"), [Ipv4Addr::new(10, 2, 0, 7)]);
    assert!(table.routes(contoso).contains(&host("10.2.0.7")));
    assert!(!table.routes(acme).contains(&host("10.2.0.7")));

    // The first learner holds a shared address; the other profile gets the alert.
    ask("cdn.acme.example");
    assert_eq!(ask("www.contoso.example"), [Ipv4Addr::new(192, 0, 2, 93)]);
    assert!(table.routes(acme).contains(&host("192.0.2.93")));
    assert!(!table.routes(contoso).contains(&host("192.0.2.93")));
    let held: Vec<String> = parts
        .alerts
        .active()
        .iter()
        .filter(|a| a.kind() == AlertKind::AddressHeld)
        .map(|a| a.id.clone())
        .collect();
    assert_eq!(held, ["address_held:Contoso:192.0.2.93"]);

    // Disconnect one: its names fall through to the other, its routes are gone.
    coordinator.disconnect("Contoso").unwrap();
    assert!(table.routes(contoso).is_empty());
    assert_eq!(ask("x.b.corp.example"), [Ipv4Addr::new(10, 2, 0, 7)]);
    assert!(table.routes(acme).contains(&host("10.2.0.7")));
    assert_eq!(nrpt.inner.rules(), ["acme.example", "corp.example"]);
    assert!(
        parts
            .alerts
            .active()
            .iter()
            .all(|a| a.kind() != AlertKind::AddressHeld)
    );

    // A failing Connect step rolls back and leaves the connected profile alone.
    let before: BTreeSet<Ipv4Net> = table.routes(acme).into_iter().collect();
    let error = coordinator.connect("HomeLab").unwrap_err();
    assert!(
        error.to_string().starts_with("step 9 (nrpt) failed"),
        "{error}"
    );
    let record = parts.states.get("HomeLab");
    assert_eq!(record.state, ProfileState::Failed);
    let attempt = record.last_attempt.unwrap();
    let results: Vec<StepResult> = attempt.steps.iter().map(|s| s.result).collect();
    use StepResult::*;
    assert_eq!(
        results,
        [
            Undone, Skipped, Undone, Undone, Undone, Undone, Undone, Undone, Failed, NotRun
        ]
    );
    assert_eq!(attempt.error.unwrap().0, ConnectStep::Nrpt);
    assert!(!parts.active.contains("HomeLab"));
    assert_eq!(
        table.routes(acme).into_iter().collect::<BTreeSet<_>>(),
        before
    );
    assert_eq!(nrpt.inner.rules(), ["acme.example", "corp.example"]);
    assert!(
        parts
            .alerts
            .active()
            .iter()
            .any(|a| a.id == "connect_failed:HomeLab:")
    );
    assert_eq!(ask("www.acme.example"), [Ipv4Addr::new(10, 1, 0, 5)]);

    // Disconnect all: no route, no rule, no proxy.
    coordinator.disconnect_all(false);
    assert!(parts.active.is_empty());
    assert!(table.is_empty());
    assert!(nrpt.inner.rules().is_empty());
    assert!(
        UdpSocket::bind(LISTEN).is_ok(),
        "the proxy port is free again"
    );
    assert_eq!(fs::read_to_string(folder.state()).unwrap(), "connected=\n");
    engine.stop();
    assert!(!folder.running_marker().exists());
    assert!(log.contains("dry-run: would add route 10.1.0.5/32"));
    // Redaction: no key, in base64 or in part, reached the log.
    for key in [ACME_PRIVATE, CONTOSO_PRIVATE, HOME_PRIVATE] {
        assert!(!log.contains(&key[..16]), "a key reached the log");
    }
}
