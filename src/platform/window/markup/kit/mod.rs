//! The shared components of the UI kit as system XAML: identity (swatch, dot,
//! pill, badge), controls (buttons, switch, Segmented, fields, links), blocks (header, stat,
//! InfoBar, cards, expanders, rows, the apply bar, empty states, tables) and alerts.

mod alerts;
mod blocks;
mod controls;
mod identity;

use super::{Align, Icons, Xml};

/// A Button's look.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Look {
    Default,
    Primary,
    Subtle,
    Danger,
}

/// An InfoBar's or a badge's tone.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tone {
    Neutral,
    Accent,
    Warning,
    Error,
}

pub struct Kit;

impl Kit {
    pub fn esc(text: &str) -> String {
        Xml::escape(text)
    }

    pub fn icon(name: &str, size: u32, color: &str) -> String {
        Icons::markup(name, size, color)
    }

    /// A TextBlock: `size` 0 keeps 13.
    pub fn text(text: &str, size: u32, color: &str, extra: &str) -> String {
        let size = if size == 0 { 13 } else { size };
        format!(
            r#"<TextBlock Text="{}" FontSize="{size}" Foreground="{color}" VerticalAlignment="Center" {extra}/>"#,
            Self::esc(text)
        )
    }

    pub fn bold(text: &str, size: u32) -> String {
        Self::text(text, size, "$text$", r#"FontWeight="SemiBold""#)
    }

    /// A text on one line of a row: its capitals in the middle of its line, so it lines up with
    /// the icons, dots and controls beside it ([`Align::line`]).
    pub fn line(text: &str, size: u32, color: &str, extra: &str) -> String {
        let size = if size == 0 { 13 } else { size };
        Self::text(text, size, color, &format!("{} {extra}", Align::line(size)))
    }

    pub fn bold_line(text: &str, size: u32) -> String {
        Self::line(text, size, "$text$", r#" FontWeight="SemiBold""#)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::platform::window::markup::region::{Event, Out};
    use crate::presentation::Page;
    use crate::presentation::{Command, Dot, InputId, Pill};
    use crate::profile::Pin;

    #[test]
    fn buttons_bind_and_take_their_look() {
        let mut out = Out::default();
        let primary = Kit::button(
            &mut out,
            "Apply",
            None,
            Look::Primary,
            false,
            true,
            Command::SettingsApply,
            "",
        );
        assert!(
            primary.contains(r#"Background="$accent$""#)
                && primary.contains("ButtonBackgroundPointerOver")
        );
        let disabled = Kit::button(
            &mut out,
            "Apply",
            None,
            Look::Primary,
            false,
            false,
            Command::SettingsApply,
            "",
        );
        assert!(
            disabled.contains(r#"IsEnabled="False""#)
                && disabled.contains(r#"Foreground="$disabled$""#)
        );
        let icon = Kit::button(
            &mut out,
            "",
            Some("more"),
            Look::Subtle,
            true,
            true,
            Command::Go(Page::Status),
            "Profile menu",
        );
        assert!(
            icon.contains(r#"Width="26""#)
                && icon.contains("ToolTipService.ToolTip=\"Profile menu\"")
        );
        let region = out.finish(
            "H",
            format!("<StackPanel>{primary}{disabled}{icon}</StackPanel>"),
        );
        assert_eq!(region.binds.len(), 3);
    }

    #[test]
    fn fields_carry_their_token_and_submit_on_enter() {
        let mut out = Out::default();
        let field = Kit::field(
            &mut out,
            InputId::LogFilter,
            "dns",
            "Filter by text",
            Some("search"),
            Some(180),
            None,
            false,
        );
        assert!(field.contains(r#"Text="§LogFilter§""#));
        let region = out.finish("H", field);
        assert_eq!(region.binds[0].event, Event::Enter);
        assert_eq!(region.inputs[0].1, "dns");
    }

    #[test]
    fn spread_stats_line_up_labels_values_and_captions_in_equal_columns() {
        let stats = [
            crate::presentation::pages::Stat::new("Handshake", "42 s ago", "keepalive 25 s"),
            crate::presentation::pages::Stat::new("Routes (DNS)", "14", ""),
        ];
        let row = Kit::spread_stats(&stats, false);
        assert_eq!(row.matches(Align::BASELINE).count(), 3);
        assert_eq!(row.matches("<ColumnDefinition/>").count(), 6);
        assert!(row.contains(r#"Text="14" FontSize="20""#));
        assert!(row.contains(r#"Text="" FontSize="11""#) && row.contains(r#"MinHeight="15""#));
        assert!(Kit::spread_stats(&stats, true).contains(r#"Text="14" FontSize="18""#));
    }

    #[test]
    fn identity_marks_use_the_pin_and_state_tokens() {
        assert!(Kit::swatch(Pin::new(3).unwrap()).contains("$pin3a$"));
        assert!(Kit::dot(Dot::Busy).contains("$statusDot$"));
        assert!(
            Kit::pill(&Pill::new(Dot::Ok, "Connected", Some("47 min".into()))).contains("· 47 min")
        );
    }
}
