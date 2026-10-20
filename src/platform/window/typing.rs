//! The typing check of `--verify-pages`: in every state of every demo scenario and both width
//! classes, every field is typed into the way a user does it. Each key puts the field's new text
//! into its TextBox with the caret at the end, then runs what the frame loop runs: the fields are
//! polled, their `Input`s go to the session, the view is rendered and the resets written. The
//! field must stay the same TextBox, with the typed text and the caret where they were: a region
//! reloaded under a field takes the focus away from the user.

use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

use super::error_info::ErrorInfo;
use super::focus::Focused;
use super::markup::{Frame, Kit, LineBreaks, Out, Palette, Xml};
use super::nav_drag::NavDrag;
use super::renderer::{Binder, Inbox, Renderer};
use super::xaml::Xaml;
use crate::platform::com::Com;
use crate::presentation::showcase::{Driver, Showcase};
use crate::presentation::transport::demo::Scenario;
use crate::presentation::{Command, InputId, WidthClass};

/// Fields typed into and keys typed.
#[derive(Default)]
pub struct Typed {
    pub fields: usize,
    pub keys: usize,
}

pub struct TypingCheck {
    width: WidthClass,
    renderer: Renderer,
}

impl TypingCheck {
    /// Every state of every scenario, a fresh walk per state so typing leaves the others as they
    /// are; then a reload under a focused field.
    pub fn run() -> Result<Typed, String> {
        Self::reload()?;
        let mut typed = Typed::default();
        let mut failures = Vec::new();
        for scenario in Scenario::ALL {
            let states = Showcase::views(scenario).len();
            for state in 0..states {
                let mut at = 0;
                Showcase::walk(scenario, |label, driver| {
                    if at == state {
                        Self::state(label, driver, &mut typed, &mut failures);
                    }
                    at += 1;
                });
            }
        }
        if failures.is_empty() {
            Ok(typed)
        } else {
            Err(failures.join(
                "
",
            ))
        }
    }

    /// Every field of one state; a field's first failure is kept, and the next field typed into.
    fn state(label: &str, driver: &mut Driver, typed: &mut Typed, failures: &mut Vec<String>) {
        for width in WidthClass::ALL {
            let label = format!("{label} {width:?}");
            let mut check = match Self::mount(driver, width, &label) {
                Ok(check) => check,
                Err(failure) => {
                    failures.push(failure);
                    continue;
                }
            };
            let ids: Vec<InputId> = check.renderer.fields().into_iter().map(|f| f.0).collect();
            for id in ids {
                if let Err(failure) = check.field(driver, &id, &label, typed) {
                    failures.push(failure);
                    // A reloaded frame starts the next field afresh.
                    match Self::mount(driver, width, &label) {
                        Ok(fresh) => check = fresh,
                        Err(failure) => {
                            failures.push(failure);
                            break;
                        }
                    }
                }
            }
        }
    }

    fn mount(driver: &mut Driver, width: WidthClass, label: &str) -> Result<Self, String> {
        let binder = Binder {
            inbox: Rc::new(RefCell::new(Inbox::default())),
            palette: Palette::BOTH[0],
            drag: Rc::new(RefCell::new(NavDrag::default())),
        };
        let renderer = Renderer::new(&Frame::skeleton(width), &Frame::HOSTS, binder)
            .map_err(|hr| Self::failed(label, "the frame does not load", hr))?;
        let mut check = Self { width, renderer };
        check.render(driver, label)?;
        Ok(check)
    }

    fn render(&mut self, driver: &mut Driver, label: &str) -> Result<(), String> {
        self.renderer
            .render(&Frame::regions(&driver.session.view(), self.width))
            .map_err(|hr| Self::failed(label, "rendering failed", hr))?;
        self.renderer.reset(&driver.session.take_resets());
        Ok(())
    }

    fn element(&self, id: &InputId) -> Option<Com> {
        self.renderer
            .fields()
            .into_iter()
            .find(|(field, _)| field == id)
            .map(|(_, element)| element)
    }

    /// Types into one field, key by key, backs out to its text again and lets a second pass. A
    /// failure puts the field's text back in the session, so the next field starts as the state
    /// does.
    fn field(
        &mut self,
        driver: &mut Driver,
        id: &InputId,
        label: &str,
        typed: &mut Typed,
    ) -> Result<(), String> {
        // A field another field's text hid (a search) is back once that text is.
        let Some(element) = self.element(id) else {
            return Ok(());
        };
        let text = Xaml::text(&element)
            .map(|t| LineBreaks::typed(&t))
            .map_err(|hr| Self::failed(label, &format!("{} has no text", id.element()), hr))?;
        typed.fields += 1;
        let keys = Showcase::keystrokes(id, &text).into_iter().map(Some);
        for key in keys.chain([None]) {
            let step = key
                .as_ref()
                .map_or("a second later".into(), |k| format!("typing {k:?}"));
            let failed = |what: &str| format!("{label}: {step} into {}: {what}", id.element());
            let result = self.key(
                driver,
                &element,
                key.as_deref().unwrap_or(&text),
                key.is_some(),
            );
            if let Err(what) = result
                .and_then(|caret| self.kept(id, &element, key.as_deref().unwrap_or(&text), caret))
            {
                driver.run(Command::Input(id.clone(), text));
                return Err(failed(&what));
            }
            typed.keys += 1;
        }
        Ok(())
    }

    /// One frame of the loop after a key (`typed`) or after a second; the caret it left.
    fn key(
        &mut self,
        driver: &mut Driver,
        element: &Com,
        text: &str,
        typed: bool,
    ) -> Result<i32, String> {
        let shown = LineBreaks::shown(text);
        let caret = shown.encode_utf16().count() as i32;
        let hex = |hr: i32| format!("0x{:08X}", hr as u32);
        if typed {
            Xaml::set_text(element, &shown)
                .and_then(|_| Xaml::select(element, (caret, 0)))
                .map_err(hex)?;
        } else {
            driver.later(Duration::from_secs(1));
        }
        for command in self.renderer.poll_inputs() {
            driver.run(command);
        }
        self.renderer
            .render(&Frame::regions(&driver.session.view(), self.width))
            .map_err(hex)?;
        self.renderer.reset(&driver.session.take_resets());
        Ok(caret)
    }

    /// The field is the same TextBox, with `text` and the caret where the key left it.
    fn kept(&self, id: &InputId, element: &Com, text: &str, caret: i32) -> Result<(), String> {
        match self.element(id) {
            Some(now) if now.raw() == element.raw() => {}
            Some(_) => return Err("its region was reloaded".into()),
            None => return Err("the field is gone".into()),
        }
        let now = Xaml::text(element).map(|t| LineBreaks::typed(&t));
        if now.as_deref() != Ok(text) {
            return Err(format!("the field holds {now:?}"));
        }
        let at = Xaml::selection(element);
        if at != Ok((caret, 0)) {
            return Err(format!("the caret moved from {caret} to {at:?}"));
        }
        Ok(())
    }

    /// A region that reloads under the focused field anyway (the service changed it): the new
    /// TextBox gets the old one's caret and selection at once, and its focus as soon as it can
    /// take it (never here, without a window).
    fn reload() -> Result<(), String> {
        let label = "a reload under the focused field";
        let failed = |hr: i32| format!("{label}: 0x{:08X}", hr as u32);
        let region = |note: &str| {
            let mut out = Out::default();
            let field = Kit::field(
                &mut out,
                InputId::ProfileAdd,
                "contoso.example",
                "",
                None,
                None,
                None,
                false,
            );
            out.finish(
                "Host",
                format!(
                    "<StackPanel>{field}{}</StackPanel>",
                    Kit::text(note, 12, "$text$", "")
                ),
            )
        };
        let inbox = Rc::new(RefCell::new(Inbox::default()));
        let binder = Binder {
            inbox: inbox.clone(),
            palette: Palette::BOTH[0],
            drag: Rc::new(RefCell::new(NavDrag::default())),
        };
        let skeleton = Xml::root(r#"<ContentControl x:Name="Host"/>"#);
        let mut renderer = Renderer::new(&skeleton, &["Host"], binder).map_err(failed)?;
        renderer.render(&[region("before")]).map_err(failed)?;
        let field = |renderer: &Renderer| renderer.fields().into_iter().next().map(|f| f.1);
        let old = field(&renderer).ok_or(format!("{label}: no field"))?;
        Xaml::select(&old, (3, 5)).map_err(failed)?;
        inbox.borrow_mut().focused = Some(Focused::new(&InputId::ProfileAdd.element(), &old));
        renderer.render(&[region("after")]).map_err(failed)?;
        let new = field(&renderer).ok_or(format!("{label}: the field is gone"))?;
        if new.raw() == old.raw() {
            return Err(format!("{label}: the region was not reloaded"));
        }
        let selection = Xaml::selection(&new);
        if selection != Ok((3, 5)) {
            return Err(format!(
                "{label}: the selection is {selection:?}, not (3, 5)"
            ));
        }
        if renderer.restoring() != Some("ProfileAdd") {
            return Err(format!("{label}: no focus waits to be given back"));
        }
        Ok(())
    }

    fn failed(label: &str, what: &str, hr: i32) -> String {
        format!(
            "{label}: {what} (0x{:08X}): {}",
            hr as u32,
            ErrorInfo::take().unwrap_or_default()
        )
    }
}
