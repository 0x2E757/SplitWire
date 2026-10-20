//! The window frame: the navigation pane on `bg`, the content layer with the page and
//! the apply bar pinned under it, and the overlays (the full navigation in the narrow layout,
//! dialogs, the toast). The skeleton is loaded once per theme and width class; regions fill it.

use super::nav::Nav;
use super::pages::{Dialogs, PageContext, page};
use super::region::{Out, Region};
use super::{Kit, Styles};
use crate::presentation::{Command, FrameView, WidthClass};

pub struct Frame;

impl Frame {
    pub const HOSTS: [&'static str; 6] = [
        "NavHost",
        "PageHost",
        "ApplyBarHost",
        "ToastHost",
        "NavOverlayHost",
        "DialogHost",
    ];

    /// The skeleton: the root with its resources and the empty hosts.
    pub fn skeleton(width: WidthClass) -> String {
        super::Xml::root(&format!(
            r#"<Grid x:Name="Root" RequestedTheme="Default" Background="$bg$"><Grid.Resources>{}</Grid.Resources><Grid.ColumnDefinitions><ColumnDefinition Width="{}"/><ColumnDefinition/></Grid.ColumnDefinitions><ContentControl x:Name="NavHost" HorizontalContentAlignment="Stretch" VerticalContentAlignment="Stretch"/><Border Grid.Column="1" Background="$layer$" BorderBrush="$border$" BorderThickness="1,1,0,0" CornerRadius="8,0,0,0"><Grid><Grid.RowDefinitions><RowDefinition/><RowDefinition Height="Auto"/></Grid.RowDefinitions><ContentControl x:Name="PageHost" HorizontalContentAlignment="Stretch" VerticalContentAlignment="Stretch"/><ContentControl x:Name="ApplyBarHost" Grid.Row="1" HorizontalContentAlignment="Stretch"/><ContentControl x:Name="ToastHost" HorizontalAlignment="Center" VerticalAlignment="Bottom" Margin="0,0,0,24" IsHitTestVisible="False"/></Grid></Border><ContentControl x:Name="NavOverlayHost" Grid.ColumnSpan="2" HorizontalContentAlignment="Stretch" VerticalContentAlignment="Stretch"/><ContentControl x:Name="DialogHost" Grid.ColumnSpan="2" HorizontalContentAlignment="Stretch" VerticalContentAlignment="Stretch"/><Canvas x:Name="DragLayer" Grid.ColumnSpan="2" IsHitTestVisible="False"/></Grid>"#,
            Styles::resources(),
            width.nav_width()
        ))
    }

    /// Every region of the frame for one view.
    pub fn regions(frame: &FrameView, width: WidthClass) -> Vec<Region> {
        let mut regions = Vec::new();
        let mut out = Out::default();
        let nav = Nav::markup(&mut out, &frame.nav, width, &frame.menus, false);
        regions.push(out.finish("NavHost", nav));

        let context = PageContext { frame, width };
        regions.push(page(&context).region(width.content_padding()));

        let mut out = Out::default();
        let bar = frame
            .apply_bar
            .as_ref()
            .map(|bar| {
                Kit::apply_bar(
                    &mut out,
                    bar,
                    Command::SettingsRevert,
                    Command::SettingsApply,
                    "Apply",
                )
            })
            .unwrap_or_default();
        regions.push(out.finish("ApplyBarHost", bar));

        let toast = frame
            .toast
            .as_ref()
            .map(|t| {
                format!(
                    r#"<Border Background="$card$" BorderBrush="$border$" BorderThickness="1" CornerRadius="8" Padding="14,8">{}</Border>"#,
                    Kit::text(t, 13, "$text$", "")
                )
            })
            .unwrap_or_default();
        regions.push(Out::default().finish("ToastHost", toast));

        let mut out = Out::default();
        let overlay = if frame.nav_overlay && width.compact() {
            let dismiss = out.bind(super::region::Event::Click, Command::NavOverlay(false));
            let nav = Nav::markup(&mut out, &frame.nav, width, &frame.menus, true);
            format!(
                r#"<Grid><Button x:Name="{dismiss}" HorizontalAlignment="Stretch" VerticalAlignment="Stretch" Background="Transparent" BorderThickness="0"/><Border Width="248" HorizontalAlignment="Left" Background="$bg$" BorderBrush="$border$" BorderThickness="0,0,1,0">{nav}</Border></Grid>"#
            )
        } else {
            String::new()
        };
        regions.push(out.finish("NavOverlayHost", overlay));

        let mut out = Out::default();
        let dialog = Dialogs::markup(&mut out, frame);
        regions.push(out.finish("DialogHost", dialog));
        regions
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_skeleton_has_every_host() {
        for width in WidthClass::ALL {
            let skeleton = Frame::skeleton(width);
            for host in Frame::HOSTS {
                assert!(skeleton.contains(&format!(r#"x:Name="{host}""#)), "{host}");
            }
        }
        assert!(Frame::skeleton(WidthClass::Compact).contains(r#"<ColumnDefinition Width="56"/>"#));
    }
}
