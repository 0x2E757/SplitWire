//! The app icon: the tray's split arrow in white on a tile that runs from orange to pink. `build/`
//! includes this file and embeds it as the first icon group of both executables, so Explorer, the
//! taskbar and the window's title bar show it.

use super::raster::{Canvas, Gradient, Shape};
use super::split_arrow::SplitArrow;

pub struct AppArt;

impl AppArt {
    /// The lowest icon group id: Explorer shows the first group as the file's icon.
    pub const RESOURCE_ID: u16 = 1;
    /// The shell's 16 to 48 px at 100 to 150 %, and Explorer's large views.
    pub const SIZES: [u32; 12] = [16, 20, 24, 30, 32, 36, 40, 48, 64, 96, 128, 256];
    const UNIT: f64 = 256.0;
    /// The tile's corner radius in the 256-unit box.
    const CORNER: f64 = 56.0;
    const TOP_LEFT: u32 = 0xFFA22B;
    const BOTTOM_RIGHT: u32 = 0xE5246F;
    const ARROW: SplitArrow = SplitArrow {
        unit: Self::UNIT,
        trunk_x: 86.0,
        width: 21.0,
        top: 44.0,
        bottom: 212.0,
        head_base: 88.62,
        head_half: 42.0,
        radius: 73.5,
        branch_y: 112.25,
        branch_base: 167.38,
        tip: 212.0,
        branch_half: 42.0,
    };

    /// The icon drawn at `size` pixels.
    pub fn canvas(size: u32) -> Canvas {
        let side = f64::from(size);
        let mut canvas = Canvas::with_box(size, side);
        let tile = Shape::Rounded((0.0, 0.0), (side, side), Self::CORNER * side / Self::UNIT);
        let gradient = Gradient {
            from: ((0.0, 0.0), Self::TOP_LEFT),
            to: ((side, side), Self::BOTTOM_RIGHT),
        };
        canvas.paint_gradient(&tile, &gradient);
        Self::ARROW.paint(&mut canvas, 0xFFFFFF);
        canvas
    }

    /// `0xAARRGGBB` pixels, straight alpha, rows top-down.
    pub fn pixels(size: u32) -> Vec<u32> {
        Self::canvas(size).argb()
    }
}
