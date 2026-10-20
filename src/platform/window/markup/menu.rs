//! The profile menu: a MenuFlyout whose items are drawn as the kit's menu
//! rows. Each item keeps the MenuFlyoutItem's behaviour (a click runs it and closes the menu,
//! Escape and a click outside close it) under a template of its own: 32 px rows with the window's
//! 16 px icons, a status line at the top, thin separators, Remove… in the danger colour.

use super::region::{Event, Out};
use super::{Align, Kit, Styles};
use crate::presentation::{MenuEntry, ProfileMenu};

pub struct Menu;

impl Menu {
    /// The menu's least width.
    pub const WIDTH: u32 = 220;

    /// A row's frame inside a MenuFlyoutItem template: icon column, label, hover and press.
    fn row(lead: &str, label: &str, states: bool) -> String {
        let states = if states {
            r#"<VisualStateManager.VisualStateGroups><VisualStateGroup x:Name="CommonStates"><VisualState x:Name="Normal"/><VisualState x:Name="PointerOver"><VisualState.Setters><Setter Target="Row.Background" Value="$ctrlHover$"/></VisualState.Setters></VisualState><VisualState x:Name="Pressed"><VisualState.Setters><Setter Target="Row.Background" Value="$ctrlPress$"/></VisualState.Setters></VisualState><VisualState x:Name="Disabled"/></VisualStateGroup></VisualStateManager.VisualStateGroups>"#
        } else {
            ""
        };
        format!(
            r#"<MenuFlyoutItem.Template><ControlTemplate TargetType="MenuFlyoutItem"><Grid x:Name="Row" Height="32" Padding="12,0" ColumnSpacing="12" CornerRadius="4" Background="Transparent"{}>{states}<Grid.ColumnDefinitions><ColumnDefinition Width="16"/><ColumnDefinition/></Grid.ColumnDefinitions><Grid VerticalAlignment="Center">{lead}</Grid><Grid Grid.Column="1" VerticalAlignment="Center">{label}</Grid></Grid></ControlTemplate></MenuFlyoutItem.Template>"#,
            Align::ROW
        )
    }

    fn label(text: &str, size: u32, color: &str) -> String {
        Kit::line(text, size, color, r#"FontFamily="Segoe UI Variable Text""#)
    }

    fn item(out: &mut Out, entry: &MenuEntry) -> String {
        let name = out.bind(Event::MenuClick, entry.command.clone());
        let (icon, text) = match (entry.enabled, entry.danger) {
            (false, _) => ("$disabled$", "$disabled$"),
            (true, true) => ("$danger$", "$danger$"),
            (true, false) => ("$text2$", "$text$"),
        };
        format!(
            r#"<MenuFlyoutItem x:Name="{name}" IsEnabled="{}">{}</MenuFlyoutItem>"#,
            if entry.enabled { "True" } else { "False" },
            Self::row(
                &Kit::icon(entry.icon, 16, icon),
                &Self::label(&entry.label, 13, text),
                true
            )
        )
    }

    /// The status line: the state dot and `AcmeCorp · Connected` in secondary text, not a command.
    fn header(menu: &ProfileMenu) -> String {
        format!(
            r#"<MenuFlyoutItem IsEnabled="False" IsTabStop="False">{}</MenuFlyoutItem>"#,
            Self::row(
                &Kit::dot(menu.dot),
                &Self::label(&menu.header, 12, "$text2$"),
                false
            )
        )
    }

    fn separator() -> &'static str {
        r#"<MenuFlyoutSeparator IsTabStop="False"><MenuFlyoutSeparator.Template><ControlTemplate TargetType="MenuFlyoutSeparator"><Rectangle Height="1" Margin="-4,4" Fill="$divider$"/></ControlTemplate></MenuFlyoutSeparator.Template></MenuFlyoutSeparator>"#
    }

    /// The header, then the groups with a separator before each.
    fn items(out: &mut Out, menu: &ProfileMenu) -> String {
        let mut items = Self::header(menu);
        for group in &menu.groups {
            items.push_str(Self::separator());
            for entry in group {
                items.push_str(&Self::item(out, entry));
            }
        }
        items
    }

    /// The menu as `element` (`Button.Flyout`, `Button.ContextFlyout`) of a button.
    pub fn flyout(out: &mut Out, menu: &ProfileMenu, element: &str) -> String {
        format!(
            r#"<{element}><MenuFlyout Placement="BottomEdgeAlignedRight">{}{}</MenuFlyout></{element}>"#,
            Styles::menu_presenter(Self::WIDTH),
            Self::items(out, menu)
        )
    }

    /// The same items in a card drawn as the menu's presenter, for `--render-pages` and the
    /// alignment check of `--verify-pages`: no flyout opens without a window.
    pub fn preview(out: &mut Out, menu: &ProfileMenu) -> String {
        format!(
            r#"<Grid Background="$bg$" Padding="24"><Border Background="$card$" BorderBrush="$border$" BorderThickness="1" CornerRadius="8" Padding="4" MinWidth="{}" HorizontalAlignment="Left" VerticalAlignment="Top"><StackPanel>{}</StackPanel></Border></Grid>"#,
            Self::WIDTH,
            Self::items(out, menu)
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::presentation::showcase::Driver;
    use crate::presentation::transport::demo::Scenario;

    #[test]
    fn every_item_is_bound_and_nothing_is_timed() {
        let view = Driver::new(Scenario::Normal).session.view();
        let menu = view.menus.iter().find(|m| m.name == "AcmeCorp").unwrap();
        let mut out = Out::default();
        let markup = Menu::flyout(&mut out, menu, "Button.Flyout");
        assert!(markup.starts_with("<Button.Flyout><MenuFlyout"));
        assert!(markup.contains(r#"Foreground="$danger$""#));
        assert!(markup.contains("AcmeCorp · On"));
        assert!(!markup.contains("Connect for") && !markup.contains("Keep"));
        assert!(!markup.contains("FontIcon"));
        assert_eq!(markup.matches("<MenuFlyoutSeparator ").count(), 3);
        assert_eq!(
            out.finish("H", format!("<Button>{markup}</Button>"))
                .binds
                .len(),
            4
        );
    }
}
