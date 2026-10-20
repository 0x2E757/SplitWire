//! The tray's view models per demo scenario: the state, the tooltip and its length,
//! the menu items and what each does.

use super::*;
use crate::ipc::{ProfileState, ProfileSummary};
use crate::presentation::samples::Samples;
use crate::presentation::showcase::Driver;
use crate::presentation::transport::demo::Scenario;
use crate::presentation::{Page, Session};

fn tray(scenario: Scenario) -> TrayView {
    Driver::new(scenario).session.tray()
}

fn entry(item: &TrayItem) -> &TrayEntry {
    match item {
        TrayItem::Entry(entry) | TrayItem::Submenu(entry, _) => entry,
        TrayItem::Separator => panic!("a separator"),
    }
}

fn labels(items: &[TrayItem]) -> Vec<String> {
    items
        .iter()
        .map(|item| match item {
            TrayItem::Separator => "-".to_owned(),
            other => entry(other).label.clone(),
        })
        .collect()
}

fn submenu<'a>(items: &'a [TrayItem], label: &str) -> &'a [TrayItem] {
    items
        .iter()
        .find_map(|item| match item {
            TrayItem::Submenu(entry, sub) if entry.label == label => Some(&sub[..]),
            _ => None,
        })
        .unwrap_or_else(|| panic!("no submenu {label}"))
}

/// The item of the profile `name`: a plain item, never a submenu.
fn profile<'a>(items: &'a [TrayItem], name: &str) -> &'a TrayEntry {
    items
        .iter()
        .find_map(|item| match item {
            TrayItem::Entry(entry) if entry.label == name => Some(entry),
            TrayItem::Submenu(entry, _) if entry.label == name => panic!("{name} is a submenu"),
            _ => None,
        })
        .unwrap_or_else(|| panic!("no item {name}"))
}

fn switch(name: &str, on: bool) -> Option<TrayAction> {
    Some(TrayAction::Session(Command::SetConnected(name.into(), on)))
}

#[test]
fn normal_shows_each_profile_as_one_item_and_the_alerts() {
    let view = tray(Scenario::Normal);
    assert_eq!(view.state, TrayState::Warning);
    assert_eq!(
        view.tooltip,
        "SplitWire · 2 of 3 connected\nHomeLab can’t connect\n4 alerts"
    );
    assert_eq!(
        labels(&view.menu),
        [
            "2 of 3 connected",
            "-",
            "Contoso",
            "AcmeCorp",
            "HomeLab",
            "-",
            "Alerts · 4",
            "-",
            "Open SplitWire",
            "Exit"
        ]
    );
    let head = entry(&view.menu[0]);
    assert_eq!(head.lead, TrayLead::State(TrayState::Warning));
    assert!(!head.enabled && head.action.is_none());
    let acme = profile(&view.menu, "AcmeCorp");
    assert_eq!(
        (acme.key.as_deref(), &acme.lead, acme.enabled),
        (Some("On"), &TrayLead::Dot(Dot::Ok), true)
    );
    assert_eq!(acme.action, switch("AcmeCorp", false));
    let home = profile(&view.menu, "HomeLab");
    assert_eq!(
        (home.key.as_deref(), &home.lead, home.enabled),
        (
            Some("Can’t connect · profile.conf is invalid"),
            &TrayLead::Dot(Dot::Error),
            false
        )
    );
    let alerts = submenu(&view.menu, "Alerts · 4");
    assert_eq!(alerts.len(), 6);
    let first = entry(&alerts[0]);
    assert_eq!(first.lead, TrayLead::Severity(Severity::Error));
    assert_eq!(first.key.as_deref(), Some("HomeLab"));
    assert_eq!(
        first.action,
        Some(TrayAction::Open(Some(Command::FollowAlert(
            "profile_settings_invalid:HomeLab:mtu".into()
        ))))
    );
    assert_eq!(entry(&alerts[1]).key.as_deref(), Some("Global"));
    assert_eq!(
        entry(&alerts[5]).action,
        Some(TrayAction::Session(Command::DismissAll))
    );
    let open = entry(&view.menu[8]);
    assert!(open.default);
    assert_eq!(open.action, Some(TrayAction::Open(None)));
    assert_eq!(entry(&view.menu[9]).action, Some(TrayAction::Exit));
}

#[test]
fn one_click_switches_a_profile_off_and_on() {
    let mut driver = Driver::new(Scenario::Normal);
    let acme = profile(&driver.session.tray().menu, "AcmeCorp").clone();
    let Some(TrayAction::Session(command)) = acme.action else {
        panic!("{acme:?}");
    };
    driver.run(command);
    let menu = driver.session.tray().menu;
    let acme = profile(&menu, "AcmeCorp");
    assert_eq!(
        (acme.key.as_deref(), &acme.lead, acme.enabled),
        (Some("Off"), &TrayLead::Dot(Dot::Off), true)
    );
    assert_eq!(acme.action, switch("AcmeCorp", true));
}

#[test]
fn a_failed_connect_is_tried_again_with_one_click() {
    let view = tray(Scenario::ConnectFail);
    assert_eq!(view.state, TrayState::Error);
    assert!(view.tooltip.contains("\nHomeLab connect failed\n"));
    let home = profile(&view.menu, "HomeLab");
    assert_eq!((home.key.as_deref(), home.enabled), (Some("Failed"), true));
    assert_eq!(home.action, switch("HomeLab", true));
}

#[test]
fn invalid_settings_hold_every_connect() {
    let view = tray(Scenario::SettingsInvalid);
    assert_eq!(view.state, TrayState::Error);
    assert_eq!(
        view.tooltip,
        "SplitWire · 0 of 3 connected\nHomeLab can’t connect\n2 alerts"
    );
    let contoso = profile(&view.menu, "Contoso");
    assert_eq!(
        (contoso.key.as_deref(), contoso.enabled),
        (Some("Off · splitwire.conf is invalid"), false)
    );
    // Both alerts are blocking: nothing to dismiss.
    let alerts = submenu(&view.menu, "Alerts · 2");
    assert_eq!(alerts.len(), 2);
}

#[test]
fn no_profile_item_is_a_submenu_in_any_scenario() {
    for scenario in Scenario::ALL {
        let view = tray(scenario);
        let submenus: Vec<&str> = view
            .menu
            .iter()
            .filter_map(|item| match item {
                TrayItem::Submenu(entry, _) => Some(entry.label.as_str()),
                _ => None,
            })
            .collect();
        assert!(
            submenus.iter().all(|label| label.starts_with("Alerts · ")),
            "{}: {submenus:?}",
            scenario.key()
        );
    }
}

#[test]
fn the_first_run_offers_add_profile() {
    let view = tray(Scenario::FirstRun);
    assert_eq!(view.state, TrayState::Off);
    assert_eq!(view.tooltip, "SplitWire · no profile yet");
    assert_eq!(
        labels(&view.menu),
        [
            "No profile yet",
            "-",
            "Add profile…",
            "-",
            "No alerts",
            "-",
            "Open SplitWire",
            "Exit"
        ]
    );
    assert_eq!(
        entry(&view.menu[2]).action,
        Some(TrayAction::Open(Some(Command::Go(Page::Add))))
    );
    assert!(!entry(&view.menu[4]).enabled);
}

#[test]
fn without_the_service_only_open_and_exit() {
    let view = tray(Scenario::NoService);
    assert_eq!(view.state, TrayState::Service);
    assert_eq!(view.tooltip, "SplitWire · service not running");
    assert_eq!(
        labels(&view.menu),
        ["Service not running", "-", "Open SplitWire", "Exit"]
    );
    assert_eq!(
        entry(&view.menu[0]).lead,
        TrayLead::State(TrayState::Service)
    );
}

#[test]
fn the_tooltip_always_fits_sz_tip() {
    for scenario in Scenario::ALL {
        assert!(tray(scenario).tooltip.encode_utf16().count() <= TrayTooltip::MAX);
    }
    // Sixteen failed profiles with the longest names, and alerts.
    let mut status = Samples::status("normal");
    let template = status.profiles[1].clone();
    status.profiles = (0..16)
        .map(|n| ProfileSummary {
            name: format!("{n:02}{}", "W".repeat(30)),
            state: ProfileState::Failed,
            ..template.clone()
        })
        .collect();
    let tooltip = TrayTooltip::of(Some(&status));
    assert!(
        tooltip.encode_utf16().count() <= TrayTooltip::MAX,
        "{tooltip}"
    );
    assert!(tooltip.starts_with("SplitWire · 0 of 16 connected\n00WWW"));
    assert!(tooltip.ends_with(" more\n4 alerts"), "{tooltip}");
    // A connected profile has no line of its own.
    for profile in &mut status.profiles[1..] {
        profile.state = template.state;
    }
    status.profiles[0].state = ProfileState::Blocked;
    assert_eq!(
        TrayTooltip::of(Some(&status)),
        format!(
            "SplitWire · 15 of 16 connected\n00{} can’t connect\n4 alerts",
            "W".repeat(30)
        )
    );
}

#[test]
fn the_tray_actions_go_through_the_session() {
    let mut driver = Driver::new(Scenario::Normal);
    driver.run(Command::ResetCache("AcmeCorp".into()));
    assert!(
        driver
            .session
            .view()
            .toast
            .unwrap()
            .starts_with("DNS cache of AcmeCorp reset")
    );
    driver.session.command(Command::Refresh, driver.now);
    driver.session.tick(driver.now);
    assert!(
        driver
            .session
            .take_requests()
            .iter()
            .any(|(_, r)| *r == crate::ipc::Request::Status)
    );
}

#[test]
fn before_the_first_answer_the_tray_waits_for_the_service() {
    let session = Session::new(
        Box::new(crate::clock::ManualClock::new(Scenario::now())),
        0,
        crate::presentation::Folders {
            data: String::new(),
            logs: Default::default(),
        },
        Default::default(),
        false,
    );
    assert_eq!(session.tray().state, TrayState::Service);
}
