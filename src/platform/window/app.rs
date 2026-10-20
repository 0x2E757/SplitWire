//! The window's frame loop: answers in, commands in, fields read, requests out, effects done, and
//! the view rendered when it changed. The frame is rebuilt when the theme or the width class
//! changes. The tray lives in the same loop and the same session: its clicks and menu items are
//! commands too, and its icon and tooltip follow the session's `Status`.

use std::cell::RefCell;
use std::rc::Rc;
use std::time::{Duration, Instant};

use super::Arguments;
use super::file_picker::ConfPicker;
use super::files::{ConfFiles, LogFiles, WindowStore};
use super::island::Island;
use super::link::EngineLink;
use super::markup::{Frame, Palette};
use super::native_window::NativeWindow;
use super::nav_drag::NavDrag;
use super::renderer::{Binder, Inbox, Renderer, SharedInbox};
use super::single_instance::Instance;
use super::startup::StartAtSignIn;
use super::system::{Clipboard, Folders, LocalOffset, Shell, SystemTheme};
use super::tray::{Tray, TrayEvent};
use crate::clock::{Clock, ManualClock, SystemClock};
use crate::platform::com::ComResult;
use crate::platform::pipe::{PipeClient, PipeName};
use crate::presentation::transport::EngineTransport;
use crate::presentation::transport::demo::{DemoTransport, Scenario};
use crate::presentation::tray::{Taskbar, TrayAction};
use crate::presentation::{Command, Effect, Placement, Session, WidthClass, WindowSettings};

/// How the frame looks: its theme and width class.
#[derive(Clone, Copy, PartialEq, Eq)]
struct Look {
    dark: bool,
    width: WidthClass,
}

pub struct WindowApp {
    session: Session,
    link: EngineLink,
    renderer: Renderer,
    inbox: SharedInbox,
    drag: Rc<RefCell<NavDrag>>,
    look: Look,
    forced_dark: Option<bool>,
    demo: Option<Scenario>,
    store: WindowStore,
    rendered: u64,
    second: Instant,
    visible: bool,
    owner: crate::platform::abi::Raw,
    instance: Instance,
    /// `None` when the notification icon's window could not be made: closing then ends.
    tray: Option<Tray>,
    taskbar: Taskbar,
    /// The session revision the tray last showed.
    tray_revision: u64,
    /// Shown since it was last closed (or never shown yet: false).
    open: bool,
    shown_once: bool,
    maximized: bool,
}

impl WindowApp {
    /// How long a right click waits for a fresh status before the menu opens.
    const MENU_WAIT: Duration = Duration::from_millis(300);

    /// The window's settings: the user's, or the defaults for a demo, which saves nothing.
    pub fn settings(arguments: &Arguments) -> (WindowStore, WindowSettings) {
        let store = WindowStore::new(&Folders::local());
        let settings = if arguments.demo.is_some() {
            WindowSettings::default()
        } else {
            store.load()
        };
        (store, settings)
    }

    pub fn new(
        arguments: &Arguments,
        instance: Instance,
        store: WindowStore,
        settings: WindowSettings,
        window: &NativeWindow,
        island: &Island,
    ) -> ComResult<Self> {
        let (transport, clock, offset): (Box<dyn EngineTransport>, Box<dyn Clock>, i64) =
            match arguments.demo {
                Some(scenario) => (
                    Box::new(DemoTransport::new(scenario)),
                    Box::new(ManualClock::new(Scenario::now())),
                    Scenario::OFFSET_SECS,
                ),
                None => (
                    Box::new(PipeClient::new(
                        arguments.pipe.clone().unwrap_or_else(PipeName::service),
                    )),
                    Box::new(SystemClock),
                    LocalOffset::now(),
                ),
            };
        let data = Folders::data();
        let session = Session::new(
            clock,
            offset,
            crate::presentation::Folders {
                data: data.display().to_string(),
                logs: data.join("logs"),
            },
            settings,
            StartAtSignIn::user().is_set(),
        );
        let inbox: SharedInbox = Rc::new(RefCell::new(Inbox::default()));
        let drag = Rc::new(RefCell::new(NavDrag::default()));
        let look = Look {
            dark: arguments
                .dark
                .unwrap_or_else(|| settings.theme.dark(SystemTheme::dark().unwrap_or(false))),
            width: WidthClass::of(window.width_dip()),
        };
        let renderer = Self::frame(look, &inbox, &drag)?;
        island.set_content(renderer.root())?;
        Self::tint(window, look.dark);
        let tray = Tray::new().ok();
        NativeWindow::keep_on_close(tray.is_some());
        Ok(Self {
            session,
            link: EngineLink::start(transport),
            renderer,
            inbox,
            drag,
            look,
            forced_dark: arguments.dark,
            demo: arguments.demo,
            store,
            rendered: 0,
            second: Instant::now(),
            // As the session starts: the first frame of a hidden window tells it otherwise.
            visible: true,
            owner: window.raw(),
            instance,
            tray,
            taskbar: SystemTheme::taskbar(),
            tray_revision: 0,
            open: false,
            shown_once: false,
            maximized: settings.placement.is_some_and(|p| p.maximized),
        })
    }

    fn frame(look: Look, inbox: &SharedInbox, drag: &Rc<RefCell<NavDrag>>) -> ComResult<Renderer> {
        let binder = Binder {
            inbox: inbox.clone(),
            palette: Palette { dark: look.dark },
            drag: drag.clone(),
        };
        Renderer::new(&Frame::skeleton(look.width), &Frame::HOSTS, binder)
    }

    fn tint(window: &NativeWindow, dark: bool) {
        let palette = Palette { dark };
        window.tint(dark, palette.color("bg"), palette.color("text"));
    }

    /// At start: the window opens unless the tray started alone (`--tray` at sign-in). Without
    /// a tray it always opens, or nothing would be left to click.
    pub fn start(&mut self, window: &NativeWindow, open: bool) {
        if open || self.tray.is_none() {
            self.show(window);
        }
    }

    /// Opens the window, or brings it to the front.
    fn show(&mut self, window: &NativeWindow) {
        if !self.shown_once {
            window.show(self.maximized);
            self.shown_once = true;
        }
        window.raise();
        self.open = true;
    }

    /// The tray's Exit: the window's frame is kept, then the loop ends.
    fn exit(&mut self, window: &NativeWindow) {
        if self.open {
            self.remember(window.placement());
        }
        self.open = false;
        NativeWindow::quit();
    }

    /// One frame of the loop.
    pub fn tick(&mut self, window: &NativeWindow, island: &Island) -> ComResult<()> {
        let now = Instant::now();
        if self.instance.show_requested() {
            self.show(window);
        }
        if self.instance.exit_requested() {
            self.exit(window);
        }
        if let Some(placement) = NativeWindow::take_closed() {
            self.open = false;
            self.remember(Some(placement));
        }
        self.tray_events(window, now);
        for (id, answer) in self.link.answers() {
            self.session.response(id, answer, now);
        }
        let dropped = NativeWindow::take_dropped();
        if !dropped.is_empty() {
            let paths: Vec<_> = dropped.into_iter().map(Into::into).collect();
            self.session
                .command(Command::FilesRead(ConfFiles::read(&paths)), now);
        }
        self.drops(now);
        let commands = std::mem::take(&mut self.inbox.borrow_mut().commands);
        for command in commands {
            self.session.command(command, now);
        }
        for command in self.renderer.poll_inputs() {
            self.session.command(command, now);
        }
        let visible = window.visible();
        if visible != self.visible {
            self.visible = visible;
            self.session.command(Command::Visible(visible), now);
        }
        self.session.tick(now);
        for (id, request) in self.session.take_requests() {
            self.link.send(id, request);
        }
        self.effects(window, now);
        let second = now.duration_since(self.second) >= Duration::from_secs(1);
        if second {
            self.second = now;
        }
        self.show_tray(second);
        let look = Look {
            dark: self.dark(second),
            width: WidthClass::of(window.width_dip()),
        };
        if look != self.look {
            self.look = look;
            self.renderer = Self::frame(look, &self.inbox, &self.drag)?;
            island.set_content(self.renderer.root())?;
            Self::tint(window, look.dark);
            self.rendered = 0;
        }
        if visible && (self.session.revision() != self.rendered || second) {
            self.rendered = self.session.revision();
            let view = self.session.view();
            {
                let mut drag = self.drag.borrow_mut();
                drag.count = view.nav.profiles.len();
                drag.pitch = if look.width.compact() { 40.0 } else { 38.0 };
            }
            self.renderer.render(&Frame::regions(&view, look.width))?;
        }
        let resets = self.session.take_resets();
        if !resets.is_empty() {
            self.renderer.reset(&resets);
        }
        self.renderer.refocus();
        Ok(())
    }

    /// A left click opens the window; a right click asks for the status and shows the menu.
    fn tray_events(&mut self, window: &NativeWindow, now: Instant) {
        let events = match self.tray.as_mut() {
            Some(tray) => tray.events(),
            None => return,
        };
        for event in events {
            let action = match event {
                TrayEvent::Open => Some(TrayAction::Open(None)),
                TrayEvent::Menu => {
                    self.refresh(now);
                    let menu = self.session.tray().menu;
                    self.tray
                        .as_ref()
                        .and_then(|tray| tray.menu(&menu, self.look.dark))
                }
                TrayEvent::TaskbarCreated => None,
            };
            match action {
                Some(TrayAction::Session(command)) => self.session.command(command, now),
                Some(TrayAction::Open(command)) => {
                    self.show(window);
                    if let Some(command) = command {
                        self.session.command(command, now);
                    }
                }
                Some(TrayAction::Exit) => self.exit(window),
                None => {}
            }
        }
    }

    /// Asks for the status at once and waits a moment for it, so the menu shows the service as it
    /// is now even while the window is hidden and polls every 10 s.
    fn refresh(&mut self, now: Instant) {
        self.session.command(Command::Refresh, now);
        self.session.tick(now);
        for (id, request) in self.session.take_requests() {
            self.link.send(id, request);
        }
        for (id, answer) in self.link.settle(Self::MENU_WAIT) {
            self.session.response(id, answer, now);
        }
    }

    /// The icon and tooltip, when the session changed; the taskbar's theme once a second.
    fn show_tray(&mut self, second: bool) {
        let Some(tray) = self.tray.as_mut() else {
            return;
        };
        if second {
            self.taskbar = SystemTheme::taskbar();
        }
        if second || self.session.revision() != self.tray_revision {
            self.tray_revision = self.session.revision();
            tray.show(&self.session.tray(), self.taskbar);
        }
    }

    fn dark(&self, check: bool) -> bool {
        if let Some(dark) = self.forced_dark {
            return dark;
        }
        if !check {
            return self.look.dark;
        }
        self.session
            .window_settings()
            .theme
            .dark(SystemTheme::dark().unwrap_or(self.look.dark))
    }

    /// Drags and drops on the drop zone whose items arrived.
    fn drops(&mut self, now: Instant) {
        let pending = std::mem::take(&mut self.inbox.borrow_mut().drops);
        let mut waiting = Vec::new();
        for drop in pending {
            match drop.poll() {
                Ok(None) => waiting.push(drop),
                Ok(Some(paths)) if drop.names_only => {
                    let names = paths
                        .iter()
                        .filter_map(|p| p.file_stem().map(|s| s.to_string_lossy().into_owned()))
                        .collect();
                    self.session.command(Command::DragOver(names), now);
                }
                Ok(Some(paths)) if !paths.is_empty() => {
                    self.session
                        .command(Command::FilesRead(ConfFiles::read(&paths)), now);
                }
                _ => self.session.command(Command::DragLeave, now),
            }
        }
        self.inbox.borrow_mut().drops.extend(waiting);
    }

    fn effects(&mut self, window: &NativeWindow, now: Instant) {
        for effect in self.session.take_effects() {
            match effect {
                Effect::Copy(text) => {
                    Clipboard::copy(self.owner, &text);
                }
                Effect::OpenFolder(path) => Shell::open(self.owner, &path),
                Effect::PickFiles => {
                    if let Ok(paths) = ConfPicker::pick(window.raw())
                        && !paths.is_empty()
                    {
                        self.session
                            .command(Command::FilesRead(ConfFiles::read(&paths)), now);
                    }
                }
                Effect::ReadLogFile => {
                    let text = match self.demo {
                        Some(scenario) => DemoTransport::log_file(scenario),
                        None => LogFiles::read(&Folders::data().join("logs")),
                    };
                    self.session.command(Command::LogFileRead(text), now);
                }
                Effect::SaveWindowSettings(settings) => {
                    if self.demo.is_none() {
                        let _ = self.store.save(&settings);
                    }
                    self.second = now - Duration::from_secs(1);
                }
                // A demo writes nothing; the switch shows the choice.
                Effect::StartAtSignIn(on) => {
                    if self.demo.is_none()
                        && let Ok(exe) = std::env::current_exe()
                    {
                        let _ = StartAtSignIn::user().set(on, &exe);
                    }
                }
                Effect::ScrollTop => {
                    self.renderer.scroll_top("PageScroll");
                    self.renderer.scroll_top("LogScroll");
                }
            }
        }
    }

    /// After the loop: the frame of a window closed without the tray.
    pub fn finish(&self) {
        if let Some(placement) = NativeWindow::take_closed() {
            self.remember(Some(placement));
        }
    }

    /// The window's frame, kept for the next start (not in a demo).
    fn remember(&self, placement: Option<Placement>) {
        if self.demo.is_some() {
            return;
        }
        let mut settings = self.session.window_settings();
        if let Some(placement) = placement {
            settings.placement = Some(placement);
            let _ = self.store.save(&settings);
        }
    }
}
