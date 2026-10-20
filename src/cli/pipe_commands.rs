use std::fs;
use std::io::{Read, Write};
use std::path::Path;

use super::text::{PreviewText, ReplyText, StatusText, Times};
use super::{ExitCode, PipeAction};
use crate::ipc::{ConfText, ProfileTarget, Request, Response};
use crate::platform::pipe::{PipeClient, PipeError};
use crate::profile::SecretBytes;

/// The command line's way to the engine; the pipe client, or a fake in tests.
pub trait EngineLink {
    fn exchange(&mut self, request: &Request) -> Result<Response, PipeError>;
    /// What a "not reachable" message names.
    fn describe(&self) -> String;
}

impl EngineLink for PipeClient {
    fn exchange(&mut self, request: &Request) -> Result<Response, PipeError> {
        PipeClient::exchange(self, request)
    }

    fn describe(&self) -> String {
        self.name().path()
    }
}

/// Where a command writes and reads.
pub struct Console<'a> {
    pub out: &'a mut dyn Write,
    pub err: &'a mut dyn Write,
    pub input: &'a mut dyn Read,
}

/// The pipe commands: each sends its request, prints the answer and returns the exit
/// code. A refusal is printed with its lines.
pub struct PipeCommands<'a, L: EngineLink> {
    link: L,
    console: Console<'a>,
    times: Times,
}

impl<'a, L: EngineLink> PipeCommands<'a, L> {
    pub fn new(link: L, console: Console<'a>, times: Times) -> Self {
        Self {
            link,
            console,
            times,
        }
    }

    pub fn run(&mut self, action: PipeAction) -> i32 {
        let result = match action {
            PipeAction::List => self.list(),
            PipeAction::Status { name } => self.status(name),
            PipeAction::Import {
                files,
                name,
                preview,
                replace,
            } => files
                .iter()
                .try_for_each(|file| self.import(file, name.as_deref(), preview, replace)),
            PipeAction::Connect { name } => self.connect(name),
            PipeAction::Disconnect { target } => self.disconnect(target),
            PipeAction::Remove { name } => self.remove(name),
            PipeAction::Lists { save } => self.lists(save),
            PipeAction::Alerts { dismiss, restore } => self.alerts(dismiss, restore),
        };
        match result {
            Ok(()) => ExitCode::OK,
            Err(code) => code,
        }
    }

    fn list(&mut self) -> Result<(), i32> {
        let Response::Status(status) = self.ask(&Request::Status)? else {
            return self.unexpected();
        };
        let text = StatusText::new(self.times).profiles(&status.profiles);
        self.print(&text)
    }

    fn status(&mut self, name: Option<String>) -> Result<(), i32> {
        let text = match name {
            None => match self.ask(&Request::Status)? {
                Response::Status(status) => StatusText::new(self.times).status(&status),
                _ => return self.unexpected(),
            },
            Some(name) => match self.ask(&Request::ProfileStatus { name })? {
                Response::ProfileStatus(detail) => StatusText::new(self.times).detail(&detail),
                _ => return self.unexpected(),
            },
        };
        self.print(&text)
    }

    /// Previews the file, then imports it with the colour the preview proposes.
    fn import(
        &mut self,
        file: &Path,
        name: Option<&str>,
        preview_only: bool,
        replace: bool,
    ) -> Result<(), i32> {
        let conf = self.read_conf(file)?;
        let name = name.map(str::to_owned).unwrap_or_else(|| {
            file.file_stem()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_default()
        });
        let file_name = file
            .file_name()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();
        let request = Request::PreviewImport {
            name: name.clone(),
            file_name,
            conf_text: self.conf_text(&conf)?,
        };
        let Response::ImportPreview(preview) = self.ask(&request)? else {
            return self.unexpected();
        };
        drop(request);
        if preview_only || preview.blocks() {
            self.print(&PreviewText::preview(&preview))?;
            return if preview.blocks() {
                Err(ExitCode::REFUSED)
            } else {
                Ok(())
            };
        }
        if preview.replace && !replace {
            return self.fail(
                ExitCode::REFUSED,
                &format!("{name} exists; add --replace to replace its .conf"),
            );
        }
        let request = Request::Import {
            name: name.clone(),
            conf_text: self.conf_text(&conf)?,
            colour: preview.colour,
            replace,
        };
        drop(conf);
        let Response::Imported(summary) = self.ask(&request)? else {
            return self.unexpected();
        };
        let verb = if replace { "Replaced" } else { "Imported" };
        let state = StatusText::new(self.times).state(&summary);
        self.print(&format!(
            "{verb} {} (colour pin {}): {state}\n",
            summary.name,
            summary.colour.number()
        ))
    }

    fn connect(&mut self, name: String) -> Result<(), i32> {
        let request = Request::Connect { name: name.clone() };
        match self.ask(&request)? {
            Response::Done => self.print(&format!("{name} is connected.\n")),
            Response::ConnectFailed { name, attempt } => {
                let text = StatusText::new(self.times).attempt(&attempt);
                self.fail(ExitCode::ERROR, &format!("{name} did not connect.\n{text}"))
            }
            _ => self.unexpected(),
        }
    }

    fn disconnect(&mut self, target: ProfileTarget) -> Result<(), i32> {
        let done = match &target {
            ProfileTarget::One(name) => format!("{name} is disconnected.\n"),
            ProfileTarget::All => "Every profile is disconnected.\n".to_owned(),
        };
        self.expect_done(&Request::Disconnect { target })?;
        self.print(&done)
    }

    fn remove(&mut self, name: String) -> Result<(), i32> {
        let done = format!("{name} is removed.\n");
        self.expect_done(&Request::RemoveProfile { name })?;
        self.print(&done)
    }

    fn lists(&mut self, save: bool) -> Result<(), i32> {
        let Response::Lists(view) = self.ask(&Request::GetAllLists)? else {
            return self.unexpected();
        };
        if !save {
            let mut text = view.text.clone();
            for problem in &view.problems {
                text.push_str(&format!("# problem: {problem}\n"));
            }
            return self.print(&text);
        }
        let mut text = String::new();
        if let Err(e) = self.console.input.read_to_string(&mut text) {
            return self.fail(
                ExitCode::ERROR,
                &format!("reading the new text failed: {e}"),
            );
        }
        let request = Request::SaveAllLists {
            base_rev: view.rev,
            text,
        };
        let Response::Lists(saved) = self.ask(&request)? else {
            return self.unexpected();
        };
        let mut text = "The lists are saved.\n".to_owned();
        for problem in &saved.problems {
            text.push_str(&format!("  {problem}\n"));
        }
        self.print(&text)
    }

    fn alerts(&mut self, dismiss: Option<String>, restore: bool) -> Result<(), i32> {
        if let Some(id) = dismiss {
            let done = format!("Dismissed {id}.\n");
            self.expect_done(&Request::DismissAlert { id })?;
            return self.print(&done);
        }
        if restore {
            self.expect_done(&Request::RestoreAlerts { id: None })?;
            return self.print("Every dismissed alert is shown again.\n");
        }
        let Response::Status(status) = self.ask(&Request::Status)? else {
            return self.unexpected();
        };
        self.print(&ReplyText::alerts(&status.alerts, &self.times))
    }

    /// The `.conf`, held in a buffer that is zeroed when dropped.
    fn read_conf(&mut self, file: &Path) -> Result<SecretBytes, i32> {
        match fs::read(file) {
            Ok(bytes) => Ok(SecretBytes::from_vec(bytes)),
            Err(e) => self.fail(
                ExitCode::ERROR,
                &format!("reading {} failed: {e}", file.display()),
            ),
        }
    }

    /// A copy for one request; `ConfText` zeroes it.
    fn conf_text(&mut self, conf: &SecretBytes) -> Result<ConfText, i32> {
        match String::from_utf8(conf.expose().to_vec()) {
            Ok(text) => Ok(ConfText::new(text)),
            Err(e) => {
                let mut bytes = e.into_bytes();
                crate::profile::Zeroize::bytes(&mut bytes);
                self.fail(ExitCode::ERROR, "the file is not UTF-8 text")
            }
        }
    }

    fn expect_done(&mut self, request: &Request) -> Result<(), i32> {
        match self.ask(request)? {
            Response::Done => Ok(()),
            _ => self.unexpected(),
        }
    }

    /// The answer, or the exit code after printing why there is none.
    fn ask(&mut self, request: &Request) -> Result<Response, i32> {
        match self.link.exchange(request) {
            Ok(Response::Error(reply)) => {
                let text = ReplyText::error(&reply);
                let _ = self.console.err.write_all(text.as_bytes());
                Err(ExitCode::of(&reply))
            }
            Ok(response) => Ok(response),
            Err(PipeError::NotRunning) => {
                let message = format!(
                    "the engine is not reachable: nothing serves {} (is the service running?)",
                    self.link.describe()
                );
                self.fail(ExitCode::NOT_REACHABLE, &message)
            }
            Err(PipeError::Failed(why)) => self.fail(ExitCode::ERROR, &why),
        }
    }

    fn unexpected<T>(&mut self) -> Result<T, i32> {
        self.fail(ExitCode::ERROR, "the engine gave an unexpected answer")
    }

    fn fail<T>(&mut self, code: i32, message: &str) -> Result<T, i32> {
        let _ = writeln!(self.console.err, "SplitWire: {}", message.trim_end());
        Err(code)
    }

    fn print(&mut self, text: &str) -> Result<(), i32> {
        match self.console.out.write_all(text.as_bytes()) {
            Ok(()) => Ok(()),
            Err(_) => Err(ExitCode::ERROR),
        }
    }
}
