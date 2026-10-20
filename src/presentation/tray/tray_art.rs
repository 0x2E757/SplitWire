//! The tray icon: the monochrome shield with the split mark, a
//! badge for warning, error and "service not running", one set per taskbar theme. `build/`
//! includes this file and embeds every state and size as an icon group of `SplitWire.Window.exe`.

use super::raster::{Canvas, Path, Point, Shape};

/// The taskbar an icon is drawn for. It follows the Windows theme, not the app's.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Taskbar {
    Light,
    Dark,
}

impl Taskbar {
    pub const BOTH: [Self; 2] = [Self::Light, Self::Dark];

    /// The shield's colour; the taskbar's own text colour.
    fn ink(self) -> u32 {
        match self {
            Self::Light => 0x1B1B1B,
            Self::Dark => 0xFFFFFF,
        }
    }

    fn warn(self) -> u32 {
        match self {
            Self::Light => 0xC27C0E,
            Self::Dark => 0xFCE100,
        }
    }
}

pub struct TrayArt;

impl TrayArt {
    /// The states in resource order: `TrayState`'s ids.
    pub const STATES: [&'static str; 5] = ["connected", "warning", "error", "off", "service"];
    /// The small icon at 100, 125, 150 and 200 %.
    pub const SIZES: [u32; 4] = [16, 20, 24, 32];
    const ERROR: u32 = 0xC42B1C;
    const MARK_DARK: u32 = 0x1B1B1B;
    const MARK_LIGHT: u32 = 0xFFFFFF;
    const BADGE: (Point, f64) = ((12.4, 12.4), 3.9);

    /// The icon group of `state` for `taskbar`: 101–105 light, 111–115 dark.
    pub fn resource_id(state: &str, taskbar: Taskbar) -> u16 {
        let index = Self::STATES.iter().position(|s| *s == state).unwrap_or(3) as u16;
        101 + index + if taskbar == Taskbar::Dark { 10 } else { 0 }
    }

    /// `M8 1.5l5.5 2v4.2c0 3.2-2.3 5.6-5.5 6.8C4.8 13.3 2.5 10.9 2.5 7.7V3.5Z`.
    fn shield() -> Vec<Point> {
        Path::start((8.0, 1.5))
            .line((13.5, 3.5))
            .line((13.5, 7.7))
            .cubic((13.5, 10.9), (11.2, 13.3), (8.0, 14.5))
            .cubic((4.8, 13.3), (2.5, 10.9), (2.5, 7.7))
            .line((2.5, 3.5))
            .closed()
    }

    /// The split mark: `M8 11.3V8.3M8 8.3L5.8 5.7M8 8.3l2.2-2.6`.
    fn split(width: f64) -> Shape {
        Shape::Stroke(
            vec![
                vec![(8.0, 11.3), (8.0, 8.3)],
                vec![(8.0, 8.3), (5.8, 5.7)],
                vec![(8.0, 8.3), (10.2, 5.7)],
            ],
            width,
        )
    }

    /// A badge in the corner, cut out of the shield by a ring.
    fn badge(canvas: &mut Canvas, rgb: u32) {
        let (centre, r) = Self::BADGE;
        canvas.paint(&Shape::Disc(centre, r), rgb, 1.0);
        canvas.erase(&Shape::Ring(centre, r, 1.4));
    }

    /// `state`'s icon drawn for `taskbar` at `size` pixels.
    pub fn canvas(state: &str, taskbar: Taskbar, size: u32) -> Canvas {
        let ink = taskbar.ink();
        let mut canvas = Canvas::new(size);
        let outline = Shape::Stroke(vec![Self::shield()], 1.3);
        match state {
            "connected" | "warning" => {
                canvas.paint(&Shape::Fill(Self::shield()), ink, 1.0);
                canvas.erase(&Self::split(1.5));
            }
            "error" | "off" => {
                canvas.paint(&outline, ink, 1.0);
                canvas.paint(&Self::split(1.3), ink, 1.0);
            }
            _ => canvas.paint(&outline, ink, 0.5),
        }
        match state {
            "warning" => {
                Self::badge(&mut canvas, taskbar.warn());
                let mark = vec![
                    vec![(12.4, 10.7), (12.4, 12.7)],
                    vec![(12.4, 14.1), (12.4, 14.15)],
                ];
                canvas.paint(&Shape::Stroke(mark, 1.2), Self::MARK_DARK, 1.0);
            }
            "error" => {
                Self::badge(&mut canvas, Self::ERROR);
                let cross = vec![
                    vec![(11.1, 11.1), (13.7, 13.7)],
                    vec![(13.7, 11.1), (11.1, 13.7)],
                ];
                canvas.paint(&Shape::Stroke(cross, 1.2), Self::MARK_LIGHT, 1.0);
            }
            "service" => {
                Self::badge(&mut canvas, ink);
                canvas.erase(&Shape::Stroke(vec![vec![(10.9, 12.4), (13.9, 12.4)]], 1.3));
            }
            _ => {}
        }
        canvas
    }

    /// `0xAARRGGBB` pixels, straight alpha, rows top-down.
    pub fn pixels(state: &str, taskbar: Taskbar, size: u32) -> Vec<u32> {
        Self::canvas(state, taskbar, size).argb()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(pixels: &[u32], size: u32, x: f64, y: f64) -> u32 {
        let unit = size as f64 / 16.0;
        pixels[((y * unit) as u32 * size + (x * unit) as u32) as usize]
    }

    #[test]
    fn each_state_has_its_own_group_and_look() {
        let mut ids: Vec<u16> = TrayArt::STATES
            .iter()
            .flat_map(|s| Taskbar::BOTH.map(|t| TrayArt::resource_id(s, t)))
            .collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), 10);
        assert_eq!(TrayArt::resource_id("connected", Taskbar::Light), 101);
        assert_eq!(TrayArt::resource_id("service", Taskbar::Dark), 115);
        let size = 32;
        let connected = TrayArt::pixels("connected", Taskbar::Dark, size);
        assert_eq!(
            at(&connected, size, 4.0, 5.0),
            0xFFFF_FFFF,
            "a filled shield"
        );
        assert_eq!(
            at(&connected, size, 8.0, 10.0) >> 24,
            0,
            "the split mark is cut out"
        );
        let off = TrayArt::pixels("off", Taskbar::Light, size);
        assert_eq!(at(&off, size, 5.0, 8.0) >> 24, 0, "an outline only");
        let error = TrayArt::pixels("error", Taskbar::Light, size);
        assert_eq!(at(&error, size, 12.4, 10.0), 0xFFC4_2B1C);
        let warning = TrayArt::pixels("warning", Taskbar::Dark, size);
        assert_eq!(at(&warning, size, 10.8, 12.4), 0xFFFC_E100);
        let service = TrayArt::pixels("service", Taskbar::Light, size);
        assert!(at(&service, size, 2.7, 6.0) >> 24 < 200, "a faded shield");
        for state in TrayArt::STATES {
            for size in TrayArt::SIZES {
                let light = TrayArt::pixels(state, Taskbar::Light, size);
                assert_eq!(light.len(), (size * size) as usize);
                assert_ne!(light, TrayArt::pixels(state, Taskbar::Dark, size));
            }
        }
    }
}
