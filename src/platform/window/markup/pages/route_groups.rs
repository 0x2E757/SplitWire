//! Routes grouped by list line: a header per group with a chevron, its icon, title and
//! caption, then its rows. The profile page adds a remove button per list line. The caption and
//! the rows move on with the routes and their ages, so they are live parts: a second's ages reload
//! them and not the chevron or the remove button.

use super::PageContext;
use crate::platform::window::markup::region::{Event, Out};
use crate::platform::window::markup::{Align, Kit, Look, Tone};
use crate::presentation::Command;
use crate::presentation::pages::{GroupIcon, RouteCellView, RouteGroupView};

pub struct RouteTable<'a> {
    pub profile: &'a str,
    pub groups: &'a [RouteGroupView],
    pub removable: bool,
}

impl RouteTable<'_> {
    pub fn widths(compact: bool) -> Vec<&'static str> {
        if compact {
            vec!["140", "*", "150"]
        } else {
            vec!["140", "*", "170", "150"]
        }
    }

    pub fn header(compact: bool) -> String {
        let labels: &[&str] = if compact {
            &["Address", "Name", "Expires"]
        } else {
            &["Address", "Name", "Last seen", "Expires"]
        };
        Kit::table_header(&Self::widths(compact), labels, "54,0,12,0")
    }

    fn cells(cell: &RouteCellView, compact: bool) -> Vec<String> {
        let t = |text: &str, color: &str| Kit::line(text, 13, color, "");
        let mut cells = match cell {
            RouteCellView::Learned {
                ip,
                name,
                last,
                expires,
            } => vec![
                t(ip, "$text$"),
                t(name, "$text$"),
                t(last, "$text2$"),
                t(expires, "$text2$"),
            ],
            RouteCellView::Static { net } => vec![
                t(net, "$text$"),
                t("static route", "$text2$"),
                t("—", "$text3$"),
                t("while connected", "$text2$"),
            ],
            RouteCellView::Upstream { net } => vec![
                t(net, "$text$"),
                t("upstream DNS", "$text2$"),
                t("—", "$text3$"),
                t("while connected", "$text2$"),
            ],
            RouteCellView::Held {
                ip,
                name,
                by,
                detail,
            } => vec![
                t(ip, "$text$"),
                t(name, "$text$"),
                Kit::badge(&format!("Held by {by}"), Tone::Warning),
                t(detail, "$text2$"),
            ],
            RouteCellView::Refused { net, name, reason } => vec![
                t(net, "$text$"),
                t(name, "$text$"),
                Kit::badge("Refused", Tone::Error),
                format!(
                    r#"<TextBlock Text="{}" FontSize="13" Foreground="$text2$"{} VerticalAlignment="Center" ToolTipService.ToolTip="{0}"/>"#,
                    Kit::esc(reason),
                    Align::line(13)
                ),
            ],
        };
        if compact {
            cells.remove(2);
        }
        cells
    }

    pub fn markup(&self, out: &mut Out, context: &PageContext) -> String {
        let compact = context.compact();
        let widths = Self::widths(compact);
        self.groups
            .iter()
            .map(|group| {
                let open = context.frame.open(&group.id, true);
                let toggle = out.bind(Event::Click, Command::Toggle(group.id.clone()));
                let (icon, color) = match group.icon {
                    GroupIcon::Domain => ("globe", "$text2$"),
                    GroupIcon::Network => ("routes", "$text2$"),
                    GroupIcon::Upstream => ("server", "$text2$"),
                    GroupIcon::Refused => ("error", "$danger$"),
                };
                let remove = match (&group.line, self.removable) {
                    (Some((line, text)), true) => Kit::button(
                        out,
                        "",
                        Some("trash"),
                        Look::Subtle,
                        true,
                        true,
                        Command::RemoveLine {
                            profile: self.profile.to_owned(),
                            line: *line,
                            text: text.clone(),
                        },
                        "Remove this line",
                    ),
                    _ => String::new(),
                };
                let header = format!(
                    r#"<Grid ColumnSpacing="4" Margin="0,4,0,0"><Grid.ColumnDefinitions><ColumnDefinition/><ColumnDefinition Width="Auto"/></Grid.ColumnDefinitions><Button x:Name="{toggle}" HorizontalAlignment="Stretch" HorizontalContentAlignment="Stretch" Background="Transparent" BorderThickness="0" MinHeight="40" Padding="8,0"><Grid ColumnSpacing="10" Tag="align:row"><Grid.ColumnDefinitions><ColumnDefinition Width="Auto"/><ColumnDefinition Width="Auto"/><ColumnDefinition Width="Auto"/><ColumnDefinition/></Grid.ColumnDefinitions>{}<Grid Grid.Column="1">{}</Grid><Grid Grid.Column="2">{}</Grid><Grid Grid.Column="3">{}</Grid></Grid></Button><Grid Grid.Column="1" VerticalAlignment="Center">{remove}</Grid></Grid>"#,
                    Kit::icon(if open { "chev" } else { "chevr" }, 12, "$text2$"),
                    Kit::icon(icon, 16, color),
                    Kit::bold_line(&group.title, 13),
                    out.live("", |_| Kit::line(&group.caption, 12, "$text3$", ""))
                );
                if !open {
                    return header;
                }
                let rows: String = if group.rows.is_empty() {
                    Kit::row(
                        &["*"],
                        &[Kit::line(&group.empty, 13, "$text3$", "")],
                        "Transparent",
                        "20,0,12,0",
                    )
                } else {
                    group
                        .rows
                        .iter()
                        .map(|cell| Kit::row(&widths, &Self::cells(cell, compact), "Transparent", "20,0,12,0"))
                        .collect()
                };
                let rows = out.live("", |_| {
                    format!(
                        r#"<Border BorderBrush="$divider$" BorderThickness="1,0,0,0" Margin="17,0,0,0"><StackPanel>{rows}</StackPanel></Border>"#
                    )
                });
                format!("{header}{rows}")
            })
            .collect()
    }
}
