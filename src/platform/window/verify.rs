//! `--verify-pages`: every page of every demo scenario, in both themes and
//! both width classes, loaded through XamlReader and mounted with its handlers, without a window.
//! Each region's bound elements and fields must exist, and every field must read back the text the
//! session gave it, line breaks included; then every field is typed into ([`TypingCheck`]). Then
//! the tray: every icon loaded from the executable's resources, every scenario's menu built in both
//! themes, nothing shown.

use std::cell::RefCell;
use std::rc::Rc;

use super::alignment::Alignment;
use super::error_info::ErrorInfo;
use super::hidden_host::HiddenHost;
use super::island::XamlRuntime;
use super::markup::{Frame, Kit, Look, Out, Palette, Region, Xml};
use super::menu_previews::MenuPreviews;
use super::nav_drag::NavDrag;
use super::pictures::PagePictures;
use super::renderer::{Binder, Inbox, Renderer};
use super::tray::{PopupMenu, Tray, TrayIcons};
use super::typing::{Typed, TypingCheck};
use super::xaml::Xaml;
use crate::platform::com::Apartment;
use crate::presentation::showcase::{Driver, Showcase};
use crate::presentation::transport::demo::Scenario;
use crate::presentation::tray::{TrayItem, TrayLead};
use crate::presentation::{Command, InputId, WidthClass};

#[derive(Default)]
pub struct Totals {
    pub states: usize,
    /// Rows whose alignment was checked.
    pub rows: usize,
    /// Headers whose click target was checked to cover them.
    pub covers: usize,
    /// Profile menus laid out and checked.
    pub menus: usize,
    pub regions: usize,
    pub elements: usize,
    pub tray_icons: usize,
    pub tray_menus: usize,
    pub tray_items: usize,
    pub typed: Typed,
}

pub struct PageVerifier;

impl PageVerifier {
    /// Loads one region and finds every name it binds; the error names the region and why.
    fn region(
        region: &Region,
        palette: Palette,
        label: &str,
        totals: &mut Totals,
    ) -> Result<(), String> {
        let markup = palette.apply(&region.loadable());
        let root = Xaml::load(&markup).map_err(|hr| {
            format!(
                "{label}: {} does not load (0x{:08X}): {}",
                region.host,
                hr as u32,
                ErrorInfo::take().unwrap_or_default()
            )
        })?;
        for name in region.names() {
            Xaml::find(&root, &name)
                .map_err(|_| format!("{label}: {} has no element {name}", region.host))?;
            totals.elements += 1;
        }
        totals.regions += 1;
        for child in &region.children {
            Self::region(child, palette, label, totals)?;
        }
        Ok(())
    }

    pub fn run() -> Result<Totals, String> {
        let _apartment =
            Apartment::single_threaded().map_err(|hr| format!("COM: 0x{:08X}", hr as u32))?;
        let _xaml = XamlRuntime::start().map_err(|hr| format!("XAML: 0x{:08X}", hr as u32))?;
        let mut totals = Totals::default();
        let host =
            HiddenHost::new().map_err(|hr| format!("the hidden island: 0x{:08X}", hr as u32))?;
        let mut alignment =
            Alignment::new().map_err(|hr| format!("the alignment check: 0x{:08X}", hr as u32))?;
        // Each problem once, with the first state that shows it and how many do.
        let mut misaligned: Vec<(String, String, usize)> = Vec::new();
        for scenario in Scenario::ALL {
            let views = Showcase::views(scenario);
            for palette in Palette::BOTH {
                for width in WidthClass::ALL {
                    let theme = if palette.dark { "dark" } else { "light" };
                    let mut renderer = Renderer::new(
                        &Frame::skeleton(width),
                        &Frame::HOSTS,
                        Self::binder(palette),
                    )
                    .map_err(|hr| {
                        format!(
                            "the frame does not load (0x{:08X}): {}",
                            hr as u32,
                            ErrorInfo::take().unwrap_or_default()
                        )
                    })?;
                    for (label, view) in &views {
                        let label = format!("{label} {theme} {width:?}");
                        let regions = Frame::regions(view, width);
                        for region in &regions {
                            Self::region(region, palette, &label, &mut totals)?;
                        }
                        renderer.render(&regions).map_err(|hr| {
                            format!(
                                "{label}: mounting failed (0x{:08X}): {}",
                                hr as u32,
                                ErrorInfo::take().unwrap_or_default()
                            )
                        })?;
                        Self::unchanged(&mut renderer, &label)?;
                        let (w, h) = PagePictures::size(width);
                        host.show(renderer.root(), w, h)
                            .and_then(|()| alignment.check(renderer.root()))
                            .map_err(|hr| {
                                format!("{label}: laying out failed (0x{:08X})", hr as u32)
                            })?;
                        for problem in alignment.problems.drain(..) {
                            match misaligned.iter_mut().find(|(p, _, _)| *p == problem) {
                                Some((_, _, count)) => *count += 1,
                                None => misaligned.push((problem, label.clone(), 1)),
                            }
                        }
                        totals.states += 1;
                    }
                }
            }
        }
        for palette in Palette::BOTH {
            let theme = if palette.dark { "dark" } else { "light" };
            for (label, region) in MenuPreviews::all() {
                let label = format!("{label} {theme}");
                Self::region(&region, palette, &label, &mut totals)?;
                let failed = |hr: i32| format!("{label}: 0x{:08X}", hr as u32);
                let mut renderer = Renderer::new(
                    &MenuPreviews::skeleton(),
                    &[MenuPreviews::HOST],
                    Self::binder(palette),
                )
                .map_err(failed)?;
                renderer.render(&[region]).map_err(failed)?;
                let (w, h) = MenuPreviews::SIZE;
                host.show(renderer.root(), w, h)
                    .and_then(|()| alignment.check(renderer.root()))
                    .map_err(failed)?;
                for problem in alignment.problems.drain(..) {
                    misaligned.push((problem, label.clone(), 1));
                }
                totals.menus += 1;
            }
        }
        totals.rows = alignment.rows;
        totals.covers = alignment.covers;
        if alignment.covers == 0 {
            return Err("no header was checked to be clickable all over".into());
        }
        if !misaligned.is_empty() {
            return Err(misaligned
                .iter()
                .map(|(problem, label, count)| format!("{label}: {problem} (in {count} states)"))
                .collect::<Vec<_>>()
                .join(
                    "
",
                ));
        }
        drop(host);
        Self::line_breaks()?;
        Self::current_commands()?;
        totals.typed = TypingCheck::run()?;
        Self::tray(&mut totals)?;
        Ok(totals)
    }

    fn binder(palette: Palette) -> Binder {
        Binder {
            inbox: Rc::new(RefCell::new(Inbox::default())),
            palette,
            drag: Rc::new(RefCell::new(NavDrag::default())),
        }
    }

    /// Fields just mounted or reset read back what the session holds: polling them finds nothing.
    fn unchanged(renderer: &mut Renderer, label: &str) -> Result<(), String> {
        match renderer.poll_inputs().into_iter().next() {
            Some(Command::Input(id, text)) => Err(format!(
                "{label}: the field {} reads back {text:?}, not the session's text",
                id.element()
            )),
            Some(other) => Err(format!("{label}: polling the fields raised {other:?}")),
            None => Ok(()),
        }
    }

    /// Multi-line text in a mounted editor, put there by the markup and then by resets.
    fn line_breaks() -> Result<(), String> {
        let label = "the editor's line breaks";
        let failed = |hr: i32| format!("{label}: 0x{:08X}", hr as u32);
        let mut out = Out::default();
        let editor = Kit::editor(
            &mut out,
            InputId::ListDialog,
            "contoso.example\nshop-contoso.example\n",
            3,
            &[],
            None,
        );
        let region = out.finish("Host", editor);
        let skeleton = Xml::root(r#"<ContentControl x:Name="Host"/>"#);
        let mut renderer =
            Renderer::new(&skeleton, &["Host"], Self::binder(Palette::BOTH[0])).map_err(failed)?;
        renderer.render(&[region]).map_err(failed)?;
        Self::unchanged(&mut renderer, label)?;
        for text in ["a\n\nb\n", "\none", ""] {
            renderer.reset(&[(InputId::ListDialog, text.to_owned())]);
            Self::unchanged(&mut renderer, &format!("{label}, reset to {text:?}"))?;
        }
        Ok(())
    }

    /// A region whose markup stays raises the commands of the latest frame: the Logs header's
    /// Copy copies the lines shown now, not those of the frame that loaded it.
    fn current_commands() -> Result<(), String> {
        let label = "a region kept as it was";
        let failed = |hr: i32| format!("{label}: 0x{:08X}", hr as u32);
        let region = |text: &str| {
            let mut out = Out::default();
            let copy = Kit::button(
                &mut out,
                "Copy",
                Some("copy"),
                Look::Default,
                false,
                true,
                Command::Copy(text.into()),
                "",
            );
            out.finish("Host", copy)
        };
        let skeleton = Xml::root(r#"<ContentControl x:Name="Host"/>"#);
        let mut renderer =
            Renderer::new(&skeleton, &["Host"], Self::binder(Palette::BOTH[0])).map_err(failed)?;
        let (old, new) = (region("old lines"), region("new lines"));
        if old.markup != new.markup {
            return Err(format!("{label}: the command changed the markup"));
        }
        renderer.render(&[old]).map_err(failed)?;
        renderer.render(&[new]).map_err(failed)?;
        match renderer.commands("Host") {
            Some(commands) if commands == [Command::Copy("new lines".into())] => Ok(()),
            other => Err(format!("{label} raises {other:?}, not the latest Copy")),
        }
    }

    /// The tray's icons and menus, built and destroyed without being shown. Every profile is one
    /// plain item of the menu itself, which switches it with one click.
    fn tray(totals: &mut Totals) -> Result<(), String> {
        totals.tray_icons = TrayIcons::verify()?;
        for scenario in Scenario::ALL {
            let driver = Driver::new(scenario);
            let view = driver.session.tray();
            let profiles = driver.session.view().menus.len();
            let plain = view
                .menu
                .iter()
                .filter(|item| {
                    matches!(item, TrayItem::Entry(entry) if matches!(entry.lead, TrayLead::Dot(_)))
                })
                .count();
            let nested = view.menu.iter().any(|item| {
                matches!(item, TrayItem::Submenu(entry, _) if matches!(entry.lead, TrayLead::Dot(_)))
            });
            if plain != profiles || nested {
                return Err(format!(
                    "{}: the tray menu has {plain} plain items for {profiles} profiles",
                    scenario.key()
                ));
            }
            for dark in [false, true] {
                let menu = PopupMenu::build(&view.menu, dark, Tray::mark_size())
                    .map_err(|e| format!("{}: the tray menu: {e}", scenario.key()))?;
                if menu.top_level() != view.menu.len() {
                    return Err(format!(
                        "{}: the tray menu has {} items, not {}",
                        scenario.key(),
                        menu.top_level(),
                        view.menu.len()
                    ));
                }
                totals.tray_menus += 1;
                totals.tray_items += menu.items();
            }
        }
        Ok(())
    }
}
