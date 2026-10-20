//! Logs (newest on top): Live or `splitwire.log`, filtered by level, profile and text.

use super::{PageContext, PageMarkup};
use crate::logs::Level;
use crate::platform::window::markup::region::{Event, Out};
use crate::platform::window::markup::{Align, Kit, Look, Styles, Tone};
use crate::presentation::pages::LogsPageView;
use crate::presentation::{Command, InputId, LogSource};

fn profile_filter(out: &mut Out, view: &LogsPageView) -> String {
    let items: String = view
        .profiles
        .iter()
        .map(|(filter, label, colour)| {
            let name = out.bind(Event::Click, Command::LogProfile(filter.clone()));
            let lead = colour.map(Kit::swatch).unwrap_or_else(|| r#"<Grid Width="10"/>"#.into());
            let check = if view.profile_label == *label
                || (label.starts_with("Global") && view.profile_label == "Global")
            {
                Kit::icon("check", 12, "$text$")
            } else {
                String::new()
            };
            format!(
                r#"<Button x:Name="{name}" HorizontalAlignment="Stretch" HorizontalContentAlignment="Stretch" Background="Transparent" BorderThickness="0" Height="32" MinHeight="32" Padding="12,0"><Grid ColumnSpacing="12" Tag="align:row"><Grid.ColumnDefinitions><ColumnDefinition Width="Auto"/><ColumnDefinition/><ColumnDefinition Width="Auto"/></Grid.ColumnDefinitions>{lead}<Grid Grid.Column="1">{}</Grid><Grid Grid.Column="2">{check}</Grid></Grid></Button>"#,
                Kit::line(label, 13, "$text$", "")
            )
        })
        .collect();
    let lead = view.profile_colour.map(Kit::swatch).unwrap_or_default();
    let button = out.name();
    let flyout = format!(
        r#"<Button.Flyout><Flyout Placement="BottomEdgeAlignedLeft">{}<StackPanel Width="250" Spacing="2"><StackPanel.Resources>{}</StackPanel.Resources>{items}</StackPanel></Flyout></Button.Flyout>"#,
        Styles::flyout_presenter(4, None),
        Styles::flyout_resources()
    );
    let clear = if view.profile_filtered {
        Kit::button(
            out,
            "",
            Some("x"),
            Look::Subtle,
            true,
            true,
            Command::LogProfile(crate::presentation::ProfileFilter::All),
            "Remove the profile filter",
        )
    } else {
        String::new()
    };
    format!(
        r#"<StackPanel Orientation="Horizontal" Spacing="2"><Button x:Name="{button}" MinWidth="160" Padding="10,0" HorizontalContentAlignment="Stretch"><Grid ColumnSpacing="8" Tag="align:row"><Grid.ColumnDefinitions><ColumnDefinition Width="Auto"/><ColumnDefinition/><ColumnDefinition Width="Auto"/></Grid.ColumnDefinitions>{lead}<Grid Grid.Column="1">{}</Grid><Grid Grid.Column="2">{}</Grid></Grid>{flyout}</Button>{clear}</StackPanel>"#,
        Kit::line(
            &view.profile_label,
            13,
            "$text$",
            if view.profile_filtered {
                r#" FontWeight="SemiBold""#
            } else {
                ""
            }
        ),
        Kit::icon("chev", 14, "$text2$")
    )
}

pub fn page(context: &PageContext, view: &LogsPageView, key: String) -> PageMarkup {
    let live = view.source == LogSource::Live;
    let mut out = Out::default();
    let seg = Kit::segmented(
        &mut out,
        vec![
            (
                view.live_label.clone(),
                live,
                Command::LogSource(LogSource::Live),
            ),
            (
                view.file_label.clone(),
                !live,
                Command::LogSource(LogSource::File),
            ),
        ],
    );
    let copy = Kit::button(
        &mut out,
        if live { "Copy" } else { "Copy all" },
        Some("copy"),
        Look::Default,
        false,
        true,
        Command::Copy(view.copy_text.clone()),
        "",
    );
    let folder = Kit::button(
        &mut out,
        "Open log folder",
        Some("folder"),
        Look::Default,
        false,
        true,
        Command::OpenLogFolder,
        "",
    );
    let header = context.header_with(&mut out, "Logs", &seg, &format!("{copy}{folder}"));
    let header = out.finish("Header", header);

    let mut tools = Out::default();
    // The counts, the profiles and the paused bar follow the filter as it is typed, and new lines.
    let levels = tools.live("", |part| {
        Kit::segmented(
            part,
            view.levels
                .iter()
                .map(|(filter, label)| {
                    (
                        label.clone(),
                        *filter == view.level,
                        Command::LogLevel(*filter),
                    )
                })
                .collect(),
        )
    });
    let filter = Kit::field(
        &mut tools,
        InputId::LogFilter,
        &context.frame.input(&InputId::LogFilter),
        "Filter by text",
        Some("search"),
        Some(180),
        None,
        false,
    );
    let end = tools.live(r#"Grid.Column="4""#, |part| {
        if live {
            Kit::toggle(part, view.follow, true, Command::LogFollow, "Follow", "")
        } else {
            Kit::line(
                view.footer.rsplit(" · ").next().unwrap_or_default(),
                12,
                "$text3$",
                "",
            )
        }
    });
    let profiles = tools.live(r#"Grid.Column="1""#, |part| profile_filter(part, view));
    let toolbar = format!(
        r#"<Grid Padding="16,6,12,6" ColumnSpacing="12" Tag="align:row"><Grid.ColumnDefinitions><ColumnDefinition Width="Auto"/><ColumnDefinition Width="Auto"/><ColumnDefinition Width="Auto"/><ColumnDefinition/><ColumnDefinition Width="Auto"/></Grid.ColumnDefinitions>{levels}{profiles}<Grid Grid.Column="2">{filter}</Grid>{end}</Grid>"#
    );
    let paused = tools.live("", |part| {
        view.paused
        .as_ref()
        .map(|(new, since)| {
            let latest = Kit::button(part, "Back to latest", Some("arrowup"), Look::Primary, true, true, Command::LogLatest, "");
            format!(
                r#"<Border Background="$accentSoft$" Padding="16,6,12,6"><Grid ColumnSpacing="10" Tag="align:row"><Grid.ColumnDefinitions><ColumnDefinition Width="Auto"/><ColumnDefinition/><ColumnDefinition Width="Auto"/></Grid.ColumnDefinitions>{}<StackPanel Grid.Column="1" Orientation="Horizontal" Spacing="6">{}{}</StackPanel><Grid Grid.Column="2">{latest}</Grid></Grid></Border>"#,
                Kit::icon("pause", 14, "$text2$"),
                Kit::bold_line(new, 13),
                Kit::line(since, 13, "$text2$", "")
            )
        })
        .unwrap_or_default()
    });
    let widths = if context.compact() {
        vec!["96", "72", "1*", "3*"]
    } else {
        vec!["104", "76", "140", "72", "*"]
    };
    let empty = Kit::empty(
        None,
        "No lines match",
        "Change the level, profile or text filter.",
    );
    let body = if live {
        let rows: String = view
            .rows
            .iter()
            .map(|row| {
                let level = match row.level {
                    Level::Warn => Kit::badge("Warning", Tone::Warning),
                    Level::Error => Kit::badge("Error", Tone::Error),
                    Level::Info => Kit::line("Info", 12, "$text3$", ""),
                };
                let profile = match &row.profile {
                    Some((name, colour)) => format!(
                        r#"<StackPanel Orientation="Horizontal" Spacing="8" Tag="align:row">{}{}</StackPanel>"#,
                        colour.map(Kit::swatch).unwrap_or_default(),
                        Kit::line(name, 13, "$text2$", "")
                    ),
                    None => Kit::line("Global", 13, "$text3$", ""),
                };
                let message = format!(
                    r#"<TextBlock Text="{}"{} VerticalAlignment="Center" ToolTipService.ToolTip="{0}"/>"#,
                    Kit::esc(&row.message),
                    Align::line(13)
                );
                let mut cells = vec![
                    Kit::line(&row.time, 13, "$text2$", ""),
                    level,
                    profile,
                    Kit::line(&row.source, 13, "$text2$", ""),
                    message,
                ];
                if context.compact() {
                    cells.remove(3);
                }
                Kit::row(
                    &widths,
                    &cells,
                    if row.level == Level::Error {
                        "$errBg$"
                    } else {
                        "Transparent"
                    },
                    "8,0,12,0",
                )
            })
            .collect();
        let labels: Vec<&str> = if context.compact() {
            vec!["Time", "Level", "Profile", "Message"]
        } else {
            vec!["Time", "Level", "Profile", "Source", "Message"]
        };
        format!(
            r#"{}<ScrollViewer x:Name="LogScroll" Grid.Row="2" VerticalScrollBarVisibility="Auto"><StackPanel Padding="8,4">{}</StackPanel></ScrollViewer>"#,
            format_args!(
                r#"<Grid Grid.Row="1">{}</Grid>"#,
                Kit::table_header(&widths, &labels, "16,0,12,0")
            ),
            if rows.is_empty() { empty } else { rows }
        )
    } else {
        let mut lines = String::new();
        for (index, row) in view.rows.iter().enumerate() {
            if let Some((_, title)) = view.days.iter().find(|(i, _)| *i == index) {
                lines.push_str(&Kit::text(
                    title,
                    12,
                    "$text2$",
                    r#"FontWeight="SemiBold" Margin="0,10,0,4""#,
                ));
            }
            let color = match row.level {
                Level::Error => "$danger$",
                Level::Warn => "$warn$",
                Level::Info => "$text$",
            };
            lines.push_str(&format!(
                r#"<TextBlock FontFamily="Cascadia Mono, Consolas" FontSize="12" TextTrimming="None" TextWrapping="NoWrap"><Run Text="{} {}" Foreground="$text3$"/><Run Text=" {:<5} " Foreground="{color}"/><Run Text="{:<13}{:<9}" Foreground="$text2$"/><Run Text="{}" Foreground="{color}"/></TextBlock>"#,
                row.day,
                row.time,
                row.level.to_string(),
                Kit::esc(&row.profile.as_ref().map_or("-".to_owned(), |(n, _)| n.clone())),
                Kit::esc(&row.source),
                Kit::esc(&row.message)
            ));
        }
        format!(
            r#"<ScrollViewer x:Name="LogScroll" Grid.Row="2" VerticalScrollBarVisibility="Auto" HorizontalScrollBarVisibility="Auto"><StackPanel Padding="16,4">{}</StackPanel></ScrollViewer>"#,
            if lines.is_empty() { empty } else { lines }
        )
    };
    let body = body
        .replace(r#"Grid.Row="1""#, r#"Grid.Row="0""#)
        .replace(r#"Grid.Row="2""#, r#"Grid.Row="1""#);
    // New lines arrive every second: the toolbar with its field stays in its own region.
    let mut out = Out::default();
    out.child(tools.finish(
        "LogTools",
        format!("<StackPanel>{toolbar}{paused}</StackPanel>"),
    ));
    out.child(Out::default().finish(
        "LogRows",
        format!(r#"<Grid><Grid.RowDefinitions><RowDefinition Height="Auto"/><RowDefinition/></Grid.RowDefinitions>{body}</Grid>"#),
    ));
    out.child(Out::default().finish("LogFoot", Kit::table_footer(&view.footer, &view.footer_end)));
    let card = out.finish(
        "Body",
        r#"<Border Background="$card$" BorderBrush="$border$" BorderThickness="1" CornerRadius="8"><Grid><Grid.RowDefinitions><RowDefinition Height="Auto"/><RowDefinition/><RowDefinition Height="Auto"/></Grid.RowDefinitions><ContentControl x:Name="LogTools" HorizontalContentAlignment="Stretch"/><ContentControl x:Name="LogRows" Grid.Row="1" HorizontalContentAlignment="Stretch" VerticalContentAlignment="Stretch"/><ContentControl x:Name="LogFoot" Grid.Row="2" HorizontalContentAlignment="Stretch"/></Grid></Border>"#.to_owned(),
    );
    PageMarkup {
        key,
        regions: vec![header, card],
        grow: Some(1),
    }
}
