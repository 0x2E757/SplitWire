//! `--render-pages <folder>`: a PNG of every page and state of every demo scenario, in both themes
//! and both width classes, and of every profile menu in both themes, laid out and rendered
//! without a window, for reviewing the look.

use std::cell::RefCell;
use std::path::Path;
use std::rc::Rc;

use super::error_info::ErrorInfo;
use super::hidden_host::HiddenHost;
use super::island::XamlRuntime;
use super::markup::{Frame, Palette, Region};
use super::menu_previews::MenuPreviews;
use super::nav_drag::NavDrag;
use super::png::Png;
use super::renderer::{Binder, Inbox, Renderer};
use super::snapshot::Snapshot;
use crate::platform::com::Apartment;
use crate::presentation::WidthClass;
use crate::presentation::showcase::Showcase;
use crate::presentation::transport::demo::Scenario;

pub struct PagePictures;

impl PagePictures {
    /// The client size a width class is shown at.
    pub fn size(width: WidthClass) -> (f64, f64) {
        match width {
            WidthClass::Full => (1280.0, 1000.0),
            WidthClass::Compact => (960.0, 1000.0),
        }
    }

    /// Writes the pictures into `folder`; how many.
    pub fn run(folder: &Path) -> Result<usize, String> {
        let _apartment =
            Apartment::single_threaded().map_err(|hr| format!("COM: 0x{:08X}", hr as u32))?;
        let _xaml = XamlRuntime::start().map_err(|hr| format!("XAML: 0x{:08X}", hr as u32))?;
        std::fs::create_dir_all(folder).map_err(|e| format!("{}: {e}", folder.display()))?;
        let failed = |what: &str, hr: i32| {
            format!(
                "{what} (0x{:08X}): {}",
                hr as u32,
                ErrorInfo::take().unwrap_or_default()
            )
        };
        let host = HiddenHost::new().map_err(|hr| failed("the hidden island", hr))?;
        let mut written = 0;
        for scenario in Scenario::ALL {
            let views = Showcase::views(scenario);
            for palette in Palette::BOTH {
                for width in WidthClass::ALL {
                    let theme = if palette.dark { "dark" } else { "light" };
                    let (w, h) = Self::size(width);
                    for (view_label, view) in &views {
                        // A fresh frame per state: nothing of the state before stays mounted.
                        let mut renderer = Renderer::new(
                            &Frame::skeleton(width),
                            &Frame::HOSTS,
                            Self::binder(palette),
                        )
                        .map_err(|hr| failed("the frame does not load", hr))?;
                        let label = format!("{view_label} {theme} {width:?}");
                        let name = format!(
                            "{}-{theme}-{}.png",
                            view_label.replace(':', "-"),
                            if width.compact() { "compact" } else { "full" }
                        );
                        Self::write(
                            &host,
                            &mut renderer,
                            &Frame::regions(view, width),
                            (w, h),
                            &folder.join(name),
                        )
                        .map_err(|e| format!("{label}: {e}"))?;
                        written += 1;
                    }
                }
            }
        }
        for palette in Palette::BOTH {
            let theme = if palette.dark { "dark" } else { "light" };
            for (menu_label, region) in MenuPreviews::all() {
                let label = format!("{menu_label} {theme}");
                let mut renderer = Renderer::new(
                    &MenuPreviews::skeleton(),
                    &[MenuPreviews::HOST],
                    Self::binder(palette),
                )
                .map_err(|hr| failed("the menu does not load", hr))?;
                let (w, h) = MenuPreviews::SIZE;
                Self::write(
                    &host,
                    &mut renderer,
                    &[region],
                    (w, h),
                    &folder.join(format!("{}-{theme}.png", menu_label.replace(':', "-"))),
                )
                .map_err(|e| format!("{label}: {e}"))?;
                written += 1;
            }
        }
        Ok(written)
    }

    fn binder(palette: Palette) -> Binder {
        Binder {
            inbox: Rc::new(RefCell::new(Inbox::default())),
            palette,
            drag: Rc::new(RefCell::new(NavDrag::default())),
        }
    }

    /// Mounts `regions`, lays them out at `size` and writes their picture to `path`.
    fn write(
        host: &HiddenHost,
        renderer: &mut Renderer,
        regions: &[Region],
        (w, h): (f64, f64),
        path: &Path,
    ) -> Result<(), String> {
        let failed = |hr: i32| {
            format!(
                "0x{:08X}: {}",
                hr as u32,
                ErrorInfo::take().unwrap_or_default()
            )
        };
        renderer.render(regions).map_err(failed)?;
        host.show(renderer.root(), w, h).map_err(failed)?;
        let picture = Snapshot::of(renderer.root()).map_err(failed)?;
        std::fs::write(
            path,
            Png::encode(picture.width, picture.height, &picture.bgra),
        )
        .map_err(|e| e.to_string())
    }
}
