//! What each answer changes.

use std::time::Instant;

use super::polls::Poll;
use super::{Dialog, Purpose, ServiceState, Session};
use crate::ipc::{
    ErrorCode, ErrorReply, ProfileState, RefusedProblem, Request, Response, StatusReport,
};
use crate::presentation::transport::TransportError;
use crate::presentation::{InputId, Page};

impl Session {
    pub fn response(&mut self, id: u64, result: Result<Response, TransportError>, now: Instant) {
        let Some(purpose) = self.pending.remove(&id) else {
            return;
        };
        if let Purpose::Poll(poll) = &purpose {
            self.polls.answered(*poll);
        }
        self.changed();
        let response = match result {
            Ok(response) => response,
            Err(TransportError::NotRunning) => return self.service_down(),
            Err(TransportError::Failed(reason)) => {
                return self.toast(format!("The service did not answer: {reason}"), now);
            }
        };
        if self.service != ServiceState::Running {
            self.service = ServiceState::Running;
            self.polls.request(Poll::Lists);
            self.polls.request(Poll::Routes);
            self.polls.request(Poll::Settings);
        }
        self.data.last_answer = Some(self.clock.now());
        if let Response::Error(error) = response {
            return self.refused(purpose, error, now);
        }
        match (purpose, response) {
            (Purpose::Poll(_), Response::Status(status)) => self.status(*status),
            (Purpose::Poll(_), Response::Traffic(traffic)) => self.data.traffic.add(traffic),
            (Purpose::Poll(_), Response::ProfileStatus(detail)) => {
                if matches!(&self.ui.page, Page::Profile(name) if *name == detail.summary.name) {
                    self.data.detail = Some(*detail);
                }
            }
            (Purpose::Poll(_), Response::Routes(routes)) => self.data.routes = Some(routes),
            (Purpose::Poll(_), Response::Lists(lists)) => self.data.lists = Some(*lists),
            (Purpose::Poll(_), Response::Settings(settings)) => {
                self.data.settings = Some(*settings)
            }
            (Purpose::Poll(_), Response::Logs(page)) => {
                self.data.logs.add(page.entries, page.last_seq)
            }
            (Purpose::History, Response::AlertHistory(history)) => self.data.history = history,
            (
                Purpose::ListEdit {
                    profile,
                    clear,
                    added,
                    removed,
                },
                Response::Lists(lists),
            ) => {
                let line = lists
                    .files
                    .iter()
                    .find(|f| f.name == profile)
                    .and_then(|f| f.lines.last())
                    .map(|l| l.line);
                self.data.lists = Some(*lists);
                if let Some(input) = clear {
                    self.reset_input(input, String::new());
                }
                match (added, removed, line) {
                    (Some(_), _, Some(line)) => {
                        self.toast(format!("Added to {profile} as line {line}"), now)
                    }
                    (_, Some(line), _) => {
                        self.toast(format!("Line {line} removed from {profile}"), now)
                    }
                    _ => {}
                }
                self.after_change();
            }
            (Purpose::ListDialog(profile), Response::Lists(lists)) => {
                self.data.lists = Some(*lists);
                self.ui.dialog = None;
                self.toast(format!("{profile}’s list saved"), now);
                self.after_change();
            }
            (Purpose::SaveAllLists, Response::Lists(lists)) => {
                let count = lists.files.len();
                self.ui.lists_draft = None;
                self.ui.lists_refused.clear();
                self.reset_input(InputId::ListsText, lists.text.clone());
                self.data.lists = Some(*lists);
                self.toast(format!("Saved {count} files, validated together"), now);
                self.after_change();
            }
            (Purpose::SaveSettings(count), Response::SettingsSaved(saved)) => {
                self.ui.settings.revert();
                self.data.settings = Some(saved.settings);
                let failed: Vec<&str> = saved
                    .reconnects
                    .iter()
                    .filter(|r| matches!(r.outcome, crate::ipc::ReconnectOutcome::Failed(_)))
                    .map(|r| r.name.as_str())
                    .collect();
                let text = if failed.is_empty() {
                    format!(
                        "Saved {count} {}",
                        if count == 1 { "change" } else { "changes" }
                    )
                } else {
                    format!("Saved; reconnecting {} failed", failed.join(", "))
                };
                self.toast(text, now);
                self.after_change();
            }
            (Purpose::Connect(name), Response::ConnectFailed { .. }) => {
                self.switches += 1;
                self.toast(format!("Connect {name} failed; see its page"), now);
                self.after_change();
            }
            (Purpose::ResetCache(name), Response::CacheReset { entries, .. }) => {
                self.toast(
                    format!(
                        "DNS cache of {name} reset · {}",
                        crate::presentation::Format::plural(entries as usize, "entry", "entries")
                    ),
                    now,
                );
                self.after_change();
            }
            (Purpose::Remove(name), _) => {
                self.go_status_if_on(&name);
                self.toast(format!("{name} removed"), now);
                self.after_change();
            }
            (Purpose::Preview(index, name), Response::ImportPreview(preview)) => {
                if let Some(file) = self.ui.import.as_mut().and_then(|b| b.files.get_mut(index)) {
                    file.preview = Some(*preview);
                    file.previewed = Some(name);
                }
            }
            (Purpose::Import(index), Response::Imported(summary)) => {
                self.imported(index, *summary, now)
            }
            (Purpose::Connect(_) | Purpose::Disconnect(_) | Purpose::Action, _) => {
                self.after_change()
            }
            _ => {}
        }
    }

    /// Something changed in the service: ask for its status at once.
    fn after_change(&mut self) {
        self.polls.request(Poll::Status);
        self.polls.request(Poll::Routes);
        if matches!(self.ui.page, Page::Profile(_)) {
            self.polls.request(Poll::Detail);
        }
    }

    fn go_status_if_on(&mut self, name: &str) {
        if matches!(&self.ui.page, Page::Profile(n) if n == name) {
            self.ui.page = Page::Status;
        }
    }

    fn service_down(&mut self) {
        if self.service != ServiceState::Down {
            self.service = ServiceState::Down;
            self.data.forget();
            self.pending.clear();
            self.polls.reset();
            self.ui.page = Page::Status;
            self.ui.dialog = None;
            self.ui.import = None;
        }
    }

    fn status(&mut self, status: StatusReport) {
        // The bell's history is fetched before it opens: an answer while it is open would
        // rebuild the header and close it.
        if self
            .data
            .status
            .as_ref()
            .is_none_or(|old| old.revs.alerts != status.revs.alerts)
        {
            self.send(Request::AlertHistory, Purpose::History);
        }
        if let Some(old) = &self.data.status {
            if old.revs.lists != status.revs.lists {
                self.polls.request(Poll::Lists);
            }
            if old.revs.settings != status.revs.settings {
                self.polls.request(Poll::Settings);
            }
            if old.revs.profiles != status.revs.profiles {
                self.polls.request(Poll::Routes);
            }
        }
        let connected: Vec<&str> = status
            .profiles
            .iter()
            .filter(|p| matches!(p.state, ProfileState::Connected(_)))
            .map(|p| p.name.as_str())
            .collect();
        self.data.traffic.retain(&connected);
        if let Page::Profile(name) = &self.ui.page
            && !status.profiles.iter().any(|p| p.name == *name)
        {
            self.ui.page = Page::Status;
        }
        if status.profiles.is_empty() && self.ui.page.needs_profiles() {
            self.ui.page = Page::Status;
        }
        self.data.status = Some(status);
    }

    fn imported(&mut self, index: usize, summary: crate::ipc::ProfileSummary, now: Instant) {
        let Some(batch) = &mut self.ui.import else {
            return;
        };
        if let Some(file) = batch.files.get_mut(index) {
            file.imported = true;
        }
        batch.importing = None;
        batch.done.push(summary);
        if batch.next().is_some() {
            self.import_next();
            return;
        }
        let done = std::mem::take(&mut batch.done);
        let replaced = batch.files.iter().all(|f| f.replace());
        self.ui.import = None;
        self.after_change();
        self.polls.request(Poll::Lists);
        if let Some(first) = done.first() {
            let name = first.name.clone();
            self.ui.page = Page::Profile(name.clone());
            self.data.detail = None;
            self.polls.request(Poll::Detail);
            self.toast(
                if replaced {
                    format!("{name} replaced")
                } else {
                    format!("{name} added · add entries to its list on its page")
                },
                now,
            );
        }
    }

    fn refused(&mut self, purpose: Purpose, error: ErrorReply, now: Instant) {
        if matches!(purpose, Purpose::Connect(_) | Purpose::Disconnect(_)) {
            self.switches += 1;
        }
        if error.code == ErrorCode::StaleRev {
            self.polls.request(Poll::Lists);
            self.polls.request(Poll::Settings);
            return self.toast(error.message, now);
        }
        let problems: Vec<String> = match &error.code {
            ErrorCode::Refused(problems) => problems
                .iter()
                .map(|p| match p {
                    RefusedProblem::List(problem) => problem.to_string(),
                    RefusedProblem::Setting(problem) => problem.to_string(),
                    RefusedProblem::Text(text) => text.clone(),
                })
                .collect(),
            _ => Vec::new(),
        };
        let first = problems.first().cloned().unwrap_or(error.message.clone());
        match purpose {
            Purpose::SaveAllLists => {
                if let ErrorCode::Refused(problems) = error.code {
                    self.ui.lists_refused = problems
                        .into_iter()
                        .filter_map(|p| match p {
                            RefusedProblem::List(problem) => Some(problem),
                            _ => None,
                        })
                        .collect();
                }
            }
            Purpose::ListDialog(profile) => {
                self.ui.dialog = Some(Dialog::ListText {
                    profile,
                    error: Some(problems.join(" · ")),
                });
            }
            Purpose::Import(index) => {
                if let Some(batch) = &mut self.ui.import {
                    batch.importing = None;
                    let file = batch
                        .files
                        .get(index)
                        .map(|f| f.name.clone())
                        .unwrap_or_default();
                    batch.failure = Some(format!(
                        "Importing {file} failed: {first}. The files before it stay imported."
                    ));
                }
                self.after_change();
            }
            Purpose::Poll(Poll::Detail) => self.ui.page = Page::Status,
            Purpose::Poll(_) | Purpose::History | Purpose::Preview(..) => {}
            _ => self.toast(first, now),
        }
    }
}
