//! The session against the demo service, as the window drives it: commands, ticks, answers.

use std::sync::Arc;
use std::time::Duration;

use super::*;
use crate::config::SettingsScope;
use crate::ipc::{ConfText, ProfileState};
use crate::presentation::showcase::Driver;
use crate::presentation::transport::demo::{DemoTransport, Scenario};
use crate::presentation::{Command, Effect, InputId, PickedFile, SettingsEdit};
use crate::profile::fixtures;

#[test]
fn the_first_answer_fills_status_and_the_nav() {
    let driver = Driver::new(Scenario::Normal);
    assert_eq!(driver.session.service(), ServiceState::Running);
    let view = driver.session.view();
    assert!(matches!(view.content, PageView::Status(_)));
    assert_eq!(view.badge, Some((4, crate::ipc::Severity::Error)));
    assert_eq!(view.menus.len(), 3);
    assert_eq!(
        view.title,
        format!("SplitWire v{}", env!("CARGO_PKG_VERSION"))
    );
    let PageView::Status(status) = view.content else {
        unreachable!()
    };
    assert_eq!(status.big.len(), 2);
    assert!(status.big[0].chart.series.is_some(), "traffic arrived");
}

#[test]
fn without_the_service_status_says_so_and_pages_stay_closed() {
    let mut driver = Driver::new(Scenario::NoService);
    assert_eq!(driver.session.service(), ServiceState::Down);
    driver.run(Command::Go(Page::Settings));
    assert_eq!(*driver.session.page(), Page::Status);
    assert!(matches!(
        driver.session.view().content,
        PageView::ServiceDown(_)
    ));
    driver.later(Duration::from_secs(1));
    assert!(
        !driver.session.take_requests().is_empty()
            || driver.session.service() == ServiceState::Down
    );
}

#[test]
fn the_first_run_shows_the_welcome_drop_zone() {
    let driver = Driver::new(Scenario::FirstRun);
    let PageView::Add(add) = driver.session.view().content else {
        panic!("the welcome page");
    };
    assert!(add.first_run);
}

#[test]
fn every_page_builds_in_every_scenario() {
    for scenario in Scenario::ALL {
        let mut driver = Driver::new(scenario);
        let names: Vec<String> = driver
            .session
            .data
            .status
            .as_ref()
            .map(|s| s.profiles.iter().map(|p| p.name.clone()).collect())
            .unwrap_or_default();
        let mut pages = vec![
            Page::Status,
            Page::Routes,
            Page::Lists,
            Page::Logs,
            Page::Settings,
            Page::Add,
        ];
        pages.extend(names.into_iter().map(Page::Profile));
        for page in pages {
            driver.run(Command::Go(page.clone()));
            let view = driver.session.view();
            assert!(
                !matches!(view.content, PageView::Loading(_)),
                "{scenario:?} {page:?} stays loading"
            );
        }
    }
}

#[test]
fn connecting_and_disconnecting_go_through_the_service() {
    let mut driver = Driver::new(Scenario::Normal);
    driver.run(Command::SetConnected("AcmeCorp".into(), false));
    let status = driver.session.data.status.clone().unwrap();
    assert_eq!(status.profiles[1].state, ProfileState::Off);
    driver.run(Command::SetConnected("AcmeCorp".into(), true));
    let status = driver.session.data.status.clone().unwrap();
    assert!(matches!(
        status.profiles[1].state,
        ProfileState::Connecting(_)
    ));
}

#[test]
fn adding_an_entry_clears_the_field_and_a_duplicate_is_refused_before_sending() {
    let mut driver = Driver::new(Scenario::Normal);
    driver.run(Command::Go(Page::Profile("AcmeCorp".into())));
    driver.run(Command::Input(
        InputId::ProfileAdd,
        "contoso.example".into(),
    ));
    driver.run(Command::Submit(InputId::ProfileAdd));
    assert!(
        driver
            .session
            .view()
            .toast
            .unwrap()
            .contains("also in Contoso line 2")
    );
    driver.run(Command::Input(
        InputId::ProfileAdd,
        "wiki.acme.example".into(),
    ));
    driver.run(Command::AddEntry("AcmeCorp".into()));
    assert_eq!(
        driver.session.view().toast.as_deref(),
        Some("Added to AcmeCorp as line 5")
    );
    assert!(
        driver
            .session
            .take_resets()
            .contains(&(InputId::ProfileAdd, String::new()))
    );
    let PageView::Profile(view) = driver.session.view().content else {
        panic!("profile page")
    };
    assert!(view.groups.iter().any(|g| g.title == "wiki.acme.example"));
    driver.run(Command::RemoveLine {
        profile: "AcmeCorp".into(),
        line: 5,
        text: "wiki.acme.example".into(),
    });
    assert_eq!(
        driver.session.view().toast.as_deref(),
        Some("Line 5 removed from AcmeCorp")
    );
}

#[test]
fn a_refused_combined_save_marks_its_lines() {
    let mut driver = Driver::new(Scenario::Normal);
    driver.run(Command::Go(Page::Lists));
    driver.run(Command::Input(
        InputId::ListsText,
        "acme.example\n[profile AcmeCorp]\n".into(),
    ));
    driver.run(Command::SaveAllLists);
    let PageView::Lists(view) = driver.session.view().content else {
        panic!("lists")
    };
    assert!(view.text.refused.is_some());
    assert!(view.text.bad_lines.contains(&1));
    driver.run(Command::RevertAllLists);
    let PageView::Lists(view) = driver.session.view().content else {
        panic!("lists")
    };
    assert!(view.text.refused.is_none() && !view.text.dirty);
}

#[test]
fn a_one_click_fix_fills_the_field_and_apply_writes_it() {
    let mut driver = Driver::new(Scenario::SettingsInvalid);
    let id = driver.session.data.status.as_ref().unwrap().alerts[0]
        .id
        .clone();
    driver.run(Command::FollowAlert(id));
    assert_eq!(*driver.session.page(), Page::Settings);
    let field = SettingsEdit::field(SettingsScope::Global, "max_ttl");
    assert!(
        driver
            .session
            .take_resets()
            .contains(&(InputId::Setting(field.clone()), "300".into()))
    );
    let view = driver.session.view();
    assert!(view.apply_bar.as_ref().unwrap().can_apply);
    driver.run(Command::SettingsApply);
    assert_eq!(
        driver.session.view().toast.as_deref(),
        Some("Saved 1 change")
    );
    let status = driver.session.data.status.as_ref().unwrap();
    assert!(status.problems.is_empty());
    assert_eq!(status.profiles[0].state, ProfileState::Off);
    assert!(driver.session.view().apply_bar.is_none());
}

#[test]
fn an_import_previews_each_file_and_lands_on_the_new_profile() {
    let mut driver = Driver::new(Scenario::Normal);
    driver.run(Command::PickFiles { replace: None });
    assert!(driver.session.take_effects().contains(&Effect::PickFiles));
    let conf = fixtures::conf(
        fixtures::PRIVATE,
        "10.77.0.5/32",
        "10.77.0.53",
        "vpn.partner.example:51820",
    );
    driver.run(Command::FilesRead(vec![PickedFile {
        file_name: "Partner.conf".into(),
        text: Ok(Arc::new(ConfText::new(conf))),
    }]));
    assert_eq!(*driver.session.page(), Page::Review);
    let PageView::Review(review) = driver.session.view().content else {
        panic!("review")
    };
    assert!(review.can_import, "{:?}", review.files[0].checks);
    driver.run(Command::Input(InputId::ReviewName(0), "AcmeCorp".into()));
    let PageView::Review(review) = driver.session.view().content else {
        panic!("review")
    };
    assert_eq!(review.files[0].replace.as_deref(), Some("AcmeCorp"));
    driver.run(Command::Input(InputId::ReviewName(0), "Partner".into()));
    driver.run(Command::Import);
    assert_eq!(*driver.session.page(), Page::Profile("Partner".into()));
    assert_eq!(
        driver.session.data.status.as_ref().unwrap().profiles.len(),
        4
    );
}

#[test]
fn remove_asks_first_then_returns_to_status() {
    let mut driver = Driver::new(Scenario::Normal);
    driver.run(Command::Go(Page::Profile("HomeLab".into())));
    driver.run(Command::AskRemove("HomeLab".into()));
    assert_eq!(
        driver.session.view().dialog,
        Some(DialogView::Remove("HomeLab".into()))
    );
    driver.run(Command::Remove("HomeLab".into()));
    assert_eq!(*driver.session.page(), Page::Status);
    assert_eq!(
        driver.session.view().toast.as_deref(),
        Some("HomeLab removed")
    );
}

#[test]
fn the_logs_follow_and_pause() {
    let mut driver = Driver::new(Scenario::Normal);
    driver.run(Command::OpenLog("HomeLab".into()));
    let PageView::Logs(view) = driver.session.view().content else {
        panic!("logs")
    };
    assert_eq!(view.profile_label, "HomeLab");
    driver.run(Command::LogFollow);
    let PageView::Logs(view) = driver.session.view().content else {
        panic!("logs")
    };
    assert!(!view.follow);
    driver.run(Command::LogSource(crate::presentation::LogSource::File));
    assert!(driver.session.take_effects().contains(&Effect::ReadLogFile));
    driver.run(Command::LogFileRead(DemoTransport::log_file(
        Scenario::Normal,
    )));
    let PageView::Logs(view) = driver.session.view().content else {
        panic!("logs")
    };
    assert!(!view.days.is_empty());
}

#[test]
fn a_refused_connect_moves_the_switch_revision() {
    let mut driver = Driver::new(Scenario::Normal);
    assert_eq!(driver.session.view().switches, 0);
    driver.run(Command::SetConnected("HomeLab".into(), true));
    assert_eq!(driver.session.view().switches, 1);
    assert!(
        driver
            .session
            .view()
            .toast
            .unwrap()
            .contains("profile.conf is invalid")
    );
}
