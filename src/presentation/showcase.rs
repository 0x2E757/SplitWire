//! Every page and state of the window for one demo scenario, as views: what `--verify-pages`
//! loads, and what the tests build markup from. A session is driven against the demo transport
//! the way the window drives it.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use super::transport::EngineTransport;
use super::transport::demo::{DemoTransport, Scenario};
use super::{
    Command, Folders, FrameView, InputId, ListsMode, LogSource, Page, PickedFile, Session,
    WindowSettings,
};
use crate::clock::{Clock, ManualClock};
use crate::ipc::ConfText;

/// A session and the engine it talks to, answered at once: the demo service, or any other
/// transport (the pipe to a dry-run engine in the integration tests).
pub struct Driver {
    pub session: Session,
    pub transport: Box<dyn EngineTransport>,
    pub now: Instant,
}

impl Driver {
    pub fn new(scenario: Scenario) -> Self {
        Self::over(
            Box::new(DemoTransport::new(scenario)),
            Box::new(ManualClock::new(Scenario::now())),
            Scenario::OFFSET_SECS,
        )
    }

    /// A session over `transport`, its times read from `clock` at `offset_secs` from UTC.
    pub fn over(
        transport: Box<dyn EngineTransport>,
        clock: Box<dyn Clock>,
        offset_secs: i64,
    ) -> Self {
        let session = Session::new(
            clock,
            offset_secs,
            Folders {
                data: r"C:\ProgramData\SplitWire".into(),
                logs: PathBuf::from(r"C:\ProgramData\SplitWire\logs"),
            },
            WindowSettings::default(),
            true,
        );
        let mut driver = Self {
            session,
            transport,
            now: Instant::now(),
        };
        driver.settle();
        driver
    }

    /// Ticks and answers until nothing is left to send.
    pub fn settle(&mut self) {
        for _ in 0..8 {
            self.session.tick(self.now);
            let requests = self.session.take_requests();
            if requests.is_empty() {
                break;
            }
            for (id, request) in requests {
                let answer = self.transport.exchange(&request);
                self.session.response(id, answer, self.now);
            }
        }
    }

    pub fn run(&mut self, command: Command) {
        self.session.command(command, self.now);
        self.settle();
    }

    pub fn later(&mut self, by: Duration) {
        self.now += by;
        self.settle();
    }
}

pub struct Showcase;

impl Showcase {
    /// A made-up `.conf` (none of its keys is real) for the review page.
    pub fn sample_conf(name_address: &str) -> String {
        format!(
            "[Interface]\nPrivateKey = yAnz5TF+lXXJte14tji3zlMNq+hd2rYUIgJBgB3fBmk=\nAddress = {name_address}\n\
             DNS = 10.77.0.53, partner.local\nListenPort = 51820\nPostUp = echo up\n\
             [Peer]\nPublicKey = xTIBA5rboUvnH4htodjb6e697QjLERt1NAB4mZqp8Dg=\n\
             AllowedIPs = 0.0.0.0/0, ::/0\nEndpoint = vpn.partner.example:51820\nPersistentKeepalive = 25\n"
        )
    }

    fn picked(file_name: &str, address: &str) -> PickedFile {
        PickedFile {
            file_name: file_name.into(),
            text: Ok(Arc::new(ConfText::new(Self::sample_conf(address)))),
        }
    }

    /// What a user types into a field holding `text`: the field's text after each key typed at its
    /// end, then after each Backspace back to `text`. The keys make the as-you-type checks change
    /// their verdict on the way (a bad entry, a listed one, a problem, new and bad lines).
    pub fn keystrokes(id: &InputId, text: &str) -> Vec<String> {
        let typed = match id {
            InputId::ProfileAdd | InputId::ListsAdd(_) => "contoso.example.",
            InputId::ListsText | InputId::ListDialog => {
                "
example.org
bad name
"
            }
            InputId::Setting(_) => "1280",
            InputId::BypassNew => "10.0.0.0/8",
            InputId::ReviewName(_) => "2 x",
            InputId::ProfileSearch
            | InputId::RoutesSearch
            | InputId::ListsSearch
            | InputId::LogFilter => "jira",
        };
        let mut texts = Vec::new();
        let mut now = text.to_owned();
        for key in typed.chars() {
            now.push(key);
            texts.push(now.clone());
        }
        while now.len() > text.len() {
            now.pop();
            texts.push(now.clone());
        }
        texts
    }

    /// `(state, view)` for every page and state the scenario can show.
    pub fn views(scenario: Scenario) -> Vec<(String, FrameView)> {
        let mut views = Vec::new();
        Self::walk(scenario, |label, driver| {
            views.push((label.to_owned(), driver.session.view()));
        });
        views
    }

    /// Drives a session through every page and state the scenario can show, handing each state to
    /// `visit` with its label. What `visit` does to the session carries on into the later states.
    pub fn walk(scenario: Scenario, visit: impl FnMut(&str, &mut Driver)) {
        Self::walk_with(Driver::new(scenario), scenario, visit);
    }

    /// [`walk`](Self::walk) with a driver of the caller's: over a transport whose service moves
    /// on while the pages are shown.
    pub fn walk_with(
        mut driver: Driver,
        scenario: Scenario,
        mut visit: impl FnMut(&str, &mut Driver),
    ) {
        let mut capture = |label: &str, driver: &mut Driver| {
            visit(&format!("{}:{label}", scenario.key()), driver);
        };
        capture("status", &mut driver);
        driver.run(Command::NavOverlay(true));
        capture("nav-overlay", &mut driver);
        driver.run(Command::NavOverlay(false));
        driver.run(Command::Go(Page::Add));
        capture("add", &mut driver);
        driver.run(Command::DragOver(vec!["Partner".into(), "Contoso".into()]));
        capture("add-drag", &mut driver);
        driver.run(Command::FilesRead(vec![
            Self::picked("Partner.conf", "10.77.0.5/32"),
            PickedFile {
                file_name: "big.conf".into(),
                text: Err("larger than 64 KB".into()),
            },
        ]));
        capture("add-bad-drop", &mut driver);
        driver.run(Command::FilesRead(vec![
            Self::picked("Partner.conf", "10.77.0.5/32"),
            Self::picked("AcmeCorp.conf", "10.88.4.12/32"),
            Self::picked("Acme Corp (old).conf", "10.66.0.2/32"),
        ]));
        capture("review", &mut driver);
        driver.run(Command::CancelImport);
        let names: Vec<String> = driver
            .session
            .view()
            .menus
            .iter()
            .map(|m| m.name.clone())
            .collect();
        for name in &names {
            driver.run(Command::Go(Page::Profile(name.clone())));
            capture(&format!("profile-{name}"), &mut driver);
        }
        if let Some(name) = names.first() {
            driver.run(Command::Go(Page::Profile(name.clone())));
            driver.run(Command::Input(
                InputId::ProfileAdd,
                "contoso.example".into(),
            ));
            driver.run(Command::Toggle(format!("profile:settings:{name}")));
            capture("profile-typing", &mut driver);
            driver.run(Command::EditListAsText(name.clone()));
            capture("dialog-list", &mut driver);
            driver.run(Command::AskRemove(name.clone()));
            capture("dialog-remove", &mut driver);
            driver.run(Command::CloseDialog);
        }
        driver.run(Command::Go(Page::Routes));
        capture("routes", &mut driver);
        if let Some(name) = names.first() {
            driver.run(Command::Toggle(format!("routes:{name}")));
            driver.run(Command::Input(InputId::RoutesSearch, "jira".into()));
            capture("routes-search", &mut driver);
        }
        driver.run(Command::Go(Page::Lists));
        capture("lists", &mut driver);
        driver.run(Command::ListsMode(ListsMode::Text));
        driver.run(Command::Input(InputId::ListsText, "acme.example\n".into()));
        driver.run(Command::SaveAllLists);
        capture("lists-text", &mut driver);
        driver.run(Command::Go(Page::Logs));
        capture("logs", &mut driver);
        driver.run(Command::LogFollow);
        capture("logs-paused", &mut driver);
        driver.run(Command::LogSource(LogSource::File));
        driver.run(Command::LogFileRead(DemoTransport::log_file(scenario)));
        capture("logs-file", &mut driver);
        driver.run(Command::Go(Page::Settings));
        capture("settings", &mut driver);
        for id in ["settings:dns", "settings:window", "settings:about"] {
            driver.run(Command::Toggle(id.into()));
        }
        driver.run(Command::Input(
            InputId::Setting(crate::presentation::SettingsEdit::field(
                crate::config::SettingsScope::Global,
                "max_ttl",
            )),
            "5".into(),
        ));
        capture("settings-open", &mut driver);
        driver.run(Command::Copy("x".into()));
        capture("toast", &mut driver);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::presentation::PageView;

    #[test]
    fn every_scenario_shows_its_pages() {
        let normal = Showcase::views(Scenario::Normal);
        assert!(normal.len() > 20, "{}", normal.len());
        let labels: Vec<&str> = normal.iter().map(|(l, _)| l.as_str()).collect();
        assert!(labels.contains(&"normal:profile-HomeLab"));
        assert!(
            normal
                .iter()
                .all(|(_, v)| !matches!(v.content, PageView::Loading(_)))
        );
        let review = normal.iter().find(|(l, _)| l == "normal:review").unwrap();
        let PageView::Review(review) = &review.1.content else {
            panic!("review")
        };
        assert_eq!(review.files.len(), 3);
        assert_eq!(review.files[1].replace.as_deref(), Some("AcmeCorp"));
        assert!(review.files[2].name_problem.is_some());
        let down = Showcase::views(Scenario::NoService);
        assert!(
            down.iter()
                .all(|(_, v)| matches!(v.content, PageView::ServiceDown(_)))
        );
    }
}
