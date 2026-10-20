//! The window's state machine: user commands become requests, responses become what the pages
//! show. It is pure: the platform hands it commands, responses and the time, and takes back
//! requests, effects and the view.

mod commands;
mod data;
mod polls;
mod responses;
mod tray;
mod ui;
mod view;

pub use data::{EngineData, ServiceState};
pub use ui::{Dialog, UiState};
pub use view::{DialogView, FrameView, PageView};

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use polls::{Poll, Polls};

use super::{Effect, InputId, LocalTime, Page, WindowSettings};
use crate::clock::Clock;
use crate::ipc::Request;

/// What a response answers.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Purpose {
    Poll(Poll),
    History,
    ListEdit {
        profile: String,
        clear: Option<InputId>,
        added: Option<String>,
        removed: Option<usize>,
    },
    ListDialog(String),
    SaveAllLists,
    SaveSettings(usize),
    Connect(String),
    Disconnect(String),
    ResetCache(String),
    Remove(String),
    Preview(usize, String),
    Import(usize),
    /// Anything that answers `Done`; a refusal becomes a toast.
    Action,
}

/// Where the window finds the service's files it reads directly.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Folders {
    /// The data folder, shown as text in Settings → About.
    pub data: String,
    /// `logs\`, opened by Open log folder.
    pub logs: PathBuf,
}

pub struct Session {
    clock: Box<dyn Clock>,
    offset_secs: i64,
    folders: Folders,
    data: EngineData,
    service: ServiceState,
    ui: UiState,
    window: WindowSettings,
    start_at_sign_in: bool,
    next_id: u64,
    outbox: Vec<(u64, Request)>,
    pending: HashMap<u64, Purpose>,
    polls: Polls,
    effects: Vec<Effect>,
    resets: Vec<(InputId, String)>,
    revision: u64,
    /// Moves when a Connect or Disconnect did not happen, so the switch the user flipped is shown
    /// as it is again.
    switches: u64,
}

impl Session {
    const TOAST: Duration = Duration::from_millis(1600);

    pub fn new(
        clock: Box<dyn Clock>,
        offset_secs: i64,
        folders: Folders,
        window: WindowSettings,
        start_at_sign_in: bool,
    ) -> Self {
        Self {
            clock,
            offset_secs,
            folders,
            data: EngineData::default(),
            service: ServiceState::Waiting,
            ui: UiState::default(),
            window,
            start_at_sign_in,
            next_id: 1,
            outbox: Vec::new(),
            pending: HashMap::new(),
            polls: Polls::default(),
            effects: Vec::new(),
            resets: Vec::new(),
            revision: 1,
            switches: 0,
        }
    }

    pub fn time(&self) -> LocalTime {
        LocalTime::new(self.clock.now(), self.offset_secs)
    }

    pub fn page(&self) -> &Page {
        &self.ui.page
    }

    pub fn service(&self) -> ServiceState {
        self.service
    }

    pub fn window_settings(&self) -> WindowSettings {
        self.window
    }

    /// Changes whenever something shown changed; the platform renders when it moves.
    pub fn revision(&self) -> u64 {
        self.revision
    }

    fn changed(&mut self) {
        self.revision += 1;
    }

    pub fn take_requests(&mut self) -> Vec<(u64, Request)> {
        std::mem::take(&mut self.outbox)
    }

    pub fn take_effects(&mut self) -> Vec<Effect> {
        std::mem::take(&mut self.effects)
    }

    /// Fields whose shown text the session replaced (a cleared Add field, a filled fix).
    pub fn take_resets(&mut self) -> Vec<(InputId, String)> {
        std::mem::take(&mut self.resets)
    }

    fn send(&mut self, request: Request, purpose: Purpose) {
        let id = self.next_id;
        self.next_id += 1;
        self.outbox.push((id, request));
        self.pending.insert(id, purpose);
    }

    fn reset_input(&mut self, id: InputId, text: String) {
        self.ui.set_input(&id, text.clone());
        self.resets.push((id, text));
    }

    fn toast(&mut self, text: impl Into<String>, now: Instant) {
        self.ui.toast = Some((text.into(), now + Self::TOAST));
        self.changed();
    }

    /// The schedule: polls the page needs, and the toast's end.
    pub fn tick(&mut self, now: Instant) {
        if self
            .ui
            .toast
            .as_ref()
            .is_some_and(|(_, until)| now >= *until)
        {
            self.ui.toast = None;
            self.changed();
        }
        let visible = self.ui.visible;
        let running = self.service == ServiceState::Running;
        let page = self.ui.page.clone();
        let profile = match &page {
            Page::Profile(name) => Some(name.clone()),
            _ => None,
        };
        let wants = |poll: Poll| match poll {
            Poll::Status => true,
            Poll::Traffic => running && matches!(page, Page::Status | Page::Profile(_)),
            Poll::Detail => running && profile.is_some(),
            Poll::Routes => running,
            Poll::Lists => running,
            Poll::Settings => running,
            Poll::Logs => running && page == Page::Logs,
        };
        for poll in [
            Poll::Status,
            Poll::Traffic,
            Poll::Detail,
            Poll::Routes,
            Poll::Lists,
            Poll::Settings,
            Poll::Logs,
        ] {
            // Routes are polled on their page; elsewhere they are fetched on a change.
            let scheduled = poll != Poll::Routes || page == Page::Routes;
            if !self
                .polls
                .ready(poll, wants(poll), visible && scheduled, now)
            {
                continue;
            }
            let request = match poll {
                Poll::Status => Request::Status,
                Poll::Traffic => Request::Traffic {
                    after_seq: self.data.traffic.last_seq(),
                },
                Poll::Detail => Request::ProfileStatus {
                    name: profile.clone().unwrap_or_default(),
                },
                Poll::Routes => Request::Routes,
                Poll::Lists => Request::GetAllLists,
                Poll::Settings => Request::GetSettings,
                Poll::Logs => Request::Logs {
                    after: self.data.logs.last_seq,
                },
            };
            self.polls.sent(poll, now);
            self.send(request, Purpose::Poll(poll));
        }
    }
}

#[cfg(test)]
pub(crate) mod tests;
