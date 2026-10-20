//! A small anti-aliased rasterizer for the tray's icons and menu marks: shapes in the 16-unit box
//! of the icons' SVG paths, sampled 8 × 8 per pixel, painted over or erased. Plain std and no crate
//! paths: `build/` includes this file to embed the same pixels as icon resources.

/// A point in the 16-unit box.
pub type Point = (f64, f64);

/// What a shape covers.
pub enum Shape {
    /// The inside of a closed polygon.
    Fill(Vec<Point>),
    /// Polylines of a width, with round caps and joins (SVG `stroke-linecap="round"`).
    Stroke(Vec<Vec<Point>>, f64),
    Disc(Point, f64),
    /// A circle's outline: centre, radius, width.
    Ring(Point, f64, f64),
}

impl Shape {
    fn covers(&self, (x, y): Point) -> bool {
        match self {
            Shape::Fill(points) => Self::inside(points, (x, y)),
            Shape::Stroke(lines, width) => lines.iter().any(|line| {
                line.windows(2)
                    .any(|pair| Self::distance((x, y), pair[0], pair[1]) <= width / 2.0)
                    || (line.len() == 1 && Self::distance((x, y), line[0], line[0]) <= width / 2.0)
            }),
            Shape::Disc((cx, cy), r) => (x - cx).hypot(y - cy) <= *r,
            Shape::Ring((cx, cy), r, width) => ((x - cx).hypot(y - cy) - r).abs() <= width / 2.0,
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
            Shape::Stroke(lines, width) => grow(&mut lines.iter().flatten().copied(), width / 2.0),
            Shape::Disc(centre, r) => grow(&mut std::iter::once(*centre), *r),
            Shape::Ring(centre, r, width) => grow(&mut std::iter::once(*centre), r + width / 2.0),
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

    fn distance((x, y): Point, (ax, ay): Point, (bx, by): Point) -> f64 {
        let (dx, dy) = (bx - ax, by - ay);
        let length = dx * dx + dy * dy;
        let t = if length == 0.0 {
            0.0
        } else {
            (((x - ax) * dx + (y - ay) * dy) / length).clamp(0.0, 1.0)
        };
        (x - ax - t * dx).hypot(y - ay - t * dy)
    }
}

/// An outline of lines and cubic curves, flattened into points.
pub struct Path(Vec<Point>);

impl Path {
    const CURVE_STEPS: usize = 16;

    pub fn start(at: Point) -> Self {
        Self(vec![at])
    }

    pub fn line(mut self, to: Point) -> Self {
        self.0.push(to);
        self
    }

    pub fn cubic(mut self, c1: Point, c2: Point, to: Point) -> Self {
        let from = *self.0.last().unwrap_or(&to);
        for step in 1..=Self::CURVE_STEPS {
            let t = step as f64 / Self::CURVE_STEPS as f64;
            let u = 1.0 - t;
            let at = |a: f64, b: f64, c: f64, d: f64| {
                u * u * u * a + 3.0 * u * u * t * b + 3.0 * u * t * t * c + t * t * t * d
            };
            self.0
                .push((at(from.0, c1.0, c2.0, to.0), at(from.1, c1.1, c2.1, to.1)));
        }
        self
    }

    /// The outline closed back to its start.
    pub fn closed(mut self) -> Vec<Point> {
        if let Some(&first) = self.0.first() {
            self.0.push(first);
        }
        self.0
    }
}

/// Pixels as premultiplied red, green, blue and alpha from 0 to 1, rows top-down.
pub struct Canvas {
    size: u32,
    pixels: Vec<[f64; 4]>,
}

impl Canvas {
    const SAMPLES: u32 = 8;

    pub fn new(size: u32) -> Self {
        Self {
            size,
            pixels: vec![[0.0; 4]; (size * size) as usize],
        }
    }

    pub fn size(&self) -> u32 {
        self.size
    }

    /// Paints `shape` over the canvas in `rgb` (`0xRRGGBB`) at `opacity`.
    pub fn paint(&mut self, shape: &Shape, rgb: u32, opacity: f64) {
        let channel = |shift: u32| f64::from((rgb >> shift) & 0xFF) / 255.0;
        let colour = [channel(16), channel(8), channel(0), 1.0];
        self.each(shape, |pixel, cover| {
            let alpha = cover * opacity;
            for (value, source) in pixel.iter_mut().zip(colour) {
                *value = source * alpha + *value * (1.0 - alpha);
            }
        });
    }

    /// Clears what `shape` covers, so the taskbar shows through (a cut-out in the art).
    pub fn erase(&mut self, shape: &Shape) {
        self.each(shape, |pixel, cover| {
            for value in pixel.iter_mut() {
                *value *= 1.0 - cover;
            }
        });
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

    fn pack(a: f64, r: f64, g: f64, b: f64) -> u32 {
        let byte = |v: f64| (v.clamp(0.0, 1.0) * 255.0).round() as u32;
        (byte(a) << 24) | (byte(r) << 16) | (byte(g) << 8) | byte(b)
    }

    /// Calls `apply` with every pixel the shape touches and the share of it covered.
    fn each(&mut self, shape: &Shape, mut apply: impl FnMut(&mut [f64; 4], f64)) {
        let unit = self.size as f64 / 16.0;
        let ((x0, y0), (x1, y1)) = shape.bounds();
        let first = |v: f64| ((v * unit).floor().max(0.0) as u32).min(self.size);
        let last = |v: f64| ((v * unit).ceil().max(0.0) as u32).min(self.size);
        let total = f64::from(Self::SAMPLES * Self::SAMPLES);
        for py in first(y0)..last(y1) {
            for px in first(x0)..last(x1) {
                let mut hits = 0u32;
                for sy in 0..Self::SAMPLES {
                    for sx in 0..Self::SAMPLES {
                        let x = (f64::from(px) + (f64::from(sx) + 0.5) / f64::from(Self::SAMPLES))
                            / unit;
                        let y = (f64::from(py) + (f64::from(sy) + 0.5) / f64::from(Self::SAMPLES))
                            / unit;
                        if shape.covers((x, y)) {
                            hits += 1;
                        }
                    }
                }
                if hits > 0 {
                    let index = (py * self.size + px) as usize;
                    apply(&mut self.pixels[index], f64::from(hits) / total);
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
    fn shapes_cover_their_pixels_and_erasing_clears_them() {
        let mut canvas = Canvas::new(16);
        canvas.paint(
            &Shape::Fill(vec![(2.0, 2.0), (14.0, 2.0), (14.0, 14.0), (2.0, 14.0)]),
            0x1B1B1B,
            1.0,
        );
        assert_eq!(alpha(&canvas, 8, 8), 255);
        assert_eq!(alpha(&canvas, 0, 0), 0);
        canvas.erase(&Shape::Stroke(vec![vec![(8.0, 4.0), (8.0, 12.0)]], 2.0));
        assert_eq!(alpha(&canvas, 7, 8), 0);
        assert_eq!(alpha(&canvas, 8, 8), 0);
        assert_eq!(alpha(&canvas, 10, 8), 255);
        assert_eq!(canvas.argb()[8 * 16 + 10], 0xFF1B_1B1B);
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
    fn curves_end_where_they_are_sent() {
        let points = Path::start((0.0, 0.0))
            .cubic((0.0, 4.0), (4.0, 8.0), (8.0, 8.0))
            .line((8.0, 0.0))
            .closed();
        assert_eq!(points[16], (8.0, 8.0));
        assert_eq!(points.last(), Some(&(0.0, 0.0)));
    }
}
