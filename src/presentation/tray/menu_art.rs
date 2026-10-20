//! The marks in front of the tray menu's items, drawn as the window's kit draws them: the state
//! dot of a profile, a severity dot for an alert, the tray icon for the header.
//! Menus follow the app theme, so the marks take the window's palette.

use super::TrayLead;
use super::raster::{Canvas, Shape};
use super::tray_art::{Taskbar, TrayArt};
use crate::ipc::Severity;
use crate::presentation::Dot;

pub struct MenuArt;

impl MenuArt {
    const CENTRE: (f64, f64) = (8.0, 8.0);

    /// `(light, dark)` of the palette's colours the marks use.
    fn colour(name: &str, dark: bool) -> u32 {
        let (light, dark_colour) = match name {
            "accent" => (0x005FB8, 0x60CDFF),
            "statusDot" => (0xC27C0E, 0xFCE100),
            "danger" => (0xC42B1C, 0xFF99A4),
            "warn" => (0x8A5100, 0xFCE100),
            _ => (0x868C95, 0x7B8087),
        };
        if dark { dark_colour } else { light }
    }

    /// The mark of `lead` at `size` pixels, `None` for an item without one.
    pub fn canvas(lead: &TrayLead, dark: bool, size: u32) -> Option<Canvas> {
        let mut canvas = Canvas::new(size);
        match lead {
            TrayLead::None => return None,
            TrayLead::State(state) => {
                let taskbar = if dark { Taskbar::Dark } else { Taskbar::Light };
                return Some(TrayArt::canvas(
                    super::TrayView::art_key(*state),
                    taskbar,
                    size,
                ));
            }
            TrayLead::Dot(Dot::Off) => {
                canvas.paint(
                    &Shape::Ring(Self::CENTRE, 3.25, 1.5),
                    Self::colour("textMuted", dark),
                    1.0,
                );
            }
            TrayLead::Dot(dot) => {
                let colour = Self::colour(
                    match dot {
                        Dot::Ok => "accent",
                        Dot::Busy => "statusDot",
                        _ => "danger",
                    },
                    dark,
                );
                canvas.paint(&Shape::Disc(Self::CENTRE, 7.0), colour, 0.22);
                canvas.paint(&Shape::Disc(Self::CENTRE, 4.0), colour, 1.0);
            }
            TrayLead::Severity(severity) => {
                let colour = Self::colour(
                    match severity {
                        Severity::Error => "danger",
                        Severity::Warning => "warn",
                        Severity::Info => "accent",
                    },
                    dark,
                );
                canvas.paint(&Shape::Disc(Self::CENTRE, 4.0), colour, 1.0);
            }
        }
        Some(canvas)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ipc::TrayState;

    #[test]
    fn marks_take_the_menu_theme() {
        let middle = |lead: TrayLead, dark: bool| {
            MenuArt::canvas(&lead, dark, 16).unwrap().argb()[8 * 16 + 8]
        };
        assert_eq!(middle(TrayLead::Dot(Dot::Ok), false), 0xFF00_5FB8);
        assert_eq!(middle(TrayLead::Dot(Dot::Error), true), 0xFFFF_99A4);
        assert_eq!(middle(TrayLead::Dot(Dot::Off), false) >> 24, 0, "hollow");
        assert_eq!(
            middle(TrayLead::Severity(Severity::Warning), true),
            0xFFFC_E100
        );
        let ring = MenuArt::canvas(&TrayLead::Dot(Dot::Busy), false, 16).unwrap();
        assert!((ring.argb()[8 * 16 + 1] >> 24) < 80, "the soft ring");
        assert!(MenuArt::canvas(&TrayLead::None, false, 16).is_none());
        let header = MenuArt::canvas(&TrayLead::State(TrayState::Connected), true, 16).unwrap();
        assert_eq!(header.argb()[5 * 16 + 4], 0xFFFF_FFFF);
    }
}
