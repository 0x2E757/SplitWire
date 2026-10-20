//! The navigation pane's markup: full 248 px or icon-only 56 px, profile items
//! with their swatch, value, right-click menu and drag to reorder.

use super::menu::Menu;
use super::region::{Event, Out};
use super::{Align, Icons, Kit};
use crate::presentation::{
    Command, NavItem, NavLead, NavValue, NavView, ProfileMenu, Tone, WidthClass,
};

pub struct Nav;

impl Nav {
    fn value(value: &NavValue, enabled: bool) -> String {
        let color = if enabled { "$text2$" } else { "$disabled$" };
        let label = |text: &str| Kit::line(text, 12, color, "");
        let content = match value {
            NavValue::None => return String::new(),
            NavValue::Text(text) => label(text),
            NavValue::Dot(dot, text) => format!("{}{}", Kit::dot(*dot), label(text)),
            NavValue::Icon(icon, tone, text) => {
                let tone = match tone {
                    Tone::Accent => "$accent$",
                    Tone::Danger => "$danger$",
                    Tone::Warning => "$warn$",
                    Tone::Muted => "$text3$",
                };
                format!(
                    "{}{}",
                    Icons::markup(icon, 12, if enabled { tone } else { color }),
                    label(text)
                )
            }
        };
        format!(
            r#"<StackPanel Orientation="Horizontal" Spacing="6" VerticalAlignment="Center"{}>{content}</StackPanel>"#,
            Align::ROW
        )
    }

    /// One item; a profile item at `index` can also be dragged to reorder.
    fn item(
        out: &mut Out,
        item: &NavItem,
        compact: bool,
        menu: Option<&ProfileMenu>,
        index: usize,
    ) -> String {
        let name = out.bind(Event::Click, item.command.clone());
        if let (Some(profile), true) = (&item.profile, item.enabled) {
            out.also(
                &name,
                Event::NavDrag,
                Command::MoveProfile(profile.clone(), index),
            );
        }
        let color = if item.enabled { "$text$" } else { "$disabled$" };
        let lead = match &item.lead {
            NavLead::Icon(icon) => Icons::markup(icon, 16, color),
            NavLead::Swatch(pin) => format!(r#"<Grid Width="16">{}</Grid>"#, Kit::swatch(*pin)),
        };
        let indicator = if item.selected {
            r#"<Border Width="3" Height="16" CornerRadius="2" Background="$accent$" HorizontalAlignment="Left" VerticalAlignment="Top" Margin="0,10,0,0"/>"#
        } else {
            ""
        };
        let body = if compact {
            format!(
                r#"<Grid HorizontalAlignment="Center" VerticalAlignment="Center">{lead}</Grid>"#
            )
        } else {
            // The value counts what the service does: a live part, so a new count reloads
            // neither the pane nor an open profile menu.
            let value = out.live(r#"Grid.Column="2""#, |_| {
                Self::value(&item.value, item.enabled)
            });
            format!(
                r#"<Grid Padding="14,0,12,0" ColumnSpacing="12"><Grid.ColumnDefinitions><ColumnDefinition Width="16"/><ColumnDefinition/><ColumnDefinition Width="Auto"/></Grid.ColumnDefinitions><Grid VerticalAlignment="Center">{lead}</Grid><TextBlock Grid.Column="1" Text="{}" FontWeight="{}" Foreground="{color}" VerticalAlignment="Center"{}/>{value}</Grid>"#,
                Kit::esc(&item.label),
                if item.selected { "SemiBold" } else { "Normal" },
                Align::line(13)
            )
        };
        let tip = if compact {
            format!(r#" ToolTipService.ToolTip="{}""#, Kit::esc(&item.tooltip))
        } else {
            String::new()
        };
        let context = menu
            .filter(|_| item.enabled)
            .map(|m| Menu::flyout(out, m, "Button.ContextFlyout"))
            .unwrap_or_default();
        format!(
            r#"<Button x:Name="{name}" Height="36" MinHeight="36" Padding="0" HorizontalAlignment="Stretch" HorizontalContentAlignment="Stretch" VerticalContentAlignment="Stretch" BorderThickness="0" Background="{}" IsEnabled="{}"{tip}{}><Button.Resources><SolidColorBrush x:Key="ButtonBackgroundPointerOver" Color="$navHover$"/><SolidColorBrush x:Key="ButtonBackgroundPressed" Color="$navPress$"/><SolidColorBrush x:Key="ButtonBackgroundDisabled" Color="Transparent"/></Button.Resources>{context}<Grid>{indicator}{body}</Grid></Button>"#,
            if item.selected {
                "$navSel$"
            } else {
                "Transparent"
            },
            if item.enabled { "True" } else { "False" },
            Align::ROW
        )
    }

    /// The pane; `menus` are the profile menus in the profiles' order.
    pub fn markup(
        out: &mut Out,
        nav: &NavView,
        width: WidthClass,
        menus: &[ProfileMenu],
        overlay: bool,
    ) -> String {
        let compact = width.compact() && !overlay;
        let menu_button = if compact {
            let name = out.bind(Event::Click, Command::NavOverlay(true));
            format!(
                r#"<Button x:Name="{name}" Height="36" MinHeight="36" Padding="0" HorizontalAlignment="Stretch" BorderThickness="0" Background="Transparent" Margin="0,0,0,10" ToolTipService.ToolTip="Menu" Tag="align:row">{}</Button>"#,
                Icons::markup("menu", 16, "$text$")
            )
        } else {
            String::new()
        };
        let profiles: String = nav
            .profiles
            .iter()
            .enumerate()
            .map(|(index, item)| {
                let menu = menus
                    .iter()
                    .find(|m| Some(&m.name) == item.profile.as_ref());
                Self::item(out, item, compact, menu, index)
            })
            .collect();
        let top = format!(
            "{menu_button}{}{profiles}{}",
            Self::item(out, &nav.status, compact, None, 0),
            Self::item(out, &nav.add, compact, None, 0)
        );
        let middle = format!(
            "{}{}{}",
            Self::item(out, &nav.routes, compact, None, 0),
            Self::item(out, &nav.lists, compact, None, 0),
            Self::item(out, &nav.logs, compact, None, 0)
        );
        let (padding, spacing) = if compact {
            ("8,4,8,12", 4)
        } else {
            ("8,8,8,12", 2)
        };
        format!(
            r#"<Grid Padding="{padding}"><Grid.RowDefinitions><RowDefinition/><RowDefinition Height="Auto"/></Grid.RowDefinitions><ScrollViewer VerticalScrollBarVisibility="Auto"><StackPanel x:Name="NavItems" Spacing="{spacing}">{top}<Border Height="1" Background="$divider$" Margin="6"/>{middle}</StackPanel></ScrollViewer><Grid Grid.Row="1" Margin="0,4,0,0">{}</Grid></Grid>"#,
            Self::item(out, &nav.settings, compact, None, 0)
        )
    }
}
