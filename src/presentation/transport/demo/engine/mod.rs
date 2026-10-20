//! The demo service: answers every request from its state and applies what the user does
//! (connect, dismiss, list edits, settings, imports), as the real service would.

mod import;
mod lists;
mod settings;

use std::time::{Duration, Instant};

use super::Scenario;
use super::model::{DemoModel, DemoProfile, DemoRoute};
use super::reports::DemoReports;
use super::scenarios::DemoScenarios;
use super::traffic::DemoTraffic;
use crate::ipc::{
    ConnectStep, ErrorCode, ErrorReply, LogsPage, ProfileState, ProfileTarget, ProfileTraffic,
    RefusedProblem, Request, Response, Revisions, RouteRow,
};
use crate::list::{ListSet, ListTarget};
use crate::tunnel::HandshakeHealth;

pub struct DemoEngine {
    started: Instant,
    scenario: Scenario,
    model: DemoModel,
    revs: Revisions,
}

impl DemoEngine {
    /// How long a demo Connect takes.
    const CONNECT_TIME: Duration = Duration::from_millis(1400);

    pub fn new(scenario: Scenario) -> Self {
        Self {
            started: Instant::now(),
            scenario,
            model: DemoScenarios::model(scenario),
            revs: Revisions {
                profiles: 1,
                lists: 1,
                settings: 1,
                alerts: 1,
            },
        }
    }

    pub fn service_running(&self) -> bool {
        self.model.service
    }

    pub fn handle(&mut self, request: Request) -> Response {
        self.advance();
        match request {
            Request::Status => {
                Response::Status(Box::new(DemoReports::status(&self.model, self.revs)))
            }
            Request::ProfileStatus { name } => match self.model.profile(&name) {
                Some(profile) => {
                    Response::ProfileStatus(Box::new(DemoReports::detail(&self.model, profile)))
                }
                None => Self::unknown(&name),
            },
            Request::Routes => Response::Routes(DemoReports::routes(&self.model)),
            Request::Traffic { after_seq } => Response::Traffic(
                self.model
                    .profiles
                    .iter()
                    .filter(|p| p.connected())
                    .map(|p| ProfileTraffic {
                        name: p.name.clone(),
                        samples: DemoTraffic::samples(
                            p.chart,
                            after_seq,
                            self.seq(),
                            p.traffic_from,
                        ),
                    })
                    .collect(),
            ),
            Request::Logs { after } => {
                let entries: Vec<_> = self
                    .model
                    .logs
                    .iter()
                    .filter(|l| l.seq > after)
                    .take(LogsPage::MAX_ENTRIES)
                    .cloned()
                    .collect();
                let last_seq = self.model.logs.last().map_or(after, |l| l.seq);
                Response::Logs(LogsPage { entries, last_seq })
            }
            Request::AlertHistory => {
                let mut history = self.model.history.clone();
                history.extend(self.model.alerts.iter().filter(|a| a.dismissed).cloned());
                Response::AlertHistory(history)
            }
            Request::DismissAlert { id } => self.dismiss(Some(&id), true),
            Request::DismissAll => self.dismiss(None, true),
            Request::RestoreAlerts { id } => self.dismiss(id.as_deref(), false),
            Request::GetAllLists => self.lists(),
            Request::EditList {
                base_rev,
                name,
                add,
                remove,
            } => self.edit_list(base_rev, &name, add, remove),
            Request::SaveList {
                base_rev,
                name,
                text,
            } => self.save_list(base_rev, &name, text),
            Request::SaveAllLists { base_rev, text } => self.save_all_lists(base_rev, &text),
            Request::GetSettings => Response::Settings(Box::new(DemoReports::settings(
                &self.model,
                self.revs.settings,
            ))),
            Request::SaveSettings { base_rev, changes } => self.save_settings(base_rev, &changes),
            Request::ResetCache { .. } => Response::CacheReset {
                entries: self.model.cache_entries,
                routes: self.model.profiles.iter().map(DemoProfile::learned).sum(),
            },
            Request::Connect { name } => self.connect(&name),
            Request::Disconnect { target } => {
                let now = Scenario::now();
                for profile in &mut self.model.profiles {
                    if matches!(&target, ProfileTarget::One(n) if !n.eq_ignore_ascii_case(&profile.name))
                    {
                        continue;
                    }
                    if profile.connected() || matches!(profile.state, ProfileState::Connecting(_)) {
                        profile.disconnect(now);
                    }
                }
                self.revs.profiles += 1;
                Response::Done
            }
            Request::PreviewImport {
                name,
                file_name,
                conf_text,
            } => Response::ImportPreview(Box::new(self.preview(
                &name,
                &file_name,
                conf_text.expose(),
            ))),
            Request::Import {
                name,
                conf_text,
                colour,
                replace,
            } => self.import(&name, conf_text.expose(), colour, replace),
            Request::RemoveProfile { name } => {
                let before = self.model.profiles.len();
                self.model
                    .profiles
                    .retain(|p| !p.name.eq_ignore_ascii_case(&name));
                if self.model.profiles.len() == before {
                    return Self::unknown(&name);
                }
                self.model
                    .alerts
                    .retain(|a| a.profile.as_deref() != Some(name.as_str()));
                self.bump_all();
                Response::Done
            }
            Request::SetProfileColor { name, colour } => match self.model.profile_mut(&name) {
                Some(profile) => {
                    profile.colour = colour;
                    self.revs.profiles += 1;
                    Response::Done
                }
                None => Self::unknown(&name),
            },
            Request::MoveProfile { name, index } => {
                let Some(from) = self
                    .model
                    .profiles
                    .iter()
                    .position(|p| p.name.eq_ignore_ascii_case(&name))
                else {
                    return Self::unknown(&name);
                };
                let profile = self.model.profiles.remove(from);
                let index = index.min(self.model.profiles.len());
                self.model.profiles.insert(index, profile);
                self.bump_all();
                Response::Done
            }
        }
    }

    /// The newest traffic sample: five minutes of history at start, one more each second.
    fn seq(&self) -> u64 {
        DemoTraffic::SAMPLES + self.started.elapsed().as_secs()
    }

    fn bump_all(&mut self) {
        self.revs.profiles += 1;
        self.revs.lists += 1;
        self.revs.settings += 1;
        self.revs.alerts += 1;
    }

    fn unknown(name: &str) -> Response {
        Self::error(ErrorCode::UnknownProfile, &format!("no profile {name}"))
    }

    fn error(code: ErrorCode, message: &str) -> Response {
        Response::Error(ErrorReply {
            code,
            message: message.into(),
        })
    }

    fn refused(message: &str) -> Response {
        Self::error(
            ErrorCode::Refused(vec![RefusedProblem::Text(message.into())]),
            message,
        )
    }

    /// A Connect in progress completes after `CONNECT_TIME`.
    fn advance(&mut self) {
        for profile in &mut self.model.profiles {
            let Some(started) = profile.connecting_since else {
                continue;
            };
            if started.elapsed() < Self::CONNECT_TIME {
                continue;
            }
            profile.connecting_since = None;
            profile.traffic_from = DemoTraffic::SAMPLES + self.started.elapsed().as_secs();
            profile.state = ProfileState::Connected(HandshakeHealth::Ok);
            profile.since = Some(Scenario::now());
            profile.uptime_secs = Some(0);
            profile.last_connected = profile.since;
            profile.handshake_age_secs = Some(2);
            profile.rx_rate = (profile.chart.level * 1000.0) as u64;
            profile.tx_rate = (profile.chart.level * 300.0) as u64;
            profile.dns_rtt_ms = Some(20);
            profile.routes = Self::fresh_routes(profile);
            self.revs.profiles += 1;
        }
    }

    /// The routes of a profile that just connected: its upstream DNS and its static entries.
    fn fresh_routes(profile: &DemoProfile) -> Vec<DemoRoute> {
        let list = ListSet::load([(profile.name.as_str(), profile.list.as_str())]);
        let mut routes: Vec<DemoRoute> = list
            .get(&profile.name)
            .map(|l| {
                l.entries
                    .iter()
                    .filter_map(|e| match &e.target {
                        ListTarget::Network(net) => Some(DemoRoute {
                            line: Some(e.line),
                            row: RouteRow::Static { net: *net },
                        }),
                        ListTarget::Domain(_) => None,
                    })
                    .collect()
            })
            .unwrap_or_default();
        routes.extend(profile.dns.iter().map(|server| DemoRoute {
            line: None,
            row: RouteRow::Upstream { ip: *server.ip() },
        }));
        routes
    }

    fn connect(&mut self, name: &str) -> Response {
        let global_invalid = !self.model.global_problems().is_empty();
        let scenario = self.scenario;
        let Some(profile) = self.model.profile_mut(name) else {
            return Self::unknown(name);
        };
        if profile.connected() || matches!(profile.state, ProfileState::Connecting(_)) {
            return Response::Done;
        }
        if global_invalid {
            return Self::refused("splitwire.conf is invalid: every profile stays disconnected");
        }
        if !profile.settings_problems().is_empty() {
            return Self::refused(&format!("{}’s profile.conf is invalid", profile.name));
        }
        if scenario == Scenario::ConnectFail && profile.name == "HomeLab" {
            let mut attempt = DemoScenarios::failed_attempt();
            attempt.at = Scenario::now();
            profile.state = ProfileState::Failed;
            profile.attempt = Some(attempt.clone());
            self.revs.profiles += 1;
            return Response::ConnectFailed {
                name: profile.name.clone(),
                attempt,
            };
        }
        profile.state = ProfileState::Connecting(ConnectStep::Peer);
        profile.connecting_since = Some(Instant::now());
        profile.attempt = None;
        self.model.alerts.retain(|a| {
            !(a.profile.as_deref() == Some(name) && a.kind == crate::ipc::AlertKind::ConnectFailed)
        });
        self.revs.profiles += 1;
        self.revs.alerts += 1;
        Response::Done
    }

    fn dismiss(&mut self, id: Option<&str>, dismissed: bool) -> Response {
        if dismissed
            && let Some(id) = id
            && self.model.alerts.iter().any(|a| a.id == id && a.blocking)
        {
            return Self::refused("this alert keeps a profile off; fix its cause instead");
        }
        for alert in &mut self.model.alerts {
            if id.is_none_or(|id| alert.id == id) && !(dismissed && alert.blocking) {
                alert.dismissed = dismissed;
            }
        }
        self.revs.alerts += 1;
        Response::Done
    }

    /// The text of `logs\splitwire.log` the demo serves for the Logs page's file view.
    pub fn log_file(&self) -> &str {
        &self.model.log_file
    }

    #[cfg(test)]
    pub fn model(&self) -> &DemoModel {
        &self.model
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ipc::ProfileTarget;

    #[test]
    fn connect_completes_and_disconnect_turns_off() {
        let mut engine = DemoEngine::new(Scenario::Normal);
        assert_eq!(
            engine.handle(Request::Disconnect {
                target: ProfileTarget::One("AcmeCorp".into())
            }),
            Response::Done
        );
        assert_eq!(engine.model().profiles[1].state, ProfileState::Off);
        assert_eq!(
            engine.handle(Request::Connect {
                name: "AcmeCorp".into()
            }),
            Response::Done
        );
        assert!(matches!(
            engine.model().profiles[1].state,
            ProfileState::Connecting(_)
        ));
        engine.model.profiles[1].connecting_since = Some(Instant::now() - Duration::from_secs(2));
        engine.advance();
        assert!(engine.model().profiles[1].connected());
    }

    #[test]
    fn a_blocked_profile_and_a_blocking_alert_are_refused() {
        let mut engine = DemoEngine::new(Scenario::Normal);
        let refused = engine.handle(Request::Connect {
            name: "HomeLab".into(),
        });
        assert!(matches!(
            refused,
            Response::Error(ErrorReply {
                code: ErrorCode::Refused(_),
                ..
            })
        ));
        let id = engine.model().alerts[0].id.clone();
        assert!(matches!(
            engine.handle(Request::DismissAlert { id }),
            Response::Error(_)
        ));
        assert_eq!(engine.handle(Request::DismissAll), Response::Done);
        assert!(
            engine
                .model()
                .alerts
                .iter()
                .filter(|a| !a.blocking)
                .all(|a| a.dismissed)
        );
    }

    #[test]
    fn the_failing_scenario_fails_again() {
        let mut engine = DemoEngine::new(Scenario::ConnectFail);
        let response = engine.handle(Request::Connect {
            name: "HomeLab".into(),
        });
        assert!(matches!(response, Response::ConnectFailed { .. }));
    }
}
