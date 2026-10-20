//! Lists: Rows (a group per profile, its lines and an Add field) or Text (the combined
//! text in a monospace editor with a line gutter, Revert and Save).

use super::{PageContext, PageMarkup};
use crate::platform::window::markup::region::{Event, Out};
use crate::platform::window::markup::{Align, Kit, Look, Tone};
use crate::presentation::pages::{InfoBar, ListGroupView, ListRowView, ListsPageView};
use crate::presentation::{Command, InputId, ListsMode, Page};

/// One profile's group: its head, its lines and its Add field. The lines (an entry typed for
/// another profile marks the line it collides with), the Add button and the verdict follow the
/// typed entry, so they are live parts.
fn group(out: &mut Out, group: &ListGroupView, searching: bool, widths: &[&str]) -> String {
    let toggle = out.bind(
        Event::Click,
        Command::Toggle(format!("lists:{}", group.name)),
    );
    let head = format!(
        r#"<Button x:Name="{toggle}" HorizontalAlignment="Stretch" HorizontalContentAlignment="Stretch" Background="Transparent" BorderThickness="0" MinHeight="40" Padding="8,0" Margin="0,4,0,0"><StackPanel Orientation="Horizontal" Spacing="10" Tag="align:row">{}{}{}{}</StackPanel></Button>"#,
        Kit::icon(if group.open { "chev" } else { "chevr" }, 12, "$text2$"),
        Kit::pname(group.colour, &group.name),
        Kit::dot(group.dot),
        Kit::line(&group.caption, 12, "$text3$", "")
    );
    if !group.open {
        return head;
    }
    let lines = out.live("", |part| {
        let lines: String = group
            .rows
            .iter()
            .map(|row| line(part, group, row, widths))
            .collect();
        if lines.is_empty() {
            Kit::row(
                &["*"],
                &[Kit::line("No matches", 13, "$text3$", "")],
                "Transparent",
                "20,0,12,0",
            )
        } else {
            format!("<StackPanel>{lines}</StackPanel>")
        }
    });
    let add = if searching {
        String::new()
    } else {
        let id = InputId::ListsAdd(group.name.clone());
        let field = Kit::field(
            out,
            id,
            &group.add_value,
            &format!("Add to {}", group.name),
            Some("plus"),
            Some(280),
            None,
            group.add_verdict.bad(),
        );
        let button = out.live("", |part| {
            Kit::button(
                part,
                "Add",
                None,
                Look::Default,
                true,
                !group.add_verdict.blocks(),
                Command::AddEntry(group.name.clone()),
                "",
            )
        });
        let message = out.live(r#"VerticalAlignment="Center""#, |_| {
            let (message, color) = match group.add_verdict.message() {
                Some(m) if group.add_verdict.bad() => (m, "$danger$"),
                Some(m) => (m, "$accent$"),
                None => (
                    "a domain (and every name under it), an IPv4 address or network",
                    "$text3$",
                ),
            };
            Kit::line(message, 12, color, "")
        });
        format!(
            r#"<StackPanel Orientation="Horizontal" Spacing="8" Margin="20,6,12,8" Tag="align:row">{field}{button}{message}</StackPanel>"#
        )
    };
    format!(
        r#"{head}<Border BorderBrush="$divider$" BorderThickness="1,0,0,0" Margin="17,0,0,0"><StackPanel>{lines}{add}</StackPanel></Border>"#
    )
}

fn line(out: &mut Out, group: &ListGroupView, row: &ListRowView, widths: &[&str]) -> String {
    let icon = if row.comment {
        String::new()
    } else {
        Kit::icon(if row.domain { "globe" } else { "routes" }, 14, "$text3$")
    };
    let (text_color, font) = if row.comment {
        (
            "$text3$",
            r#"FontFamily="Cascadia Mono, Consolas" FontSize="12""#.to_owned() + &Align::line(12),
        )
    } else {
        ("$text$", Align::line(13))
    };
    let note = if row.clash {
        Kit::line(
            "the entry typed for another profile collides with this line",
            12,
            "$danger$",
            "",
        )
    } else {
        row.note
            .as_ref()
            .map(|n| {
                let color = if n == "static" { "$text3$" } else { "$accent$" };
                format!(
                    r#"<TextBlock Text="{}" FontSize="12" Foreground="{color}"{} VerticalAlignment="Center" ToolTipService.ToolTip="{0}"/>"#,
                    Kit::esc(n),
                    Align::line(12)
                )
            })
            .unwrap_or_default()
    };
    let remove = Kit::button(
        out,
        "",
        Some("x"),
        Look::Subtle,
        true,
        true,
        Command::RemoveLine {
            profile: group.name.clone(),
            line: row.line,
            text: row.text.clone(),
        },
        "Remove the line",
    );
    Kit::row(
        widths,
        &[
            icon,
            format!(
                r#"<TextBlock Text="{}" Foreground="{text_color}" VerticalAlignment="Center" {font}/>"#,
                Kit::esc(&row.text)
            ),
            Kit::line(&row.line.to_string(), 13, "$text3$", ""),
            note,
            Kit::line(
                &row.routes,
                13,
                "$text2$",
                r#" HorizontalAlignment="Right""#,
            ),
            remove,
        ],
        if row.clash { "$errBg$" } else { "Transparent" },
        "20,0,12,0",
    )
}

fn rows(out: &mut Out, context: &PageContext, view: &ListsPageView) -> String {
    let widths = ["18", "*", "40", "1.3*", "56", "26"];
    let search = Kit::field(
        out,
        InputId::ListsSearch,
        &context.frame.input(&InputId::ListsSearch),
        "Search entries",
        Some("search"),
        Some(240),
        None,
        false,
    );
    let searching = !context.frame.input(&InputId::ListsSearch).trim().is_empty();
    // The groups follow the search as it is typed.
    let groups = out.live("", |part| {
        let groups: String = view
            .groups
            .iter()
            .map(|g| group(part, g, searching, &widths))
            .collect();
        format!(r#"<StackPanel Padding="8,4">{groups}</StackPanel>"#)
    });
    format!(
        r#"<Border Background="$card$" BorderBrush="$border$" BorderThickness="1" CornerRadius="8"><Grid><Grid.RowDefinitions><RowDefinition Height="Auto"/><RowDefinition Height="Auto"/><RowDefinition/></Grid.RowDefinitions><Grid Padding="16,4,12,4" MinHeight="40" ColumnSpacing="12" Tag="align:row"><Grid.ColumnDefinitions><ColumnDefinition/><ColumnDefinition Width="Auto"/></Grid.ColumnDefinitions>{}<Grid Grid.Column="1">{search}</Grid></Grid><Grid Grid.Row="1">{}</Grid><ScrollViewer Grid.Row="2" VerticalScrollBarVisibility="Auto">{groups}</ScrollViewer></Grid></Border>"#,
        Kit::line(
            "One profile per entry; a more specific entry wins while both profiles are connected.",
            12,
            "$text2$",
            ""
        ),
        Kit::table_header(
            &widths,
            &["", "Entry", "Line", "Note", "Routes", ""],
            "46,0,12,0"
        )
    )
}

fn text(out: &mut Out, view: &ListsPageView) -> String {
    let text = &view.text;
    let mut marks: Vec<(usize, &str)> = text
        .section_lines
        .iter()
        .map(|l| (*l, "$accent$"))
        .collect();
    marks.extend(text.bad_lines.iter().map(|l| (*l, "$danger$")));
    let editor = Kit::editor(
        out,
        InputId::ListsText,
        &text.text,
        text.line_count,
        &marks,
        None,
    );
    // Revert, Save and the status follow the typed text.
    let foot = out.live("", |part| {
        let revert = Kit::button(
            part,
            "Revert",
            None,
            Look::Default,
            true,
            text.dirty,
            Command::RevertAllLists,
            "",
        );
        let save = Kit::button(
            part,
            "Save",
            None,
            Look::Primary,
            true,
            text.dirty,
            Command::SaveAllLists,
            "",
        );
        let status = if text.skipped.is_empty() {
            Kit::line(
                if text.dirty {
                    "Unsaved changes"
                } else {
                    "No changes"
                },
                12,
                "$text3$",
                "",
            )
        } else {
            format!(
                r#"<StackPanel Orientation="Horizontal" Spacing="8" Tag="align:row">{}{}</StackPanel>"#,
                Kit::icon("info", 14, "$accent$"),
                Kit::line(&text.skipped.join(" · "), 12, "$text2$", "")
            )
        };
        format!(
            r#"<Grid ColumnSpacing="8" Tag="align:row"><Grid.ColumnDefinitions><ColumnDefinition/><ColumnDefinition Width="Auto"/><ColumnDefinition Width="Auto"/></Grid.ColumnDefinitions>{status}<Grid Grid.Column="1">{revert}</Grid><Grid Grid.Column="2">{save}</Grid></Grid>"#
        )
    });
    format!(
        r#"<Border Background="$card$" BorderBrush="$border$" BorderThickness="1" CornerRadius="8"><Grid><Grid.RowDefinitions><RowDefinition Height="Auto"/><RowDefinition/><RowDefinition Height="Auto"/></Grid.RowDefinitions><Grid Padding="16,8,12,8">{}</Grid><Grid Grid.Row="1" Padding="12,0,12,8">{editor}</Grid><Border Grid.Row="2" BorderBrush="$divider$" BorderThickness="0,1,0,0" MinHeight="40" Padding="20,4,12,4">{foot}</Border></Grid></Border>"#,
        Kit::text(
            "All lists as one text, in the profiles’ order. Each [profile …] section stays exactly once; the files are validated together and written only if nothing blocks.",
            12,
            "$text2$",
            r#"TextWrapping="Wrap""#
        )
    )
}

pub fn page(context: &PageContext, view: &ListsPageView, key: String) -> PageMarkup {
    let mode = context.frame.lists_mode;
    let mut out = Out::default();
    let seg = Kit::segmented(
        &mut out,
        vec![
            (
                "Rows".into(),
                mode == ListsMode::Rows,
                Command::ListsMode(ListsMode::Rows),
            ),
            (
                "Text".into(),
                mode == ListsMode::Text,
                Command::ListsMode(ListsMode::Text),
            ),
        ],
    );
    let header = context.header(&mut out, "Lists", Some(&view.subtitle), &seg);
    let header = out.finish("Header", header);

    let mut out = Out::default();
    let mut messages = String::new();
    if let Some(bar) = &view.message {
        let open = Kit::button(
            &mut out,
            "Open settings",
            None,
            Look::Default,
            true,
            true,
            Command::Go(Page::Settings),
            "",
        );
        messages.push_str(&Kit::infobar(Tone::Error, bar, &open));
    }
    if mode == ListsMode::Text
        && let Some(refused) = &view.text.refused
    {
        messages.push_str(&Kit::infobar(
            Tone::Error,
            &InfoBar::new(&refused.title, &refused.message),
            "",
        ));
    }
    let message = out.finish(
        "Message",
        format!(r#"<StackPanel Spacing="14">{messages}</StackPanel>"#),
    );

    let mut out = Out::default();
    let body = match mode {
        ListsMode::Rows => rows(&mut out, context, view),
        ListsMode::Text => text(&mut out, view),
    };
    let body = out.finish(
        if mode == ListsMode::Rows {
            "Rows"
        } else {
            "Text"
        },
        body,
    );
    PageMarkup {
        key,
        regions: vec![header, message, body],
        grow: Some(2),
    }
}
