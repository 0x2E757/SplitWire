//! What each command does: requests, state, effects.

use std::sync::Arc;
use std::time::Instant;

use super::polls::Poll;
use super::{Dialog, Purpose, ServiceState, Session};
use crate::config::SettingsScope;
use crate::ipc::{ConfText, LineRef, ProfileTarget, Request, SettingField};
use crate::presentation::{
    Command, Effect, EntryCheck, ImportBatch, InputId, LogSource, Page, SettingsEdit,
};

impl Session {
    pub fn command(&mut self, command: Command, now: Instant) {
        self.changed();
        match command {
            Command::Go(page) => self.go(page),
            Command::NavOverlay(open) => self.ui.nav_overlay = open,
            Command::SetConnected(name, true) => self.send(
                Request::Connect { name: name.clone() },
                Purpose::Connect(name),
            ),
            Command::SetConnected(name, false) => self.send(
                Request::Disconnect {
                    target: ProfileTarget::One(name.clone()),
                },
                Purpose::Disconnect(name),
            ),
            Command::OpenLog(name) => {
                self.ui.logs.profile = crate::presentation::ProfileFilter::Profile(name);
                self.go(Page::Logs);
            }
            Command::AskRemove(name) => self.ui.dialog = Some(Dialog::Remove(name)),
            Command::Remove(name) => {
                self.ui.dialog = None;
                self.send(
                    Request::RemoveProfile { name: name.clone() },
                    Purpose::Remove(name),
                );
            }
            Command::CloseDialog => self.ui.dialog = None,
            Command::MoveProfile(name, index) => {
                self.send(Request::MoveProfile { name, index }, Purpose::Action)
            }
            Command::Dismiss(id) => self.send(Request::DismissAlert { id }, Purpose::Action),
            Command::DismissAll => self.send(Request::DismissAll, Purpose::Action),
            Command::RestoreAlerts => {
                self.send(Request::RestoreAlerts { id: None }, Purpose::Action)
            }
            Command::FollowAlert(id) => self.follow_alert(&id),
            Command::Toggle(id) => self.ui.flip(&id),
            Command::Input(id, text) => self.input(id, text),
            Command::Submit(id) => match id {
                InputId::ProfileAdd => {
                    if let Page::Profile(name) = self.ui.page.clone() {
                        self.add_entry(name, now);
                    }
                }
                InputId::ListsAdd(name) => self.add_entry(name, now),
                InputId::BypassNew => self.bypass_add(now),
                _ => {}
            },
            Command::Copy(text) => {
                self.effects.push(Effect::Copy(text));
                self.toast("Copied", now);
            }
            Command::AddEntry(profile) => self.add_entry(profile, now),
            Command::RemoveLine {
                profile,
                line,
                text,
            } => self.edit_list(profile, None, Some(LineRef { line, text }), None),
            Command::EditListAsText(profile) => self.edit_as_text(profile),
            Command::SaveListDialog => self.save_list_dialog(),
            Command::ListsMode(mode) => self.ui.lists_mode = mode,
            Command::SaveAllLists => self.save_all_lists(),
            Command::RevertAllLists => {
                self.ui.lists_draft = None;
                self.ui.lists_refused.clear();
                let text = self
                    .data
                    .lists
                    .as_ref()
                    .map(|l| l.text.clone())
                    .unwrap_or_default();
                self.reset_input(InputId::ListsText, text);
            }
            Command::LogSource(source) => {
                self.ui.logs.source = source;
                if source == LogSource::File {
                    self.effects.push(Effect::ReadLogFile);
                }
            }
            Command::LogLevel(level) => self.ui.logs.level = level,
            Command::LogProfile(profile) => self.ui.logs.profile = profile,
            Command::LogFollow => {
                self.ui.logs.paused_at = match self.ui.logs.paused_at {
                    Some(_) => None,
                    None => Some(self.data.logs.last_seq),
                };
            }
            Command::LogLatest => {
                self.ui.logs.paused_at = None;
                self.effects.push(Effect::ScrollTop);
            }
            Command::OpenLogFolder => self
                .effects
                .push(Effect::OpenFolder(self.folders.logs.clone())),
            Command::SettingToggle(field) => {
                let view = self.data.settings.as_ref();
                self.ui.settings.toggle(view, field);
            }
            Command::SettingUse(field, value) => {
                self.ui.settings.set(field.clone(), value.clone());
                self.reset_input(InputId::Setting(field), value);
            }
            Command::BypassRemove(name) => {
                let field = SettingsEdit::field(SettingsScope::Global, "bypass");
                let current = self.ui.settings.value(self.data.settings.as_ref(), &field);
                let kept: Vec<&str> = current
                    .split(',')
                    .map(str::trim)
                    .filter(|n| !n.is_empty() && *n != name)
                    .collect();
                self.ui.settings.set(field, kept.join(","));
            }
            Command::BypassAdd => self.bypass_add(now),
            Command::SettingsRevert => self.revert_settings(),
            Command::SettingsApply => self.apply_settings(),
            Command::StartAtSignIn(on) => {
                self.start_at_sign_in = on;
                self.effects.push(Effect::StartAtSignIn(on));
                self.toast("Saved for your account", now);
            }
            Command::OpenWindowAtSignIn(on) => {
                self.window.open_window_at_sign_in = on;
                self.effects.push(Effect::SaveWindowSettings(self.window));
                self.toast("Saved for your account", now);
            }
            Command::Theme(theme) => {
                self.window.theme = theme;
                self.effects.push(Effect::SaveWindowSettings(self.window));
            }
            Command::PickFiles { replace } => {
                self.ui.replace = replace;
                self.effects.push(Effect::PickFiles);
            }
            Command::FilesRead(files) => self.files_read(files),
            Command::DragOver(names) => self.ui.dragging = Some(names),
            Command::DragLeave => self.ui.dragging = None,
            Command::ReviewColour(index, pin) => {
                if let Some(file) = self.ui.import.as_mut().and_then(|b| b.files.get_mut(index)) {
                    file.colour = Some(pin);
                }
            }
            Command::Import => {
                if self.ui.import.as_ref().is_some_and(ImportBatch::ready) {
                    self.import_next();
                }
            }
            Command::CancelImport => {
                self.ui.import = None;
                self.go(Page::Add);
            }
            Command::LogFileRead(text) => {
                let lines = crate::presentation::LogFile::parse(&text);
                self.data.log_file = Some((lines, text.len()));
            }
            Command::Visible(visible) => self.ui.visible = visible,
            Command::ResetCache(name) => self.send(
                Request::ResetCache {
                    name: Some(name.clone()),
                },
                Purpose::ResetCache(name),
            ),
            Command::Refresh => self.polls.request(Poll::Status),
        }
    }

    fn go(&mut self, page: Page) {
        let running = self.service == ServiceState::Running;
        let profiles = self
            .data
            .status
            .as_ref()
            .is_some_and(|s| !s.profiles.is_empty());
        if (page.needs_service() && !running) || (page.needs_profiles() && !profiles) {
            return;
        }
        if page != self.ui.page {
            if let Page::Profile(_) = page {
                self.data.detail = None;
                self.polls.request(Poll::Detail);
                self.polls.request(Poll::Traffic);
            }
            self.effects.push(Effect::ScrollTop);
        }
        match page {
            Page::Routes => self.polls.request(Poll::Routes),
            Page::Lists | Page::Profile(_) => {
                self.polls.request(Poll::Lists);
                self.polls.request(Poll::Settings);
            }
            Page::Settings => self.polls.request(Poll::Settings),
            Page::Logs if self.ui.logs.source == LogSource::File => {
                self.effects.push(Effect::ReadLogFile)
            }
            _ => {}
        }
        self.ui.page = page;
        self.ui.nav_overlay = false;
        self.ui.dialog = None;
        self.ui.dragging = None;
    }

    fn follow_alert(&mut self, id: &str) {
        let Some(alert) = self
            .data
            .status
            .as_ref()
            .and_then(|s| s.alerts.iter().find(|a| a.id == id))
            .cloned()
        else {
            return;
        };
        if let Some(suggest) = &alert.suggest {
            self.ui
                .settings
                .set(suggest.field.clone(), suggest.value.clone());
            self.reset_input(
                InputId::Setting(suggest.field.clone()),
                suggest.value.clone(),
            );
        }
        let Some(action) = alert.action else {
            return;
        };
        if let crate::ipc::ActionTarget::Settings(Some(SettingField { scope, .. })) = &action.target
        {
            let id = match scope {
                SettingsScope::Global => "settings:cache".to_owned(),
                SettingsScope::Profile(name) => format!("settings:profile:{name}"),
            };
            self.ui.force_open(&id, false);
        }
        match Command::target(&action.target) {
            Command::OpenLog(name) => self.command(Command::OpenLog(name), Instant::now()),
            Command::Go(page) => self.go(page),
            _ => {}
        }
    }

    fn input(&mut self, id: InputId, text: String) {
        match &id {
            InputId::Setting(field) => self.ui.settings.set(field.clone(), text.clone()),
            InputId::ListsText => {
                self.ui.lists_draft = Some(text.clone());
                self.ui.lists_refused.clear();
            }
            InputId::ReviewName(index) => {
                if let Some(file) = self
                    .ui
                    .import
                    .as_mut()
                    .and_then(|b| b.files.get_mut(*index))
                {
                    file.name = text.trim().to_owned();
                }
                self.preview_pending();
            }
            _ => {}
        }
        self.ui.set_input(&id, text);
    }

    fn add_entry(&mut self, profile: String, now: Instant) {
        let input = match &self.ui.page {
            Page::Profile(name) if *name == profile => InputId::ProfileAdd,
            _ => InputId::ListsAdd(profile.clone()),
        };
        let text = self.ui.input(&input).trim().to_owned();
        let verdict = EntryCheck::new(self.data.lists.as_ref()).check(&profile, &text);
        if verdict.blocks() {
            if let Some(message) = verdict.message() {
                self.toast(format!("{text}: {message}"), now);
            }
            return;
        }
        self.edit_list(profile, Some(text), None, Some(input));
    }

    fn edit_list(
        &mut self,
        profile: String,
        add: Option<String>,
        remove: Option<LineRef>,
        clear: Option<InputId>,
    ) {
        let Some(base_rev) = self.data.lists.as_ref().map(|l| l.rev) else {
            self.polls.request(Poll::Lists);
            return;
        };
        let removed = remove.as_ref().map(|r| r.line);
        self.send(
            Request::EditList {
                base_rev,
                name: profile.clone(),
                add: add.clone(),
                remove,
            },
            Purpose::ListEdit {
                profile,
                clear,
                added: add,
                removed,
            },
        );
    }

    fn edit_as_text(&mut self, profile: String) {
        let text = self
            .data
            .lists
            .as_ref()
            .and_then(|l| l.files.iter().find(|f| f.name == profile))
            .map(|f| {
                f.lines
                    .iter()
                    .map(|l| l.text.as_str())
                    .collect::<Vec<_>>()
                    .join("\n")
            })
            .unwrap_or_default();
        self.reset_input(InputId::ListDialog, text);
        self.ui.dialog = Some(Dialog::ListText {
            profile,
            error: None,
        });
    }

    fn save_list_dialog(&mut self) {
        let Some(Dialog::ListText { profile, .. }) = self.ui.dialog.clone() else {
            return;
        };
        let Some(base_rev) = self.data.lists.as_ref().map(|l| l.rev) else {
            return;
        };
        let mut text = self.ui.input(&InputId::ListDialog);
        if !text.is_empty() && !text.ends_with('\n') {
            text.push('\n');
        }
        self.send(
            Request::SaveList {
                base_rev,
                name: profile.clone(),
                text,
            },
            Purpose::ListDialog(profile),
        );
    }

    fn save_all_lists(&mut self) {
        let (Some(text), Some(base_rev)) = (
            self.ui.lists_draft.clone(),
            self.data.lists.as_ref().map(|l| l.rev),
        ) else {
            return;
        };
        self.send(
            Request::SaveAllLists { base_rev, text },
            Purpose::SaveAllLists,
        );
    }

    fn bypass_add(&mut self, now: Instant) {
        let name = self
            .ui
            .input(&InputId::BypassNew)
            .trim()
            .to_ascii_lowercase();
        if let Err(reason) = crate::config::SettingValue::names(&name) {
            self.toast(
                format!("A bypass name needs at least 2 labels: {reason}"),
                now,
            );
            return;
        }
        let field = SettingsEdit::field(SettingsScope::Global, "bypass");
        let current = self.ui.settings.value(self.data.settings.as_ref(), &field);
        let mut names: Vec<&str> = current
            .split(',')
            .map(str::trim)
            .filter(|n| !n.is_empty())
            .collect();
        if !names.contains(&name.as_str()) {
            names.push(&name);
        }
        let joined = names.join(",");
        self.ui.settings.set(field, joined);
        self.reset_input(InputId::BypassNew, String::new());
    }

    fn revert_settings(&mut self) {
        let view = self.data.settings.as_ref();
        let fields: Vec<SettingField> = self
            .ui
            .settings
            .changed_fields(view)
            .into_iter()
            .cloned()
            .collect();
        let saved: Vec<(SettingField, String)> = fields
            .into_iter()
            .map(|f| {
                let value = SettingsEdit::saved(view, &f);
                (f, value)
            })
            .collect();
        self.ui.settings.revert();
        for (field, value) in saved {
            self.reset_input(InputId::Setting(field), value);
        }
    }

    fn apply_settings(&mut self) {
        let view = self.data.settings.as_ref();
        let Some(base_rev) = view.map(|v| v.rev) else {
            return;
        };
        let changes = self.ui.settings.changes(view);
        let invalid = self
            .ui
            .settings
            .changed_fields(view)
            .iter()
            .any(|f| self.ui.settings.problem(view, f).is_some());
        if changes.is_empty() || invalid {
            return;
        }
        let count = changes.len();
        self.send(
            Request::SaveSettings { base_rev, changes },
            Purpose::SaveSettings(count),
        );
    }

    fn files_read(&mut self, files: Vec<crate::presentation::PickedFile>) {
        let replace = self.ui.replace.take();
        let batch = ImportBatch::new(files, replace);
        let unread = !batch.unread.is_empty();
        self.ui.dragging = None;
        self.ui.import = Some(batch);
        if unread {
            self.go(Page::Add);
        } else {
            self.go(Page::Review);
        }
        self.preview_pending();
    }

    /// Previews the files whose name changed since their last preview.
    fn preview_pending(&mut self) {
        let Some(batch) = &self.ui.import else {
            return;
        };
        let wanted: Vec<(usize, String, String, Arc<ConfText>)> = batch
            .to_preview()
            .into_iter()
            .map(|(i, name)| {
                let file = &batch.files[i];
                (i, name, file.file_name.clone(), file.text.clone())
            })
            .collect();
        for (index, name, file_name, text) in wanted {
            self.send(
                Request::PreviewImport {
                    name: name.clone(),
                    file_name,
                    conf_text: ConfText::new(text.expose().to_owned()),
                },
                Purpose::Preview(index, name),
            );
        }
    }

    /// Imports the next file of the batch; the next one follows its answer.
    pub(super) fn import_next(&mut self) {
        let Some(batch) = &mut self.ui.import else {
            return;
        };
        let Some(index) = batch.next() else {
            return;
        };
        batch.importing = Some(index);
        batch.failure = None;
        let file = &batch.files[index];
        let request = Request::Import {
            name: file.name.clone(),
            conf_text: ConfText::new(file.text.expose().to_owned()),
            colour: file.colour(),
            replace: file.replace(),
        };
        self.send(request, Purpose::Import(index));
    }

    /// The review of a bad drop's readable files only.
    pub fn review_readable(&mut self) {
        if let Some(batch) = &mut self.ui.import {
            batch.unread.clear();
        }
        self.go(Page::Review);
    }
}
