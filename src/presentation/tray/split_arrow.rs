//! The split arrow of the app and tray icons: a trunk pointing up and a branch that bends off it
//! to the right, painted at 65 % as one layer. Every size snaps its own geometry to whole pixels,
//! as the approved renders did. `build/` includes this file.

use super::raster::{Canvas, Shape};

/// The arrow in a box `unit` wide.
pub struct SplitArrow {
    pub unit: f64,
    /// The trunk's centre and the strokes' width.
    pub trunk_x: f64,
    pub width: f64,
    /// The up-head's apex and the trunk's bottom end.
    pub top: f64,
    pub bottom: f64,
    /// The up-head's base and half width.
    pub head_base: f64,
    pub head_half: f64,
    /// The branch's bend, a quarter circle from the trunk into its stroke at `branch_y`.
    pub radius: f64,
    pub branch_y: f64,
    /// The branch head's base, tip and half height.
    pub branch_base: f64,
    pub tip: f64,
    pub branch_half: f64,
}

impl SplitArrow {
    const BRANCH_OPACITY: f64 = 0.65;

    /// Paints the arrow in `rgb` on `canvas`, whose box is its pixels.
    pub fn paint(&self, canvas: &mut Canvas, rgb: u32) {
        let arrow = self.snapped(canvas.size());
        canvas.paint(&arrow.trunk(), rgb, 1.0);
        canvas.paint(&arrow.branch(), rgb, Self::BRANCH_OPACITY);
    }

    /// The arrow in pixels at `size`: the width rounds, the strokes' edges land on whole pixels
    /// and every other measure rounds, half to even as in the renders the owner approved.
    fn snapped(&self, size: u32) -> Self {
        let scale = f64::from(size) / self.unit;
        let round = |v: f64| (v * scale).round_ties_even();
        let width = round(self.width).max(1.0);
        let edge = |v: f64| (v * scale - width / 2.0).round_ties_even() + width / 2.0;
        Self {
            unit: f64::from(size),
            trunk_x: edge(self.trunk_x),
            width,
            top: round(self.top),
            bottom: round(self.bottom),
            head_base: round(self.head_base),
            head_half: round(self.head_half),
            radius: round(self.radius),
            branch_y: edge(self.branch_y),
            branch_base: round(self.branch_base),
            tip: round(self.tip),
            branch_half: round(self.branch_half),
        }
    }

    fn trunk(&self) -> Shape {
        let (x, base) = (self.trunk_x, self.head_base);
        Shape::Union(vec![
            Shape::Line((x, self.bottom), (x, base), self.width),
            Shape::Fill(vec![
                (x + self.head_half, base),
                (x, self.top),
                (x - self.head_half, base),
            ]),
        ])
    }

    fn branch(&self) -> Shape {
        let (y, bend) = (self.branch_y, self.trunk_x + self.radius);
        Shape::Union(vec![
            Shape::Arc(
                (bend, y + self.radius),
                self.radius,
                self.width,
                (180.0, 270.0),
            ),
            Shape::Line((bend, y), (self.branch_base, y), self.width),
            Shape::Fill(vec![
                (self.branch_base, y + self.branch_half),
                (self.tip, y),
                (self.branch_base, y - self.branch_half),
            ]),
        ])
    }
}
