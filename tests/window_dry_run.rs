//! The window against a real engine without showing anything: a dry-run
//! engine host serves `SplitWire.Test.<pid>.Window`, two made-up profiles are imported over the
//! pipe, one is connected and the other blocked by a bad line in its `profile.conf`. The window's
//! `Session` then runs over a real `PipeClient`, as `SplitWire.Window.exe --pipe` does, and its
//! navigation, Status page, alerts, profile pages and tray show what the engine says.

use std::fs;
use std::thread;
use std::time::{Duration, Instant};

use splitwire::application::{DataFolder, EngineConfig, EngineHost, HostConfig, StopSignal};
use splitwire::clock::SystemClock;
use splitwire::ipc::{
    ConfText, ProfileState, Request, Response, Severity, StatusReport, TrayState,
};
use splitwire::platform::pipe::{PipeClient, PipeError, PipeName};
use splitwire::presentation::session::ServiceState;
use splitwire::presentation::showcase::Driver;
use splitwire::presentation::tray::{TrayItem, TrayLead};
use splitwire::presentation::{Command, Dot, NavValue, Page, PageView};

// Made-up keys; none of them is a real key.
const ACME_PRIVATE: &str = "yAnz5TF+lXXJte14tji3zlMNq+hd2rYUIgJBgB3fBmk=";
const PARTNER_PRIVATE: &str = "cGFydG5lci1wcml2YXRlLWtleS1tYWRlLXVwLTAwMDA=";
const PEER_PUBLIC: &str = "xTIBA5rboUvnH4htodjb6e697QjLERt1NAB4mZqp8Dg=";
const LISTEN: &str = "127.0.0.1:15355";

fn conf(private_key: &str, address: &str, endpoint: &str) -> String {
    format!(
        "[Interface]\nPrivateKey = {private_key}\nAddress = {address}\nDNS = 10.66.0.53\n\
         [Peer]\nPublicKey = {PEER_PUBLIC}\nEndpoint = {endpoint}\n"
    )
}

fn status(client: &mut PipeClient) -> Box<StatusReport> {
    match client.exchange(&Request::Status) {
        Ok(Response::Status(status)) => status,
        other => panic!("{other:?}"),
    }
}

/// Asks until `done` holds for the status, for at most 15 s.
fn wait_for(client: &mut PipeClient, done: impl Fn(&StatusReport) -> bool) -> Box<StatusReport> {
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        let status = status(client);
        if done(&status) {
            return status;
        }
        assert!(Instant::now() < deadline, "never came: {status:?}");
        thread::sleep(Duration::from_millis(100));
    }
}

fn import(client: &mut PipeClient, name: &str, text: String) {
    let preview = match client.exchange(&Request::PreviewImport {
        name: name.into(),
        file_name: format!("{name}.conf"),
        conf_text: ConfText::new(text.clone()),
    }) {
        Ok(Response::ImportPreview(preview)) => preview,
        other => panic!("{other:?}"),
    };
    assert!(!preview.blocks(), "{:?}", preview.problems());
    let imported = client.exchange(&Request::Import {
        name: name.into(),
        conf_text: ConfText::new(text),
        colour: preview.colour,
        replace: false,
    });
    assert!(
        matches!(imported, Ok(Response::Imported(_))),
        "{imported:?}"
    );
}

#[test]
fn the_window_shows_a_dry_run_engine_over_the_pipe() {
    let root = std::env::temp_dir().join(format!("splitwire-window-it-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    let pipe = PipeName::parse(&format!("SplitWire.Test.{}.Window", std::process::id())).unwrap();
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
    let mut setup = PipeClient::new(pipe.clone());
    let deadline = Instant::now() + Duration::from_secs(20);
    while let Err(PipeError::NotRunning) = setup.exchange(&Request::Status) {
        assert!(
            Instant::now() < deadline,
            "the dry run never served the pipe"
        );
        thread::sleep(Duration::from_millis(50));
    }

    // Two profiles: AcmeCorp connected with a list line, Partner blocked by its profile.conf.
    import(
        &mut setup,
        "AcmeCorp",
        conf(ACME_PRIVATE, "10.88.4.12/24", "198.51.100.7:51820"),
    );
    import(
        &mut setup,
        "Partner",
        conf(PARTNER_PRIVATE, "10.77.0.5/32", "198.51.100.8:51820"),
    );
    let lists = match setup.exchange(&Request::GetAllLists) {
        Ok(Response::Lists(lists)) => lists,
        other => panic!("{other:?}"),
    };
    let edited = setup.exchange(&Request::EditList {
        base_rev: lists.rev,
        name: "AcmeCorp".into(),
        add: Some("acme.example".into()),
        remove: None,
    });
    assert!(matches!(edited, Ok(Response::Lists(_))), "{edited:?}");
    let connected = setup.exchange(&Request::Connect {
        name: "AcmeCorp".into(),
    });
    assert_eq!(connected, Ok(Response::Done));
    // The engine's file watcher compares with what it saw a second earlier: the hand edit comes
    // after its first look.
    thread::sleep(Duration::from_millis(1500));
    fs::write(
        root.join("profiles").join("Partner").join("profile.conf"),
        "# made up\nmtu = 9000\n",
    )
    .unwrap();
    wait_for(&mut setup, |s| {
        s.profiles
            .iter()
            .any(|p| p.name == "Partner" && p.state == ProfileState::Blocked)
    });

    // The window's session over its own pipe client, answered at once.
    let mut driver = Driver::over(
        Box::new(PipeClient::new(pipe.clone())),
        Box::new(SystemClock),
        0,
    );
    assert_eq!(driver.session.service(), ServiceState::Running);
    let view = driver.session.view();

    // The navigation: both profiles in order with their states, the counts beside the pages.
    assert_eq!(view.nav.status.value, NavValue::Text("1 of 2".into()));
    let names: Vec<&str> = view.nav.profiles.iter().map(|p| p.label.as_str()).collect();
    assert_eq!(names, ["AcmeCorp", "Partner"]);
    assert_eq!(view.nav.profiles[0].tooltip, "AcmeCorp · Connected");
    assert_eq!(
        view.nav.profiles[0].value,
        NavValue::Dot(Dot::Ok, "On".into())
    );
    assert!(matches!(
        &view.nav.profiles[1].value,
        NavValue::Icon("error", _, label) if label == "Can’t connect"
    ));
    assert_eq!(view.nav.add.value, NavValue::Text("2 of 16".into()));
    assert_eq!(view.nav.lists.value, NavValue::Text("1".into()));
    assert!(view.nav.routes.enabled && view.nav.settings.enabled);

    // The bell and the Status page: a card for the connected profile, a slim line with its
    // blocking alert for the other.
    assert_eq!(view.badge, Some((1, Severity::Error)));
    assert_eq!(view.menus.len(), 2);
    let PageView::Status(page) = view.content else {
        panic!("the Status page: {:?}", view.content);
    };
    assert_eq!(page.subtitle, "1 of 2 profiles connected");
    assert_eq!(page.big.len(), 1);
    assert_eq!(page.big[0].name, "AcmeCorp");
    assert_eq!(page.big[0].pill.dot, Dot::Ok);
    assert!(page.big[0].switch.on);
    assert_eq!(page.slim.len(), 1);
    let partner = &page.slim[0];
    assert_eq!(
        (partner.name.as_str(), partner.pill.dot),
        ("Partner", Dot::Error)
    );
    assert!(!partner.switch.enabled);
    assert_eq!(partner.alerts.len(), 1);
    assert_eq!(partner.alerts[0].title, "Partner cannot connect");
    assert!(partner.alerts[0].message.contains("mtu = 9000"));
    assert!(!partner.alerts[0].dismissible, "a blocking alert stays");

    // The profile pages: the connected one with its adapter and list line, the blocked one with
    // its bad line and the fix waiting for Apply.
    driver.run(Command::Go(Page::Profile("AcmeCorp".into())));
    let PageView::Profile(acme) = driver.session.view().content else {
        panic!("AcmeCorp's page");
    };
    assert_eq!(
        acme.subtitle,
        "198.51.100.7:51820 · adapter SplitWire-AcmeCorp"
    );
    assert!(acme.switch.on && acme.blocked.is_none());
    assert_eq!(acme.groups[0].title, "acme.example");
    assert!(acme.groups.iter().any(|g| g.title == "Upstream DNS"));
    driver.run(Command::Go(Page::Profile("Partner".into())));
    let PageView::Profile(partner) = driver.session.view().content else {
        panic!("Partner's page");
    };
    assert_eq!(partner.pill.label, "Can’t connect");
    assert!(partner.blocked.is_some());
    assert!(
        partner
            .apply_bar
            .as_ref()
            .is_some_and(|bar| bar.text.contains("mtu = 9000"))
    );
    assert_eq!(partner.alerts.len(), 1);

    // The tray from the same session.
    let tray = driver.session.tray();
    assert_eq!(tray.state, TrayState::Warning);
    assert_eq!(
        tray.tooltip,
        "SplitWire · 1 of 2 connected\nPartner can’t connect\n1 alert"
    );
    let items: Vec<(&str, &TrayLead, bool)> = tray
        .menu
        .iter()
        .filter_map(|item| match item {
            TrayItem::Entry(entry) if matches!(entry.lead, TrayLead::Dot(_)) => {
                Some((entry.label.as_str(), &entry.lead, entry.enabled))
            }
            _ => None,
        })
        .collect();
    assert_eq!(
        items,
        [
            ("AcmeCorp", &TrayLead::Dot(Dot::Ok), true),
            ("Partner", &TrayLead::Dot(Dot::Error), false)
        ]
    );

    // A tray item reaches the engine, and every surface follows.
    driver.run(Command::SetConnected("AcmeCorp".into(), false));
    driver.later(Duration::from_secs(1));
    assert_eq!(
        driver.session.view().nav.status.value,
        NavValue::Text("0 of 2".into())
    );
    assert_eq!(driver.session.tray().state, TrayState::Warning);

    stop.stop("the test is done");
    host.join().unwrap().unwrap();
    let _ = fs::remove_dir_all(&root);
}
