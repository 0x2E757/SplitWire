//! The marks `--verify-pages` reads to check alignment (`Tag` values, invisible), and the text
//! that keeps its letters in the middle of its line.
//!
//! A single-line row (`ROW`) has every mark in it on its middle line: a TextBlock by the middle of
//! its capitals, an icon, a dot, a swatch or a TextBox's text by theirs. A `BASELINE` row has its
//! texts on one baseline, for texts of different sizes side by side (a title and its subtitle).
//! `BLOCK` is one mark by its box (a title over a description); `FREE` is left out (a count badge
//! on a corner). `COVER` is an area whose first child is its click target: the target must cover
//! the whole area (a Routes card's header).

pub struct Align;

impl Align {
    pub const ROW: &'static str = r#" Tag="align:row""#;
    pub const BASELINE: &'static str = r#" Tag="align:baseline""#;
    pub const BLOCK: &'static str = r#" Tag="align:block""#;
    pub const FREE: &'static str = r#" Tag="align:free""#;
    pub const COVER: &'static str = r#" Tag="align:cover""#;

    /// The line height of a single-line text of `size`: XAML puts the baseline at 3/4 of a
    /// block line height, so 1.4 em puts the middle of the capitals (0.7 em high) in the middle of
    /// the line; even, so a 14 or 16 px mark beside it needs no rounding.
    pub fn line_height(size: u32) -> u32 {
        (f64::from(size) * 0.7).round() as u32 * 2
    }

    /// The attributes of a single-line text of `size`.
    pub fn line(size: u32) -> String {
        format!(
            r#" LineHeight="{}" LineStackingStrategy="BlockLineHeight""#,
            Self::line_height(size)
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn line_heights_are_even_and_near_one_and_four_tenths() {
        assert_eq!(Align::line_height(11), 16);
        assert_eq!(Align::line_height(12), 16);
        assert_eq!(Align::line_height(13), 18);
        assert_eq!(Align::line_height(15), 22);
        assert!(Align::line(12).contains(r#"LineHeight="16""#));
    }
}
