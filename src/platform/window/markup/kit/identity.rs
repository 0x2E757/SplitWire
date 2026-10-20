//! Identity marks: the profile's swatch, the state dot and pill, badges, severity icons.

use super::{Kit, Tone};
use crate::ipc::Severity;
use crate::platform::window::markup::Align;
use crate::presentation::{Dot, Pill};
use crate::profile::Pin;

impl Kit {
    /// The profile's colour: a 10 × 10 square, the pin at 35 % with a 1.5 stroke.
    pub fn swatch(pin: Pin) -> String {
        let n = pin.number();
        format!(
            r#"<Border Width="10" Height="10" CornerRadius="3" BorderThickness="1.5" BorderBrush="$pin{n}$" Background="$pin{n}a$" VerticalAlignment="Center"/>"#
        )
    }

    /// A swatch and a bold name.
    pub fn pname(pin: Pin, name: &str) -> String {
        format!(
            r#"<StackPanel Orientation="Horizontal" Spacing="8" VerticalAlignment="Center"{}>{}{}</StackPanel>"#,
            Align::ROW,
            Self::swatch(pin),
            Self::bold_line(name, 0)
        )
    }

    pub fn dot(dot: Dot) -> String {
        let (ring, fill) = match dot {
            Dot::Ok => ("$accentRing$", "$accent$"),
            Dot::Busy => ("$busyRing$", "$statusDot$"),
            Dot::Error => ("$dangerRing$", "$danger$"),
            Dot::Off => {
                return r#"<Grid Width="14" Height="14" VerticalAlignment="Center"><Ellipse Width="8" Height="8" Stroke="$textMuted$" StrokeThickness="1.5"/></Grid>"#.into();
            }
        };
        format!(
            r#"<Grid Width="14" Height="14" VerticalAlignment="Center"><Ellipse Fill="{ring}"/><Ellipse Width="8" Height="8" Fill="{fill}"/></Grid>"#
        )
    }

    pub fn pill(pill: &Pill) -> String {
        let tail = pill
            .tail
            .as_ref()
            .map(|t| Self::line(&format!("· {t}"), 12, "$text3$", ""))
            .unwrap_or_default();
        format!(
            r#"<Border Background="$subtle$" BorderBrush="$divider$" BorderThickness="1" Height="28" CornerRadius="14" Padding="10,0" VerticalAlignment="Center"{}><StackPanel Orientation="Horizontal" Spacing="7">{}{}{tail}</StackPanel></Border>"#,
            Align::ROW,
            Self::dot(pill.dot),
            Self::line(&pill.label, 12, "$text2$", "")
        )
    }

    pub fn badge(text: &str, tone: Tone) -> String {
        Self::badge_with(text, tone, "")
    }

    /// A badge with `lead` (an icon) before its text.
    pub fn badge_with(text: &str, tone: Tone, lead: &str) -> String {
        let (background, border, color) = match tone {
            Tone::Neutral => ("$subtle$", "$divider$", "$text2$"),
            Tone::Accent => ("$accentSoft$", "Transparent", "$accent$"),
            Tone::Warning => ("$warnBg$", "$warnBorder$", "$warn$"),
            Tone::Error => ("$errBg$", "$errBorder$", "$danger$"),
        };
        format!(
            r#"<Border Background="{background}" BorderBrush="{border}" BorderThickness="1" Height="20" CornerRadius="4" Padding="6,0" VerticalAlignment="Center" HorizontalAlignment="Left"{}><StackPanel Orientation="Horizontal" Spacing="4">{lead}{}</StackPanel></Border>"#,
            Align::ROW,
            Self::line(text, 11, color, "")
        )
    }

    pub fn severity_icon(severity: Severity, size: u32) -> String {
        match severity {
            Severity::Error => Self::icon("error", size, "$danger$"),
            Severity::Warning => Self::icon("alert", size, "$warn$"),
            Severity::Info => Self::icon("info", size, "$accent$"),
        }
    }
}
