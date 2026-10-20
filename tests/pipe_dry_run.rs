//! The pipe in the user session: a dry-run engine host serving
//! `SplitWire.Test.<pid>` and a client driving it the way the window and the CLI do: import with a
//! preview, settings and lists saved, a connect whose listed name routes, the status and route
//! reports, the alerts, a disconnect and a remove. Nothing on the machine changes, and no answer
//! carries a key.

use std::collections::HashMap;
use std::fs;
use std::net::{Ipv4Addr, SocketAddr, UdpSocket};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use splitwire::application::{DataFolder, EngineConfig, EngineHost, HostConfig, StopSignal};
use splitwire::config::{SettingChange, SettingsScope};
use splitwire::dns::ResponseBuilder;
use splitwire::dns::wire::{DnsQuery, DnsResponse, TYPE_A};
use splitwire::ipc::{
    ConfText, ErrorCode, ProfileState, ProfileTarget, Request, Response, RouteGroupKey, RouteRow,
};
use splitwire::platform::pipe::{PipeClient, PipeError, PipeName};
use splitwire::profile::Pin;

// Made-up keys; none of them is a real key.
const ACME_PRIVATE: &str = "yAnz5TF+lXXJte14tji3zlMNq+hd2rYUIgJBgB3fBmk=";
const PARTNER_PRIVATE: &str = "cGFydG5lci1wcml2YXRlLWtleS1tYWRlLXVwLTAwMDA=";
const PEER_PUBLIC: &str = "xTIBA5rboUvnH4htodjb6e697QjLERt1NAB4mZqp8Dg=";
const LISTEN: &str = "127.0.0.1:15354";

fn conf(private_key: &str, address: &str, endpoint: &str) -> String {
    format!(
        "[Interface]\nPrivateKey = {private_key}\nAddress = {address}\nDNS = 10.66.0.53\n\
         [Peer]\nPublicKey = {PEER_PUBLIC}\nEndpoint = {endpoint}\n"
    )
}

/// A DNS server on loopback answering one name: the profiles' upstream.
struct FakeUpstream {
    address: SocketAddr,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl FakeUpstream {
    fn start(zone: &[(&str, Ipv4Addr)]) -> Self {
        let zone: HashMap<String, Ipv4Addr> =
            zone.iter().map(|(n, ip)| ((*n).to_owned(), *ip)).collect();
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

/// The client: every answer is checked for the keys before it is returned.
struct Client(PipeClient);

impl Client {
    fn ask(&mut self, request: Request) -> Response {
        let response = self.0.exchange(&request).unwrap();
        let encoded = response.encode().unwrap();
        for key in [ACME_PRIVATE, PARTNER_PRIVATE] {
            let text = format!("{response:?}");
            assert!(!text.contains(&key[..16]), "a key in {text}");
            let raw = splitwire::profile::Base64::decode_key(key).unwrap();
            assert!(
                !encoded.windows(raw.len()).any(|w| w == raw),
                "raw key bytes in an answer"
            );
        }
        response
    }

    fn done(&mut self, request: Request) {
        match self.ask(request) {
            Response::Done => {}
            other => panic!("{other:?}"),
        }
    }

    fn status(&mut self) -> Box<splitwire::ipc::StatusReport> {
        match self.ask(Request::Status) {
            Response::Status(status) => status,
            other => panic!("{other:?}"),
        }
    }
}

fn ask_dns(name: &str) -> Vec<Ipv4Addr> {
    let socket = UdpSocket::bind("127.0.0.1:0").unwrap();
    socket
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    socket
        .send_to(&DnsQuery::build(0x5157, name, TYPE_A), LISTEN)
        .unwrap();
    let mut buffer = [0u8; 4096];
    let len = socket.recv(&mut buffer).unwrap();
    DnsResponse::parse(buffer[..len].to_vec())
        .unwrap()
        .ipv4_chain(name)
}

#[test]
fn a_client_drives_a_dry_run_engine_over_the_pipe() {
    let root = std::env::temp_dir().join(format!("splitwire-pipe-it-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    let pipe = PipeName::parse(&format!("SplitWire.Test.{}", std::process::id())).unwrap();
    let upstream = FakeUpstream::start(&[("www.acme.example", Ipv4Addr::new(198, 18, 1, 5))]);
    let stop = StopSignal::default();
    let host = {
        let (stop, pipe, folder) = (stop.clone(), pipe.clone(), DataFolder::new(root.clone()));
        thread::spawn(move || {
            let config = HostConfig {
                engine: EngineConfig {
                    folder,
                    listen: Some(LISTEN.parse().unwrap()),
                    connect: Vec::new(),
                    restore_all: false,
                },
                dry_run: true,
                limit: Some(Duration::from_secs(120)),
                pipe,
                echo: false,
            };
            EngineHost::run(config, &stop, |_| {})
        })
    };
    let mut client = Client(PipeClient::new(pipe.clone()));
    let deadline = Instant::now() + Duration::from_secs(20);
    let status = loop {
        match client.0.exchange(&Request::Status) {
            Ok(Response::Status(status)) => break status,
            Err(PipeError::NotRunning) if Instant::now() < deadline => {
                thread::sleep(Duration::from_millis(50));
            }
            other => panic!("{other:?}"),
        }
    };
    assert!(status.profiles.is_empty());
    assert_eq!(status.wgnt_version, None);

    // Import: a preview stores nothing, then the import with the proposed colour.
    let preview =
        |client: &mut Client, name: &str, text: String| match client.ask(Request::PreviewImport {
            name: name.into(),
            file_name: format!("{name}.conf"),
            conf_text: ConfText::new(text),
        }) {
            Response::ImportPreview(preview) => preview,
            other => panic!("{other:?}"),
        };
    let acme_conf = conf(ACME_PRIVATE, "10.88.4.12/24", "198.51.100.7:51820");
    let first = preview(&mut client, "AcmeCorp", acme_conf.clone());
    assert!(!first.blocks(), "{:?}", first.problems());
    assert!(client.status().profiles.is_empty());
    let imported = client.ask(Request::Import {
        name: "AcmeCorp".into(),
        conf_text: ConfText::new(acme_conf),
        colour: first.colour,
        replace: false,
    });
    assert!(matches!(&imported, Response::Imported(s) if s.name == "AcmeCorp"));
    let partner_conf = conf(PARTNER_PRIVATE, "10.88.4.13/24", "198.51.100.8:51820");
    let overlap = preview(&mut client, "Partner", partner_conf);
    assert!(overlap.blocks(), "the tunnels overlap");
    let partner_conf = conf(PARTNER_PRIVATE, "10.77.0.5/32", "198.51.100.8:51820");
    let second = preview(&mut client, "Partner", partner_conf.clone());
    assert_ne!(second.colour, first.colour);
    client.ask(Request::Import {
        name: "Partner".into(),
        conf_text: ConfText::new(partner_conf),
        colour: Pin::new(5).unwrap(),
        replace: false,
    });

    // Settings: a stale revision is refused, then the upstream goes to the fake DNS.
    let settings = match client.ask(Request::GetSettings) {
        Response::Settings(settings) => settings,
        other => panic!("{other:?}"),
    };
    assert_eq!(settings.scopes.len(), 3);
    let upstream_change = vec![SettingChange {
        scope: SettingsScope::Profile("AcmeCorp".into()),
        key: "upstream".into(),
        value: Some(upstream.address.to_string()),
    }];
    let stale = client.ask(Request::SaveSettings {
        base_rev: settings.rev ^ 1,
        changes: upstream_change.clone(),
    });
    assert!(matches!(stale, Response::Error(e) if e.code == ErrorCode::StaleRev));
    let saved = client.ask(Request::SaveSettings {
        base_rev: settings.rev,
        changes: upstream_change,
    });
    assert!(matches!(saved, Response::SettingsSaved(s) if s.reconnects.is_empty()));

    // Lists: an entry added, then a duplicate across profiles refused with both numbers.
    let lists = match client.ask(Request::GetAllLists) {
        Response::Lists(lists) => lists,
        other => panic!("{other:?}"),
    };
    let added = match client.ask(Request::EditList {
        base_rev: lists.rev,
        name: "AcmeCorp".into(),
        add: Some("acme.example".into()),
        remove: None,
    }) {
        Response::Lists(lists) => lists,
        other => panic!("{other:?}"),
    };
    assert_eq!(
        added.text,
        "[profile AcmeCorp]\nacme.example\n\n[profile Partner]\n"
    );
    let duplicate = client.ask(Request::SaveAllLists {
        base_rev: added.rev,
        text: "[profile AcmeCorp]\nacme.example\n\n[profile Partner]\nacme.example\n".into(),
    });
    match duplicate {
        Response::Error(reply) => match reply.code {
            ErrorCode::Refused(problems) => assert!(!problems.is_empty()),
            other => panic!("{other:?}"),
        },
        other => panic!("{other:?}"),
    }

    // Connect: the listed name routes on the fake adapter; status and routes show it.
    client.done(Request::Connect {
        name: "acmecorp".into(),
    });
    assert_eq!(ask_dns("www.acme.example"), [Ipv4Addr::new(198, 18, 1, 5)]);
    let status = client.status();
    let acme = &status.profiles[0];
    assert!(matches!(acme.state, ProfileState::Connected(_)), "{acme:?}");
    assert_eq!(acme.learned, 1);
    assert_eq!(status.nrpt.rules, 1);
    assert_eq!(status.counters.queries.listed, 1);
    let routes = match client.ask(Request::Routes) {
        Response::Routes(routes) => routes,
        other => panic!("{other:?}"),
    };
    let line = &routes[0].groups[0];
    assert_eq!(
        line.key,
        RouteGroupKey::Line {
            line: 1,
            text: "acme.example".into()
        }
    );
    assert!(
        matches!(&line.rows[..], [RouteRow::Learned { name, .. }] if name == "www.acme.example")
    );
    let detail = match client.ask(Request::ProfileStatus {
        name: "AcmeCorp".into(),
    }) {
        Response::ProfileStatus(detail) => detail,
        other => panic!("{other:?}"),
    };
    assert_eq!(detail.adapter, "SplitWire-AcmeCorp");
    assert_eq!(detail.peer_public_key, PEER_PUBLIC);
    assert_eq!(detail.list[0].routes, 1);
    assert!(detail.last_attempt.is_some());
    assert!(matches!(
        client.ask(Request::ProfileStatus { name: "Ghost".into() }),
        Response::Error(e) if e.code == ErrorCode::UnknownProfile
    ));
    assert!(matches!(
        client.ask(Request::Traffic { after_seq: 0 }),
        Response::Traffic(_)
    ));
    let logs = match client.ask(Request::Logs { after: 0 }) {
        Response::Logs(page) => page,
        other => panic!("{other:?}"),
    };
    assert!(
        logs.entries
            .iter()
            .any(|l| l.message.contains("dry-run: would add route 198.18.1.5/32"))
    );
    assert!(logs.last_seq >= logs.entries.len() as u64);

    // Alerts: an unknown id is a bad request; the history of today answers.
    assert!(matches!(
        client.ask(Request::DismissAlert { id: "nope".into() }),
        Response::Error(e) if e.code == ErrorCode::BadRequest
    ));
    assert!(matches!(
        client.ask(Request::AlertHistory),
        Response::AlertHistory(_)
    ));
    client.done(Request::DismissAll);
    client.done(Request::RestoreAlerts { id: None });
    assert!(matches!(
        client.ask(Request::ResetCache { name: None }),
        Response::CacheReset { routes: 1, .. }
    ));

    // Colour, order, disconnect, remove.
    client.done(Request::SetProfileColor {
        name: "Partner".into(),
        colour: Pin::new(6).unwrap(),
    });
    client.done(Request::MoveProfile {
        name: "Partner".into(),
        index: 0,
    });
    client.done(Request::Disconnect {
        target: ProfileTarget::All,
    });
    client.done(Request::RemoveProfile {
        name: "Partner".into(),
    });
    let status = client.status();
    let names: Vec<&str> = status.profiles.iter().map(|p| p.name.as_str()).collect();
    assert_eq!(names, ["AcmeCorp"]);
    assert!(matches!(status.profiles[0].state, ProfileState::Off));

    // A frame that is not a request gets bad_request and the connection stays usable.
    let garbage = splitwire::ipc::Writer::frame(99).finish().unwrap();
    use std::io::Write;
    let mut raw = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(pipe.path())
        .unwrap();
    raw.write_all(&garbage).unwrap();
    let body = splitwire::ipc::Frame::read(&mut raw).unwrap();
    assert!(matches!(
        Response::decode(&body).unwrap(),
        Response::Error(e) if e.code == ErrorCode::BadRequest
    ));
    drop(raw);

    stop.stop("the test is done");
    host.join().unwrap().unwrap();
    assert_eq!(
        PipeClient::new(pipe).exchange(&Request::Status),
        Err(PipeError::NotRunning)
    );
    let log = fs::read_to_string(root.join("logs").join("splitwire.log")).unwrap_or_default();
    for key in [ACME_PRIVATE, PARTNER_PRIVATE] {
        assert!(!log.contains(&key[..16]));
    }
    let _ = fs::remove_dir_all(&root);
}
