/// The window's width classes (TaskbarMetrics' responsiveness): the full 248 px
/// navigation from 1100 DIP, the 56 px icon navigation below. The minimum width of 780 makes the
/// icon navigation the normal narrow layout.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WidthClass {
    Full,
    Compact,
}

impl WidthClass {
    pub const ALL: [Self; 2] = [Self::Full, Self::Compact];

    pub fn of(width: f64) -> Self {
        if width >= 1100.0 {
            Self::Full
        } else {
            Self::Compact
        }
    }

    pub fn compact(self) -> bool {
        self == Self::Compact
    }

    pub fn nav_width(self) -> u32 {
        match self {
            Self::Full => 248,
            Self::Compact => 56,
        }
    }

    /// Horizontal content padding.
    pub fn content_padding(self) -> &'static str {
        match self {
            Self::Full => "24,18,24,20",
            Self::Compact => "16,18,16,20",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn breakpoint_is_1100() {
        assert_eq!(WidthClass::of(1280.0), WidthClass::Full);
        assert_eq!(WidthClass::of(1100.0), WidthClass::Full);
        assert_eq!(WidthClass::of(1099.0), WidthClass::Compact);
        assert_eq!(WidthClass::of(780.0).nav_width(), 56);
        assert!(WidthClass::Compact.compact());
    }
}
