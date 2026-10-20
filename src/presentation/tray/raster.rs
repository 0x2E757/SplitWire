//! A small anti-aliased rasterizer for the app and tray icons and the menu marks: shapes in a box
//! of units (16 for the menu marks, the icon's own pixels for art snapped per size), sampled
//! 16 × 16 per pixel and painted over. Plain std and no crate paths: `build/` includes this file to
//! embed the same pixels as icon resources.

/// A point in the canvas's box.
pub type Point = (f64, f64);

/// What a shape covers.
pub enum Shape {
    /// The inside of a closed polygon.
    Fill(Vec<Point>),
    /// A straight stroke between two points of a width, with butt caps (SVG's default).
    Line(Point, Point, f64),
    /// A stroked arc with butt ends: centre, radius, width and the angles it runs between, in
    /// degrees clockwise from the right, as y grows downwards.
    Arc(Point, f64, f64, (f64, f64)),
    Disc(Point, f64),
    /// A circle's outline: centre, radius, width.
    Ring(Point, f64, f64),
    /// A rectangle from its top-left to its bottom-right corner, rounded by a radius (SVG `rx`).
    Rounded(Point, Point, f64),
    /// Shapes painted as one layer: where they overlap, the opacity does not add up
    /// (SVG `<g opacity>`).
    Union(Vec<Shape>),
}

impl Shape {
    fn covers(&self, (x, y): Point) -> bool {
        match self {
            Shape::Fill(points) => Self::inside(points, (x, y)),
            Shape::Line(a, b, width) => {
                Self::beside((x, y), *a, *b).is_some_and(|d| d <= width / 2.0)
            }
            Shape::Arc((cx, cy), r, width, (from, to)) => {
                let angle = (y - cy).atan2(x - cx).to_degrees().rem_euclid(360.0);
                ((x - cx).hypot(y - cy) - r).abs() <= width / 2.0 && (*from..=*to).contains(&angle)
            }
            Shape::Disc((cx, cy), r) => (x - cx).hypot(y - cy) <= *r,
            Shape::Ring((cx, cy), r, width) => ((x - cx).hypot(y - cy) - r).abs() <= width / 2.0,
            Shape::Rounded((x0, y0), (x1, y1), r) => {
                let dx = (x0 + r - x).max(x - x1 + r).max(0.0);
                let dy = (y0 + r - y).max(y - y1 + r).max(0.0);
                dx.hypot(dy) <= *r
            }
            Shape::Union(shapes) => shapes.iter().any(|shape| shape.covers((x, y))),
        }
    }

    /// The box the shape stays in, so pixels outside it are not sampled.
    fn bounds(&self) -> (Point, Point) {
        let grow = |points: &mut dyn Iterator<Item = Point>, by: f64| {
            points.fold(
                ((f64::MAX, f64::MAX), (f64::MIN, f64::MIN)),
                |((x0, y0), (x1, y1)), (x, y)| {
                    (
                        (x0.min(x - by), y0.min(y - by)),
                        (x1.max(x + by), y1.max(y + by)),
                    )
                },
            )
        };
        match self {
            Shape::Fill(points) => grow(&mut points.iter().copied(), 0.0),
            Shape::Line(a, b, width) => grow(&mut [*a, *b].into_iter(), width / 2.0),
            Shape::Arc(centre, r, width, _) | Shape::Ring(centre, r, width) => {
                grow(&mut std::iter::once(*centre), r + width / 2.0)
            }
            Shape::Disc(centre, r) => grow(&mut std::iter::once(*centre), *r),
            Shape::Rounded(a, b, _) => grow(&mut [*a, *b].into_iter(), 0.0),
            Shape::Union(shapes) => grow(
                &mut shapes.iter().flat_map(|shape| {
                    let (a, b) = shape.bounds();
                    [a, b]
                }),
                0.0,
            ),
        }
    }

    /// Even-odd crossings of a ray to the right.
    fn inside(points: &[Point], (x, y): Point) -> bool {
        let mut inside = false;
        for (i, &(x1, y1)) in points.iter().enumerate() {
            let (x2, y2) = points[(i + 1) % points.len()];
            if (y1 > y) != (y2 > y) && x < x1 + (y - y1) * (x2 - x1) / (y2 - y1) {
                inside = !inside;
            }
        }
        inside
    }

    /// How far `(x, y)` is from the line through `a` and `b`, when it lies between their ends.
    fn beside((x, y): Point, (ax, ay): Point, (bx, by): Point) -> Option<f64> {
        let (dx, dy) = (bx - ax, by - ay);
        let t = ((x - ax) * dx + (y - ay) * dy) / (dx * dx + dy * dy);
        (0.0..=1.0)
            .contains(&t)
            .then(|| (x - ax - t * dx).hypot(y - ay - t * dy))
    }
}

/// A linear gradient from one colour (`0xRRGGBB`) at a point to another at a second point
/// (SVG `linearGradient`).
pub struct Gradient {
    pub from: (Point, u32),
    pub to: (Point, u32),
}

impl Gradient {
    /// The colour at `(x, y)`, mixed by where it falls between the two points.
    fn colour(&self, (x, y): Point) -> [f64; 4] {
        let (((ax, ay), first), ((bx, by), last)) = (self.from, self.to);
        let (dx, dy) = (bx - ax, by - ay);
        let t = (((x - ax) * dx + (y - ay) * dy) / (dx * dx + dy * dy)).clamp(0.0, 1.0);
        let (first, last) = (Canvas::colour(first), Canvas::colour(last));
        [0, 1, 2, 3].map(|i| first[i] + (last[i] - first[i]) * t)
    }
}

/// Pixels as premultiplied red, green, blue and alpha from 0 to 1, rows top-down.
pub struct Canvas {
    size: u32,
    /// How wide the box of the shapes is.
    units: f64,
    pixels: Vec<[f64; 4]>,
}

impl Canvas {
    const SAMPLES: u32 = 16;

    /// A canvas of `size` pixels for shapes in the 16-unit box.
    pub fn new(size: u32) -> Self {
        Self::with_box(size, 16.0)
    }

    /// A canvas of `size` pixels for shapes in a box `units` wide; art snapped to whole pixels
    /// passes `size`.
    pub fn with_box(size: u32, units: f64) -> Self {
        Self {
            size,
            units,
            pixels: vec![[0.0; 4]; (size * size) as usize],
        }
    }

    pub fn size(&self) -> u32 {
        self.size
    }

    /// Paints `shape` over the canvas in `rgb` (`0xRRGGBB`) at `opacity`.
    pub fn paint(&mut self, shape: &Shape, rgb: u32, opacity: f64) {
        let colour = Self::colour(rgb);
        self.cover(shape, opacity, |_| colour);
    }

    /// Paints `shape` over the canvas in `gradient`, opaque.
    pub fn paint_gradient(&mut self, shape: &Shape, gradient: &Gradient) {
        self.cover(shape, 1.0, |at| gradient.colour(at));
    }

    /// `0xAARRGGBB` with straight alpha: an icon resource's colours.
    pub fn argb(&self) -> Vec<u32> {
        self.pixels
            .iter()
            .map(|[r, g, b, a]| {
                let straight = |v: f64| if *a > 0.0 { v / a } else { 0.0 };
                Self::pack(*a, straight(*r), straight(*g), straight(*b))
            })
            .collect()
    }

    /// `0xAARRGGBB` with premultiplied alpha: a 32-bit menu bitmap's colours.
    pub fn premultiplied(&self) -> Vec<u32> {
        self.pixels
            .iter()
            .map(|[r, g, b, a]| Self::pack(*a, *r, *g, *b))
            .collect()
    }

    fn colour(rgb: u32) -> [f64; 4] {
        let channel = |shift: u32| f64::from((rgb >> shift) & 0xFF) / 255.0;
        [channel(16), channel(8), channel(0), 1.0]
    }

    fn pack(a: f64, r: f64, g: f64, b: f64) -> u32 {
        let byte = |v: f64| (v.clamp(0.0, 1.0) * 255.0).round() as u32;
        (byte(a) << 24) | (byte(r) << 16) | (byte(g) << 8) | byte(b)
    }

    /// Blends `colour` at each pixel's centre over what `shape` covers of it.
    fn cover(&mut self, shape: &Shape, opacity: f64, colour: impl Fn(Point) -> [f64; 4]) {
        self.each(shape, |pixel, cover, centre| {
            let alpha = cover * opacity;
            for (value, source) in pixel.iter_mut().zip(colour(centre)) {
                *value = source * alpha + *value * (1.0 - alpha);
            }
        });
    }

    /// Calls `apply` with every pixel the shape touches, the share of it covered and its centre.
    fn each(&mut self, shape: &Shape, mut apply: impl FnMut(&mut [f64; 4], f64, Point)) {
        let unit = f64::from(self.size) / self.units;
        let ((x0, y0), (x1, y1)) = shape.bounds();
        let first = |v: f64| ((v * unit).floor().max(0.0) as u32).min(self.size);
        let last = |v: f64| ((v * unit).ceil().max(0.0) as u32).min(self.size);
        let samples = f64::from(Self::SAMPLES);
        let at = |pixel: u32, sample: f64| (f64::from(pixel) + sample / samples) / unit;
        for py in first(y0)..last(y1) {
            for px in first(x0)..last(x1) {
                let mut hits = 0u32;
                for sy in 0..Self::SAMPLES {
                    for sx in 0..Self::SAMPLES {
                        let x = at(px, f64::from(sx) + 0.5);
                        let y = at(py, f64::from(sy) + 0.5);
                        if shape.covers((x, y)) {
                            hits += 1;
                        }
                    }
                }
                if hits > 0 {
                    let index = (py * self.size + px) as usize;
                    let centre = (at(px, samples / 2.0), at(py, samples / 2.0));
                    apply(
                        &mut self.pixels[index],
                        f64::from(hits) / (samples * samples),
                        centre,
                    );
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn alpha(canvas: &Canvas, x: u32, y: u32) -> u32 {
        canvas.argb()[(y * canvas.size() + x) as usize] >> 24
    }

    #[test]
    fn lines_and_arcs_end_square() {
        let mut canvas = Canvas::new(16);
        canvas.paint(&Shape::Line((8.0, 4.0), (8.0, 12.0), 2.0), 0x1B1B1B, 1.0);
        assert_eq!(alpha(&canvas, 7, 4), 255);
        assert_eq!(alpha(&canvas, 7, 3), 0, "no round cap");
        assert_eq!(alpha(&canvas, 8, 12), 0);
        assert_eq!(alpha(&canvas, 6, 8), 0);
        assert_eq!(canvas.argb()[8 * 16 + 8], 0xFF1B_1B1B);
        let mut canvas = Canvas::new(16);
        canvas.paint(
            &Shape::Arc((8.0, 8.0), 5.0, 2.0, (180.0, 270.0)),
            0x1B1B1B,
            1.0,
        );
        assert_eq!(alpha(&canvas, 3, 7), 255, "the left end");
        assert_eq!(alpha(&canvas, 3, 8), 0, "cut square below it");
        assert_eq!(alpha(&canvas, 7, 3), 255, "the top end");
        assert_eq!(alpha(&canvas, 8, 3), 0, "cut square right of it");
    }

    #[test]
    fn a_union_is_one_layer() {
        let square =
            |x: f64| Shape::Fill(vec![(x, 2.0), (x + 8.0, 2.0), (x + 8.0, 10.0), (x, 10.0)]);
        let mut canvas = Canvas::new(16);
        canvas.paint(&Shape::Union(vec![square(2.0), square(6.0)]), 0xFFFFFF, 0.5);
        assert_eq!(alpha(&canvas, 3, 4), 128);
        assert_eq!(alpha(&canvas, 8, 4), 128, "the overlap");
    }

    #[test]
    fn edges_are_smoothed_and_colours_stay_straight() {
        let mut canvas = Canvas::new(32);
        canvas.paint(&Shape::Disc((8.0, 8.0), 5.0), 0xC42B1C, 1.0);
        let edge = alpha(&canvas, 23, 23);
        assert!(edge > 0 && edge < 255, "{edge}");
        let pixel = canvas.argb()[23 * 32 + 23];
        assert_eq!(pixel & 0x00FF_FFFF, 0xC42B1C);
        let premultiplied = canvas.premultiplied()[23 * 32 + 23];
        assert!((premultiplied >> 16) & 0xFF < 0xC4);
        canvas.paint(&Shape::Ring((8.0, 8.0), 5.0, 1.0), 0xFFFFFF, 0.5);
        assert_eq!(alpha(&canvas, 16, 16), 255);
    }

    #[test]
    fn rounded_corners_and_gradients() {
        let mut canvas = Canvas::with_box(16, 16.0);
        let gradient = Gradient {
            from: ((0.0, 0.0), 0x000000),
            to: ((16.0, 16.0), 0xFFFFFF),
        };
        canvas.paint_gradient(&Shape::Rounded((0.0, 0.0), (16.0, 16.0), 4.0), &gradient);
        let pixels = canvas.argb();
        assert_eq!(pixels[0] >> 24, 0, "the corner is round");
        assert_eq!(pixels[8 * 16] >> 24, 255);
        assert_eq!(pixels[8 * 16 + 7], 0xFF80_8080, "the middle is half way");
        assert!(pixels[15 * 16 + 8] & 0xFF > pixels[8 * 16] & 0xFF);
    }
}
