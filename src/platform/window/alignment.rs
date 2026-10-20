//! The alignment check of `--verify-pages`: in a laid-out page, every row marked by
//! [`Align`](super::markup::Align) must have its marks on one line, within a pixel, and that line
//! in the middle of the row. A text's line is the middle of its capitals, as the eye reads it next
//! to an icon or a dot; texts of different sizes in a baseline row share their baseline instead.

use super::layout::{Layout, Rect};
use crate::platform::com::{Com, ComResult};

/// One mark of a row: what it shows, where its middle and, for a text, its baseline lie, and the
/// box it covers. A frame (a Border around content: a pill, a badge) only widens a block.
struct Mark {
    what: String,
    middle: f64,
    baseline: Option<f64>,
    top: f64,
    bottom: f64,
    frame: bool,
}

impl Mark {
    fn boxed(what: String, bounds: Rect) -> Self {
        Self {
            what,
            middle: bounds.middle(),
            baseline: None,
            top: bounds.top(),
            bottom: bounds.top() + f64::from(bounds.height),
            frame: false,
        }
    }
}

pub struct Alignment {
    layout: Layout,
    pub rows: usize,
    /// Click areas checked to cover their area.
    pub covers: usize,
    pub problems: Vec<String>,
}

impl Alignment {
    /// How far apart two lines of a row may be, in DIP.
    const TOLERANCE: f64 = 1.0;
    /// Segoe UI Variable's capitals, in em.
    const CAPITALS: f64 = 0.7;
    /// Where a TextBox's line has its baseline, in em from the top of its text view.
    const TEXT_BOX_BASELINE: f64 = 1.079;

    pub fn new() -> ComResult<Self> {
        Ok(Self {
            layout: Layout::new()?,
            rows: 0,
            covers: 0,
            problems: Vec::new(),
        })
    }

    /// Checks every marked row under `root`.
    pub fn check(&mut self, root: &Com) -> ComResult<()> {
        self.walk(root, root)
    }

    fn walk(&mut self, element: &Com, root: &Com) -> ComResult<()> {
        let tag = Layout::tag(element);
        if tag.as_deref() == Some("align:cover") {
            self.cover(element, root)?;
        }
        if let Some(kind @ ("align:row" | "align:baseline")) = tag.as_deref() {
            let bounds = Layout::bounds(element, root)?;
            if !bounds.empty() {
                let mut marks = Vec::new();
                for child in self.layout.children(element)? {
                    self.marks(&child, root, &mut marks)?;
                }
                self.rows += 1;
                if kind == "align:baseline" {
                    self.baselines(&marks);
                } else {
                    self.row(bounds, &marks);
                }
            }
        }
        for child in self.layout.children(element)? {
            self.walk(&child, root)?;
        }
        Ok(())
    }

    /// The first child of an `align:cover` area, its click target, covers all of it.
    fn cover(&mut self, element: &Com, root: &Com) -> ComResult<()> {
        let area = Layout::bounds(element, root)?;
        if area.empty() {
            return Ok(());
        }
        self.covers += 1;
        let target = match self.layout.children(element)?.into_iter().next() {
            Some(child) => Layout::bounds(&child, root)?,
            None => Rect {
                x: area.x,
                y: area.y,
                width: 0.0,
                height: 0.0,
            },
        };
        if !target.covers(&area, Self::TOLERANCE as f32) {
            self.problems.push(format!(
                "a click area of {:.0} × {:.0} at ({:.0}, {:.0}) leaves part of its {:.0} × {:.0} header at ({:.0}, {:.0}) dead",
                target.width, target.height, target.x, target.y, area.width, area.height, area.x, area.y
            ));
        }
        Ok(())
    }

    /// The marks share one line, in the middle of the row.
    fn row(&mut self, bounds: Rect, marks: &[Mark]) {
        let marks: Vec<&Mark> = marks.iter().filter(|m| !m.frame).collect();
        let by_middle = |a: &&Mark, b: &&Mark| a.middle.total_cmp(&b.middle);
        let (Some(high), Some(low)) = (
            marks.iter().copied().min_by(by_middle),
            marks.iter().copied().max_by(by_middle),
        ) else {
            return;
        };
        let name = Self::name(marks.iter().copied());
        if low.middle - high.middle > Self::TOLERANCE {
            self.problems.push(format!(
                "{} is {:.1} px below {} in the row of {name}",
                low.what,
                low.middle - high.middle,
                high.what
            ));
            return;
        }
        let line = (high.middle + low.middle) / 2.0;
        if (line - bounds.middle()).abs() > Self::TOLERANCE {
            self.problems.push(format!(
                "the row of {name} has its marks {:.1} px {} its middle",
                (line - bounds.middle()).abs(),
                if line < bounds.middle() {
                    "above"
                } else {
                    "below"
                }
            ));
        }
    }

    /// The texts share one baseline.
    fn baselines(&mut self, marks: &[Mark]) {
        let texts: Vec<(&str, f64)> = marks
            .iter()
            .filter_map(|m| m.baseline.map(|b| (m.what.as_str(), b)))
            .collect();
        let Some((first, line)) = texts.first().copied() else {
            return;
        };
        for (what, at) in &texts[1..] {
            if (at - line).abs() > Self::TOLERANCE {
                self.problems.push(format!(
                    "the baseline of {what} is {:.1} px {} {first}'s",
                    (at - line).abs(),
                    if *at < line { "above" } else { "below" }
                ));
            }
        }
    }

    /// A text in `bounds`, by the middle of its capitals.
    fn text(what: String, bounds: Rect, baseline: f64, size: f64) -> Mark {
        Mark {
            middle: baseline - size * Self::CAPITALS / 2.0,
            baseline: Some(baseline),
            ..Mark::boxed(what, bounds)
        }
    }

    /// The row's first text, to name it.
    fn name<'a>(marks: impl Iterator<Item = &'a Mark> + Clone) -> String {
        marks
            .clone()
            .find(|m| m.baseline.is_some())
            .or(marks.clone().find(|m| !m.frame))
            .map(|m| m.what.clone())
            .unwrap_or_default()
    }

    /// The marks in `element`: texts, icons, dots, swatches, a TextBox's line, blocks.
    fn marks(&self, element: &Com, root: &Com, marks: &mut Vec<Mark>) -> ComResult<()> {
        let bounds = Layout::bounds(element, root)?;
        if bounds.empty() {
            return Ok(());
        }
        let class = Layout::class(element);
        let short = class.rsplit('.').next().unwrap_or_default().to_owned();
        match Layout::tag(element).as_deref() {
            Some("align:free") => return Ok(()),
            Some("align:block") => {
                // A block by what it shows: an empty line in it is no part of it.
                let mut inner = Vec::new();
                for child in self.layout.children(element)? {
                    self.marks(&child, root, &mut inner)?;
                }
                let top = inner.iter().map(|m| m.top).fold(f64::MAX, f64::min);
                let bottom = inner.iter().map(|m| m.bottom).fold(f64::MIN, f64::max);
                if top < bottom {
                    marks.push(Mark {
                        what: format!("the block of {}", Self::name(inner.iter())),
                        middle: (top + bottom) / 2.0,
                        baseline: None,
                        top,
                        bottom,
                        frame: false,
                    });
                }
                return Ok(());
            }
            _ => {}
        }
        if let Some(line) = Layout::text(element) {
            if !line.text.trim().is_empty() {
                let baseline = bounds.top() + line.baseline;
                marks.push(Self::text(
                    format!("\"{}\" ({} px)", Self::clip(&line.text), line.size),
                    bounds,
                    baseline,
                    line.size,
                ));
            }
            return Ok(());
        }
        if let Some((text, size)) = Layout::text_box(element)
            && !text.is_empty()
        {
            if let Some(view) = self.find(element, "ScrollContentPresenter")? {
                let baseline = Layout::bounds(&view, root)?.top() + size * Self::TEXT_BOX_BASELINE;
                marks.push(Self::text(
                    format!("the text \"{}\" of a TextBox", Self::clip(&text)),
                    bounds,
                    baseline,
                    size,
                ));
            }
            return Ok(());
        }
        let children = self.layout.children(element)?;
        let shape = matches!(
            short.as_str(),
            "Viewbox" | "Ellipse" | "Path" | "Rectangle" | "Image" | "Line" | "ProgressRing"
        );
        // A childless Border or Grid is a mark too: a swatch, a frame (XAML may report a
        // childless Border as a Grid).
        if shape || (matches!(short.as_str(), "Border" | "Grid") && children.is_empty()) {
            let what = match short.as_str() {
                "Viewbox" => "an icon".to_owned(),
                "Border" => "a swatch".to_owned(),
                other => format!("a {other}"),
            };
            marks.push(Mark::boxed(what, bounds));
            return Ok(());
        }
        if short == "Border" {
            marks.push(Mark {
                frame: true,
                ..Mark::boxed("a frame".to_owned(), bounds)
            });
        }
        for child in children {
            self.marks(&child, root, marks)?;
        }
        Ok(())
    }

    /// The first element of class `name` under `element`.
    fn find(&self, element: &Com, name: &str) -> ComResult<Option<Com>> {
        for child in self.layout.children(element)? {
            if Layout::class(&child).ends_with(name) {
                return Ok(Some(child));
            }
            if let Some(found) = self.find(&child, name)? {
                return Ok(Some(found));
            }
        }
        Ok(None)
    }

    fn clip(text: &str) -> String {
        let mut clipped: String = text.chars().take(32).collect();
        if clipped.len() < text.len() {
            clipped.push('…');
        }
        clipped
    }
}
