//! Sample engine data for the view-model tests: the demo transport's scenarios.

use std::time::{Duration, SystemTime};

use super::LocalTime;
use super::transport::EngineTransport;
use super::transport::demo::{DemoTransport, Scenario};
use crate::ipc::{
    ListsView, ProfileDetail, ProfileRoutes, ProfileState, ProfileSummary, Request, Response,
    SettingsView, StatusReport,
};
use crate::profile::Pin;

pub struct Samples;

impl Samples {
    pub fn time() -> LocalTime {
        LocalTime::new(Scenario::now(), Scenario::OFFSET_SECS)
    }

    pub fn ago(secs: u64) -> SystemTime {
        Scenario::now() - Duration::from_secs(secs)
    }

    fn scenario(key: &str) -> Scenario {
        Scenario::parse(key).expect("a scenario")
    }

    fn ask(key: &str, request: Request) -> Response {
        DemoTransport::new(Self::scenario(key))
            .exchange(&request)
            .expect("the demo answers")
    }

    pub fn status(key: &str) -> StatusReport {
        match Self::ask(key, Request::Status) {
            Response::Status(status) => *status,
            other => panic!("status expected, got {other:?}"),
        }
    }

    pub fn detail(key: &str, name: &str) -> ProfileDetail {
        match Self::ask(key, Request::ProfileStatus { name: name.into() }) {
            Response::ProfileStatus(detail) => *detail,
            other => panic!("detail expected, got {other:?}"),
        }
    }

    pub fn routes(key: &str) -> Vec<ProfileRoutes> {
        match Self::ask(key, Request::Routes) {
            Response::Routes(routes) => routes,
            other => panic!("routes expected, got {other:?}"),
        }
    }

    pub fn lists(key: &str) -> ListsView {
        match Self::ask(key, Request::GetAllLists) {
            Response::Lists(lists) => *lists,
            other => panic!("lists expected, got {other:?}"),
        }
    }

    pub fn settings(key: &str) -> SettingsView {
        match Self::ask(key, Request::GetSettings) {
            Response::Settings(settings) => *settings,
            other => panic!("settings expected, got {other:?}"),
        }
    }

    pub fn summary(name: &str, state: ProfileState) -> ProfileSummary {
        ProfileSummary {
            name: name.into(),
            colour: Pin::new(2).expect("a pin"),
            state,
            since: None,
            uptime_secs: None,
            last_connected: None,
            handshake_age_secs: None,
            rx_rate: 0,
            tx_rate: 0,
            rx_total: 0,
            tx_total: 0,
            learned: 0,
            dns_rtt_ms: None,
            upstream: None,
            keepalive: 0,
            last_attempt: None,
            problem: None,
        }
    }
}
