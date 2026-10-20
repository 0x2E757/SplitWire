//! The tray icon: the split arrow, whose colour alone tells the state, one set per taskbar theme.
//! `build/` includes this file and embeds every state and size as an icon group of
//! `SplitWire.Window.exe`.

use super::raster::Canvas;
use super::split_arrow::SplitArrow;

/// The taskbar an icon is drawn for. It follows the Windows theme, not the app's.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Taskbar {
    Light,
    Dark,
}

impl Taskbar {
    pub const BOTH: [Self; 2] = [Self::Light, Self::Dark];

    /// The arrow's colour in `state`; ink is the taskbar's own text colour.
    fn ink(self, state: &str) -> u32 {
        let (light, dark) = match state {
            "connected" => (0x1B1B1B, 0xFFFFFF),
            "warning" => (0xC27C0E, 0xFCE100),
            "error" => (0xC42B1C, 0xFF6B5E),
            _ => (0x8A8A8A, 0x8A8A8A),
        };
        if self == Self::Light { light } else { dark }
    }
}

pub struct TrayArt;

impl TrayArt {
    /// The states in resource order: `TrayState`'s ids.
    pub const STATES: [&'static str; 4] = ["connected", "warning", "error", "off"];
    /// The small icon at 100, 125, 150 and 200 %.
    pub const SIZES: [u32; 4] = [16, 20, 24, 32];
    const ARROW: SplitArrow = SplitArrow {
        unit: 16.0,
        trunk_x: 5.0,
        width: 2.0,
        top: 1.0,
        bottom: 15.0,
        head_base: 4.75,
        head_half: 3.5,
        radius: 5.0,
        branch_y: 8.0,
        branch_base: 11.0,
        tip: 14.0,
        branch_half: 3.5,
    };

    /// The icon group of `state` for `taskbar`: 101–104 light, 111–114 dark.
    pub fn resource_id(state: &str, taskbar: Taskbar) -> u16 {
        let index = Self::STATES.iter().position(|s| *s == state).unwrap_or(3) as u16;
        101 + index + if taskbar == Taskbar::Dark { 10 } else { 0 }
    }

    /// `state`'s icon drawn for `taskbar` at `size` pixels.
    pub fn canvas(state: &str, taskbar: Taskbar, size: u32) -> Canvas {
        let mut canvas = Canvas::with_box(size, f64::from(size));
        Self::ARROW.paint(&mut canvas, taskbar.ink(state));
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

    #[test]
    fn each_state_has_its_own_group_and_colour() {
        let mut ids: Vec<u16> = TrayArt::STATES
            .iter()
            .flat_map(|s| Taskbar::BOTH.map(|t| TrayArt::resource_id(s, t)))
            .collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), 8);
        assert_eq!(TrayArt::resource_id("connected", Taskbar::Light), 101);
        assert_eq!(TrayArt::resource_id("off", Taskbar::Dark), 114);
        let trunk =
            |state: &str, taskbar: Taskbar| TrayArt::pixels(state, taskbar, 16)[10 * 16 + 4];
        assert_eq!(trunk("connected", Taskbar::Dark), 0xFFFF_FFFF);
        assert_eq!(trunk("warning", Taskbar::Light), 0xFFC2_7C0E);
        assert_eq!(trunk("error", Taskbar::Dark), 0xFFFF_6B5E);
        assert_eq!(trunk("off", Taskbar::Light), trunk("off", Taskbar::Dark));
        let branch = TrayArt::pixels("connected", Taskbar::Light, 16)[7 * 16 + 11];
        assert_eq!(branch, 0xA61B_1B1B, "the branch at 65 %");
    }
}
