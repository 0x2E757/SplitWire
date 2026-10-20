use std::sync::Arc;
use std::time::SystemTime;

use super::control_replies::ControlReplies as Replies;
use super::wire_alerts::WireAlerts;
use super::{Coordinator, ListsViewBuilder, ProfileReport, SettingsViewBuilder, StatusReporter};
use crate::clock::local_midnight;
use crate::ipc::{
    ConfText, ErrorCode, ErrorReply, LineRef, LogLine, LogsPage, ProfileTarget, Request, Response,
};
use crate::logs::Log;
use crate::profile::{ImportRequest, Pin};

/// The pipe's dispatcher: every request mapped to the coordinator or a report, and
/// the answer in the wire's types. A request that cannot be decoded gets `bad_request`.
pub struct Control {
    coordinator: Arc<Coordinator>,
    reporter: StatusReporter,
    log: Log,
}

impl Control {
    /// `started` is when the engine started (`Status.since`).
    pub fn new(coordinator: Arc<Coordinator>, started: SystemTime, log: Log) -> Self {
        Self {
            reporter: StatusReporter::new(coordinator.clone(), started),
            coordinator,
            log,
        }
    }

    /// One request frame body, as the pipe read it, to a whole response frame.
    pub fn serve(&self, body: &[u8]) -> Vec<u8> {
        let response = match Request::decode(body) {
            Ok(request) => self.answer(request),
            Err(e) => Replies::undecodable(&e),
        };
        response.encode().unwrap_or_else(|e| {
            Response::Error(ErrorReply {
                code: ErrorCode::BadRequest,
                message: format!("the answer is too large: {e}"),
            })
            .encode()
            .expect("a short error encodes")
        })
    }

    pub fn answer(&self, request: Request) -> Response {
        match request {
            Request::Status => Response::Status(Box::new(self.reporter.status())),
            Request::ProfileStatus { name } => self.profile_status(&name),
            Request::Routes => Response::Routes(self.report().routes()),
            Request::Traffic { after_seq } => Response::Traffic(self.reporter.traffic(after_seq)),
            Request::Logs { after } => Response::Logs(self.logs(after)),
            Request::AlertHistory => self.alert_history(),
            Request::DismissAlert { id } => match self.parts().alerts.dismiss(&id) {
                Ok(()) => Response::Done,
                Err(e) => Replies::dismiss(e),
            },
            Request::DismissAll => {
                self.parts().alerts.dismiss_all();
                Response::Done
            }
            Request::RestoreAlerts { id } => {
                self.parts().alerts.restore(id.as_deref());
                Response::Done
            }
            Request::GetAllLists => self.lists(),
            Request::EditList {
                base_rev,
                name,
                add,
                remove,
            } => self.list_change(self.coordinator.edit_list(
                base_rev,
                &name,
                add.as_deref(),
                remove.as_ref().map(|r: &LineRef| (r.line, r.text.as_str())),
            )),
            Request::SaveList {
                base_rev,
                name,
                text,
            } => self.list_change(self.coordinator.save_list(base_rev, &name, &text)),
            Request::SaveAllLists { base_rev, text } => {
                self.list_change(self.coordinator.save_all_lists(base_rev, &text))
            }
            Request::GetSettings => Response::Settings(Box::new(self.settings())),
            Request::SaveSettings { base_rev, changes } => {
                match self.coordinator.save_settings(base_rev, &changes) {
                    Ok((_, reconnects)) => Response::SettingsSaved(Box::new(
                        SettingsViewBuilder::saved(self.settings(), reconnects),
                    )),
                    Err(e) => Replies::settings(e),
                }
            }
            Request::ResetCache { name } => match self.coordinator.reset_cache(name.as_deref()) {
                Ok((entries, routes)) => Response::CacheReset { entries, routes },
                Err(e) => Replies::coordinator(e, name.as_deref().unwrap_or_default()),
            },
            Request::Connect { name } => Self::done_or(self.coordinator.connect(&name), &name),
            Request::Disconnect { target } => match target {
                ProfileTarget::One(name) => {
                    Self::done_or(self.coordinator.disconnect(&name), &name)
                }
                ProfileTarget::All => {
                    self.coordinator.disconnect_all(false);
                    Response::Done
                }
            },
            Request::PreviewImport {
                name,
                file_name,
                conf_text,
            } => self.preview(&name, &file_name, &conf_text),
            Request::Import {
                name,
                conf_text,
                colour,
                replace,
            } => self.import(&name, &conf_text, colour, replace),
            Request::RemoveProfile { name } => match self.coordinator.remove(&name) {
                Ok(()) => Response::Done,
                Err(e) => Replies::profile(e),
            },
            Request::SetProfileColor { name, colour } => {
                match self.coordinator.set_colour(&name, colour) {
                    Ok(()) => Response::Done,
                    Err(e) => Replies::profile(e),
                }
            }
            Request::MoveProfile { name, index } => {
                match self.coordinator.move_profile(&name, index) {
                    Ok(()) => Response::Done,
                    Err(e) => Replies::profile(e),
                }
            }
        }
    }

    fn profile_status(&self, name: &str) -> Response {
        match self.report().detail(name) {
            Some(Ok(detail)) => Response::ProfileStatus(Box::new(detail)),
            Some(Err(problem)) => Replies::bad_request(format!("{name}: {problem}")),
            None => Replies::unknown_profile(name),
        }
    }

    /// The newest lines after `after`, at most a page; `last_seq` is the next `after`.
    fn logs(&self, after: u64) -> LogsPage {
        let last = self.log.last_seq();
        let from = after.max(last.saturating_sub(LogsPage::MAX_ENTRIES as u64));
        let entries: Vec<LogLine> = self
            .log
            .since(from, LogsPage::MAX_ENTRIES)
            .iter()
            .map(LogLine::from)
            .collect();
        LogsPage {
            last_seq: entries.last().map_or(last, |e| e.seq),
            entries,
        }
    }

    /// The alerts cleared or dismissed since local midnight.
    fn alert_history(&self) -> Response {
        let clock = &self.parts().clock;
        let now = clock.now();
        let midnight = local_midnight(now, clock.utc_offset_secs(now));
        Response::AlertHistory(
            self.parts()
                .alerts
                .history(midnight)
                .iter()
                .map(WireAlerts::alert)
                .collect(),
        )
    }

    fn lists(&self) -> Response {
        match self.coordinator.list_texts() {
            Ok(texts) => {
                let catalog = self.coordinator.catalog();
                let view = ListsViewBuilder::new(&self.parts().active, &catalog).view(&texts);
                Response::Lists(Box::new(view))
            }
            Err(e) => Replies::bad_request(format!("reading the lists failed: {e}")),
        }
    }

    fn list_change(&self, result: Result<(), super::ListRefusal>) -> Response {
        match result {
            Ok(()) => self.lists(),
            Err(e) => Replies::list(e),
        }
    }

    fn settings(&self) -> crate::ipc::SettingsView {
        SettingsViewBuilder::new(&self.parts().settings_files)
            .view(self.coordinator.settings_rev(), &self.coordinator.catalog())
    }

    fn preview(&self, name: &str, file_name: &str, conf_text: &ConfText) -> Response {
        let request = ImportRequest {
            name,
            file_name,
            conf_text: conf_text.expose(),
        };
        Response::ImportPreview(Box::new(self.coordinator.preview_import(&request)))
    }

    fn import(&self, name: &str, conf_text: &ConfText, colour: Pin, replace: bool) -> Response {
        match self
            .coordinator
            .import(name, conf_text.expose(), colour, replace)
        {
            Ok(_) => match self.reporter.summary_of(name) {
                Some(summary) => Response::Imported(Box::new(summary)),
                None => Replies::unknown_profile(name),
            },
            Err(e) => Replies::profile(e),
        }
    }

    fn done_or(result: Result<(), super::CoordinatorError>, name: &str) -> Response {
        match result {
            Ok(()) => Response::Done,
            Err(e) => Replies::coordinator(e, name),
        }
    }

    fn report(&self) -> ProfileReport<'_> {
        ProfileReport::new(&self.coordinator, &self.reporter)
    }

    fn parts(&self) -> &super::EngineParts {
        self.coordinator.parts()
    }
}
