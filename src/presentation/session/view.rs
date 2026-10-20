//! Everything the window shows, built from the session's state.

use super::{Dialog, ServiceState, Session};
use crate::ipc::Severity;
use crate::presentation::pages::{
    AddView, ListsPageView, ListsSource, LogsPageView, LogsSource, ProfilePageView, ProfileSource,
    ReviewView, RoutesPageView, RoutesSource, ServiceDownView, SettingsPageView, SettingsSource,
    StatusPageView, StatusSource,
};
use crate::presentation::{
    AlertList, ApplyBarView, BellFlyout, EntryCheck, InputId, ListsMode, NavSource, NavView, Page,
    ProfileMenu,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PageView {
    ServiceDown(ServiceDownView),
    Status(Box<StatusPageView>),
    Add(Box<AddView>),
    Profile(Box<ProfilePageView>),
    Routes(Box<RoutesPageView>),
    Lists(Box<ListsPageView>),
    Logs(Box<LogsPageView>),
    Settings(Box<SettingsPageView>),
    Review(Box<ReviewView>),
    /// The page's data has not arrived yet.
    Loading(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DialogView {
    Remove(String),
    ListText {
        profile: String,
        error: Option<String>,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FrameView {
    pub title: String,
    pub page: Page,
    pub nav: NavView,
    pub nav_overlay: bool,
    pub content: PageView,
    pub badge: Option<(usize, Severity)>,
    pub bell: BellFlyout,
    pub menus: Vec<ProfileMenu>,
    pub apply_bar: Option<ApplyBarView>,
    pub dialog: Option<DialogView>,
    pub toast: Option<String>,
    pub lists_mode: ListsMode,
    /// Expanders and groups whose open state differs from their default.
    pub flipped: Vec<String>,
    /// The typed text of every field, by element name.
    pub inputs: Vec<(String, String)>,
    /// Changes when a flipped Connect switch must show the service's state again.
    pub switches: u64,
}

impl FrameView {
    pub fn input(&self, id: &InputId) -> String {
        let element = id.element();
        self.inputs
            .iter()
            .find(|(e, _)| *e == element)
            .map(|(_, text)| text.clone())
            .unwrap_or_default()
    }

    pub fn open(&self, id: &str, default: bool) -> bool {
        default != self.flipped.iter().any(|f| f == id)
    }
}

impl Session {
    pub fn view(&self) -> FrameView {
        let time = self.time();
        let status = self.data.status.as_ref();
        let running = self.service == ServiceState::Running;
        let alerts = status.map_or(&[][..], |s| &s.alerts[..]);
        let list = AlertList::new(alerts);
        let global_invalid = status.is_some_and(|s| !s.problems.is_empty());
        let nav = NavSource {
            page: &self.ui.page,
            status: status.filter(|_| running),
            lists: self.data.lists.as_ref(),
            routes: self.data.routes.as_deref(),
            waiting: self.service == ServiceState::Waiting,
        }
        .view();
        let content = self.content();
        let apply_bar = match &content {
            PageView::Settings(view) => view.apply_bar.clone(),
            PageView::Profile(view) => view.apply_bar.clone(),
            _ => None,
        };
        FrameView {
            title: format!("SplitWire v{}", env!("CARGO_PKG_VERSION")),
            page: self.ui.page.clone(),
            nav,
            nav_overlay: self.ui.nav_overlay,
            content,
            badge: list.badge(),
            bell: list.flyout(
                &self.data.history,
                &time,
                self.service == ServiceState::Down,
            ),
            menus: status
                .map(|s| {
                    s.profiles
                        .iter()
                        .map(|p| ProfileMenu::of(p, global_invalid))
                        .collect()
                })
                .unwrap_or_default(),
            apply_bar,
            dialog: self.ui.dialog.as_ref().map(|d| match d {
                Dialog::Remove(name) => DialogView::Remove(name.clone()),
                Dialog::ListText { profile, error } => DialogView::ListText {
                    profile: profile.clone(),
                    error: error.clone(),
                },
            }),
            toast: self.ui.toast.as_ref().map(|(text, _)| text.clone()),
            lists_mode: self.ui.lists_mode,
            flipped: self.ui.flipped.iter().cloned().collect(),
            switches: self.switches,
            inputs: self
                .ui
                .inputs
                .iter()
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect(),
        }
    }

    fn content(&self) -> PageView {
        let time = self.time();
        let Some(status) = self.data.status.as_ref() else {
            return match self.service {
                ServiceState::Down => {
                    PageView::ServiceDown(ServiceDownView::new(self.data.last_answer, &time))
                }
                _ => PageView::Loading("Status".into()),
            };
        };
        let loading = |title: &str| PageView::Loading(title.into());
        match &self.ui.page {
            Page::Status if status.profiles.is_empty() => PageView::Add(Box::new(self.add(status))),
            Page::Status => {
                let traffic = |name: &str| self.data.traffic.samples(name);
                let listen = self
                    .data
                    .settings
                    .as_ref()
                    .and_then(|v| v.scopes.first())
                    .and_then(|s| s.values.iter().find(|e| e.key == "listen"))
                    .map_or("127.0.0.53".to_owned(), |e| {
                        e.value.split(':').next().unwrap_or_default().to_owned()
                    });
                let boot = |name: &str| {
                    let field = crate::presentation::SettingsEdit::field(
                        crate::config::SettingsScope::Profile(name.to_owned()),
                        "connect_at_boot",
                    );
                    self.data.settings.is_some()
                        && crate::presentation::SettingsEdit::saved(
                            self.data.settings.as_ref(),
                            &field,
                        ) == "true"
                };
                PageView::Status(Box::new(
                    StatusSource {
                        status,
                        traffic: &traffic,
                        listen,
                        boot: &boot,
                        time: &time,
                    }
                    .view(),
                ))
            }
            Page::Profile(name) => match &self.data.detail {
                Some(detail) if detail.summary.name == *name => {
                    let add = self.ui.input(&InputId::ProfileAdd);
                    let verdict = EntryCheck::new(self.data.lists.as_ref()).check(name, &add);
                    let search = self.ui.input(&InputId::ProfileSearch);
                    PageView::Profile(Box::new(
                        ProfileSource {
                            status,
                            detail,
                            settings: self.data.settings.as_ref(),
                            edit: &self.ui.settings,
                            traffic: self.data.traffic.samples(name),
                            add: &add,
                            verdict,
                            search: &search,
                            settings_open: self.ui.open(&format!("profile:settings:{name}"), false),
                            time: &time,
                        }
                        .view(),
                    ))
                }
                _ => loading(name),
            },
            Page::Routes => match &self.data.routes {
                Some(routes) => {
                    let search = self.ui.input(&InputId::RoutesSearch);
                    let open = |name: &str| self.ui.open(&format!("routes:{name}"), false);
                    PageView::Routes(Box::new(
                        RoutesSource {
                            status,
                            routes,
                            lists: self.data.lists.as_ref(),
                            search: &search,
                            open: &open,
                            time: &time,
                        }
                        .view(),
                    ))
                }
                None => loading("Routes"),
            },
            Page::Lists => match &self.data.lists {
                Some(lists) => {
                    let search = self.ui.input(&InputId::ListsSearch);
                    let adds = |name: &str| self.ui.input(&InputId::ListsAdd(name.to_owned()));
                    let collapsed = |name: &str| !self.ui.open(&format!("lists:{name}"), true);
                    PageView::Lists(Box::new(
                        ListsSource {
                            status,
                            lists,
                            search: &search,
                            adds: &adds,
                            collapsed: &collapsed,
                            draft: self.ui.lists_draft.as_deref(),
                            refused: &self.ui.lists_refused,
                        }
                        .view(),
                    ))
                }
                None => loading("Lists"),
            },
            Page::Logs => {
                let filter = self.ui.input(&InputId::LogFilter);
                let (file, bytes) = self
                    .data
                    .log_file
                    .as_ref()
                    .map_or((&[][..], 0), |(lines, bytes)| (&lines[..], *bytes));
                PageView::Logs(Box::new(
                    LogsSource {
                        status: Some(status),
                        live: &self.data.logs.lines,
                        file,
                        file_bytes: bytes,
                        filter: &filter,
                        ui: &self.ui.logs,
                        time: &time,
                    }
                    .view(),
                ))
            }
            Page::Settings => {
                let bypass_new = self.ui.input(&InputId::BypassNew);
                let open = |id: &str, default: bool| self.ui.open(id, default);
                PageView::Settings(Box::new(
                    SettingsSource {
                        status,
                        settings: self.data.settings.as_ref(),
                        edit: &self.ui.settings,
                        window: &self.window,
                        start_at_sign_in: self.start_at_sign_in,
                        bypass_new: &bypass_new,
                        data_folder: &self.folders.data,
                        open: &open,
                        time: &time,
                    }
                    .view(),
                ))
            }
            Page::Add => PageView::Add(Box::new(self.add(status))),
            Page::Review => match &self.ui.import {
                Some(batch) => PageView::Review(Box::new(ReviewView::of(batch, status))),
                None => PageView::Add(Box::new(self.add(status))),
            },
        }
    }

    fn add(&self, status: &crate::ipc::StatusReport) -> AddView {
        let (unread, readable): (&[crate::presentation::UnreadFile], Vec<String>) =
            match &self.ui.import {
                Some(batch) => (
                    &batch.unread,
                    batch.files.iter().map(|f| f.file_name.clone()).collect(),
                ),
                None => (&[], Vec::new()),
            };
        AddView::of(status, self.ui.dragging.as_deref(), unread, &readable)
    }
}
