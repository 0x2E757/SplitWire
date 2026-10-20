//! Alerts: the row shared by the bell, Status and the profile page, and the bell with
//! its badge and flyout.

use super::{Kit, Look};
use crate::ipc::Severity;
use crate::platform::window::markup::Align;
use crate::platform::window::markup::Styles;
use crate::platform::window::markup::region::Out;
use crate::presentation::{AlertView, BellFlyout, Command};

impl Kit {
    pub fn alert(
        out: &mut Out,
        alert: &AlertView,
        compact: bool,
        command: Option<Command>,
    ) -> String {
        let dismiss = if alert.dismissible {
            Self::button(
                out,
                "",
                Some("x"),
                Look::Subtle,
                true,
                true,
                Command::Dismiss(alert.id.clone()),
                "Dismiss until the condition changes",
            )
        } else {
            String::new()
        };
        let action = match (&alert.action, command) {
            (Some(label), Some(command)) => Self::link(out, label, command),
            _ => String::new(),
        };
        if compact {
            let background = match alert.severity {
                Severity::Error => "$errBg$",
                Severity::Warning => "$warnBg$",
                Severity::Info => "$subtle$",
            };
            return format!(
                r#"<Border Background="{background}" CornerRadius="4" Padding="10,3"><Grid ColumnSpacing="10"{}><Grid.ColumnDefinitions><ColumnDefinition Width="Auto"/><ColumnDefinition Width="Auto"/><ColumnDefinition/><ColumnDefinition Width="Auto"/><ColumnDefinition Width="Auto"/></Grid.ColumnDefinitions>{}<Grid Grid.Column="1">{}</Grid><Grid Grid.Column="2">{}</Grid><Grid Grid.Column="3">{action}</Grid><Grid Grid.Column="4">{dismiss}</Grid></Grid></Border>"#,
                Align::ROW,
                Self::severity_icon(alert.severity, 12),
                Self::line(&alert.title, 12, "$text$", r#" FontWeight="SemiBold""#),
                Self::line(&alert.message, 12, "$text2$", "")
            );
        }
        format!(
            r#"<Grid ColumnSpacing="10" RowSpacing="2" Padding="12,10"><Grid.ColumnDefinitions><ColumnDefinition Width="16"/><ColumnDefinition/><ColumnDefinition Width="Auto"/></Grid.ColumnDefinitions><Grid.RowDefinitions><RowDefinition Height="Auto"/><RowDefinition Height="Auto"/></Grid.RowDefinitions>{}<StackPanel Grid.Row="1" Grid.Column="1">{}<StackPanel Orientation="Horizontal" Spacing="10" Margin="0,4,0,0"{}>{}{action}</StackPanel></StackPanel><Grid Grid.Column="2" Grid.RowSpan="2" VerticalAlignment="Top">{dismiss}</Grid></Grid>"#,
            Self::first_line(
                &Self::severity_icon(alert.severity, 16),
                &alert.title,
                13,
                10
            ),
            Self::text(&alert.message, 12, "$text2$", r#"TextWrapping="Wrap""#),
            Align::ROW,
            Self::line(&alert.meta, 12, "$text3$", "")
        )
    }

    /// The bell with its badge in the worst severity, and its flyout.
    pub fn bell(
        out: &mut Out,
        badge: Option<(usize, Severity)>,
        flyout: &BellFlyout,
        actions: &dyn Fn(&str) -> Option<Command>,
    ) -> String {
        let badge = badge
            .map(|(count, severity)| {
                let color = match severity {
                    Severity::Error => "$danger$",
                    Severity::Warning => "$statusDot$",
                    Severity::Info => "$accent$",
                };
                format!(
                    r#"<Border HorizontalAlignment="Right" VerticalAlignment="Top" Margin="0,-5,-6,0" MinWidth="16" Height="16" CornerRadius="8" Padding="4,0" Background="{color}" IsHitTestVisible="False"{}><TextBlock Text="{count}" FontSize="10" FontWeight="SemiBold" Foreground="$card$" HorizontalAlignment="Center" VerticalAlignment="Center"/></Border>"#,
                    Align::FREE
                )
            })
            .unwrap_or_default();
        let dismiss_all = if flyout.can_dismiss_all {
            Self::link(out, "Dismiss all", Command::DismissAll)
        } else {
            String::new()
        };
        let body = if flyout.service_down {
            Self::empty(
                None,
                "",
                "Alerts come from the service, which is not running.",
            )
        } else if flyout.active.is_empty() {
            Self::empty(Some(("ok", "$accent$")), "", "No alerts")
        } else {
            flyout
                .active
                .iter()
                .map(|a| {
                    let command = actions(&a.id);
                    Self::alert(out, a, false, command)
                })
                .collect()
        };
        let dismissed = flyout
            .dismissed
            .as_ref()
            .map(|text| {
                format!(
                    r#"<Border Background="$subtle$" CornerRadius="4" Padding="12,8" Margin="0,4,0,0"><Grid ColumnSpacing="8"><Grid.ColumnDefinitions><ColumnDefinition Width="Auto"/><ColumnDefinition/><ColumnDefinition Width="Auto"/></Grid.ColumnDefinitions>{}<Grid Grid.Column="1">{}</Grid><Grid Grid.Column="2">{}</Grid></Grid></Border>"#,
                    Self::icon("eyeoff", 14, "$text2$"),
                    Self::text(text, 12, "$text2$", ""),
                    Self::link(out, "Restore", Command::RestoreAlerts)
                )
            })
            .unwrap_or_default();
        let cleared = if flyout.cleared.is_empty() {
            String::new()
        } else {
            let lines: String = flyout
                .cleared
                .iter()
                .map(|c| Self::text(c, 12, "$text3$", ""))
                .collect();
            format!(
                r#"<StackPanel Padding="12,8,12,0" Spacing="2">{}{lines}</StackPanel>"#,
                Self::text("Cleared today", 12, "$text2$", r#"FontWeight="SemiBold""#)
            )
        };
        let logs = Self::link(
            out,
            "Open Logs",
            Command::Go(crate::presentation::Page::Logs),
        );
        let button = out.name();
        let content = format!(
            r#"<StackPanel Width="380"><StackPanel.Resources>{}</StackPanel.Resources><Grid Padding="12,10,12,8" ColumnSpacing="12"><Grid.ColumnDefinitions><ColumnDefinition Width="Auto"/><ColumnDefinition/><ColumnDefinition Width="Auto"/></Grid.ColumnDefinitions>{}<Grid Grid.Column="1">{}</Grid><Grid Grid.Column="2">{dismiss_all}</Grid></Grid><ScrollViewer MaxHeight="520" VerticalScrollBarVisibility="Auto"><StackPanel>{body}</StackPanel></ScrollViewer>{dismissed}{cleared}<Border BorderBrush="$divider$" BorderThickness="0,1,0,0" Margin="0,8,0,0" Padding="12,10,12,8"><Grid ColumnSpacing="8"><Grid.ColumnDefinitions><ColumnDefinition Width="Auto"/><ColumnDefinition/></Grid.ColumnDefinitions>{logs}<Grid Grid.Column="1">{}</Grid></Grid></Border></StackPanel>"#,
            Styles::flyout_resources(),
            Self::bold("Alerts", 13),
            Self::text(&flyout.counts, 12, "$text3$", ""),
            Self::text(
                "Dismissed alerts return when the condition changes",
                12,
                "$text3$",
                r#"HorizontalAlignment="Right""#
            )
        );
        let flyout = format!(
            r#"<Button.Flyout><Flyout Placement="BottomEdgeAlignedRight">{}{content}</Flyout></Button.Flyout>"#,
            Styles::flyout_presenter(4, Some(392))
        );
        format!(
            r#"<Grid VerticalAlignment="Center">{}{badge}</Grid>"#,
            Self::button_named(
                &button,
                "",
                Some("bell"),
                Look::Default,
                false,
                true,
                "Alerts",
                &flyout
            )
        )
    }
}
