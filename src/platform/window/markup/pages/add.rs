//! Add profile and the first run, and the import review.

use super::{PageContext, PageMarkup};
use crate::platform::window::markup::region::{Event, Out};
use crate::platform::window::markup::{Kit, Look, Tone};
use crate::presentation::pages::{AddView, DropState, InfoBar, ReviewView};
use crate::presentation::{Command, Dot, InputId, Pill};
use crate::profile::CheckLevel;

pub fn page(context: &PageContext, view: &AddView, key: String) -> PageMarkup {
    let mut out = Out::default();
    let end = if view.first_run {
        Kit::pill(&Pill::new(Dot::Off, "No profile", None))
    } else {
        String::new()
    };
    let header = context.header(&mut out, &view.title, Some(&view.subtitle), &end);
    let header = out.finish("Header", header);

    let mut out = Out::default();
    let zone = out.bind(Event::Drop, Command::DragLeave);
    let (stroke, fill, icon, icon_color) = match &view.state {
        DropState::Idle => ("$ctrlBorder$", "$card$", "import", "$text2$"),
        DropState::Over(_) => ("$accent$", "$accentSoft$", "import", "$accent$"),
        DropState::Bad(_) => ("$danger$", "$errBg$", "error", "$danger$"),
    };
    let body = match &view.state {
        DropState::Idle => String::new(),
        DropState::Over(names) => {
            let chips: String = names
                .iter()
                .map(|n| {
                    format!(
                        r#"<Border Height="24" CornerRadius="12" BorderBrush="$border$" BorderThickness="1" Background="$card$" Padding="10,0" Tag="align:row"><StackPanel Orientation="Horizontal" Spacing="6">{}{}</StackPanel></Border>"#,
                        Kit::icon("file", 12, "$text2$"),
                        Kit::line(n, 12, "$text2$", "")
                    )
                })
                .collect();
            format!(
                r#"<StackPanel Orientation="Horizontal" Spacing="8" HorizontalAlignment="Center">{chips}</StackPanel>"#
            )
        }
        DropState::Bad(rows) => {
            let rows: String = rows
                .iter()
                .map(|(file, outcome, ok)| {
                    format!(
                        r#"<Grid ColumnSpacing="10" Padding="12,8" Background="$card$" BorderBrush="{}" BorderThickness="1" CornerRadius="6" Tag="align:row"><Grid.ColumnDefinitions><ColumnDefinition Width="Auto"/><ColumnDefinition/></Grid.ColumnDefinitions>{}<StackPanel Grid.Column="1" Tag="align:block">{}{}</StackPanel></Grid>"#,
                        if *ok { "$border$" } else { "$errBorder$" },
                        Kit::icon(if *ok { "ok" } else { "error" }, 16, if *ok { "$accent$" } else { "$danger$" }),
                        Kit::bold(file, 13),
                        Kit::text(outcome, 12, "$text2$", r#"TextWrapping="Wrap""#)
                    )
                })
                .collect();
            let review = if view.review_others {
                Kit::button(
                    &mut out,
                    "Review the others",
                    None,
                    Look::Primary,
                    true,
                    true,
                    Command::Go(crate::presentation::Page::Review),
                    "",
                )
            } else {
                String::new()
            };
            format!(
                r#"<StackPanel Spacing="6" Width="520">{rows}<Grid HorizontalAlignment="Right">{review}</Grid></StackPanel>"#
            )
        }
    };
    let choose = Kit::button(
        &mut out,
        "Choose files…",
        Some("folder"),
        Look::Primary,
        false,
        !view.full,
        Command::PickFiles { replace: None },
        "",
    );
    let zone_markup = format!(
        r#"<Grid x:Name="{zone}" AllowDrop="True" Background="Transparent" MinHeight="380"><Rectangle RadiusX="8" RadiusY="8" Stroke="{stroke}" StrokeThickness="2" StrokeDashArray="4,3" Fill="{fill}"/><StackPanel HorizontalAlignment="Center" VerticalAlignment="Center" Spacing="10" Padding="24,40" MaxWidth="640"><Border Width="56" Height="56" CornerRadius="28" Background="$subtle$" HorizontalAlignment="Center">{}</Border>{}{}{body}<Grid HorizontalAlignment="Center" Margin="0,4,0,0">{choose}</Grid><StackPanel Orientation="Horizontal" Spacing="6" HorizontalAlignment="Center" Tag="align:row">{}<TextBlock Text="SplitWire.exe import Contoso.conf" FontFamily="Cascadia Mono, Consolas" FontSize="12" Foreground="$text3$" LineHeight="16" LineStackingStrategy="BlockLineHeight" VerticalAlignment="Center"/></StackPanel></StackPanel></Grid>"#,
        Kit::icon(icon, 24, icon_color),
        Kit::text(
            &view.zone_title,
            20,
            "$text$",
            r#"FontWeight="SemiBold" HorizontalAlignment="Center" TextAlignment="Center""#
        ),
        Kit::text(
            &view.zone_text,
            13,
            "$text2$",
            r#"TextWrapping="Wrap" TextAlignment="Center" HorizontalAlignment="Center""#
        ),
        Kit::line("Or in a terminal:", 12, "$text3$", "")
    );
    let zone = out.finish("Zone", zone_markup);

    let facts: String = view
        .facts
        .iter()
        .enumerate()
        .map(|(i, fact)| {
            format!(
                r#"<Border Grid.Column="{i}" Background="$card$" BorderBrush="$border$" BorderThickness="1" CornerRadius="8" Padding="14,12"><Grid ColumnSpacing="12"><Grid.ColumnDefinitions><ColumnDefinition Width="16"/><ColumnDefinition/></Grid.ColumnDefinitions><Grid.RowDefinitions><RowDefinition Height="Auto"/><RowDefinition Height="Auto"/></Grid.RowDefinitions>{}<Grid Grid.Row="1" Grid.Column="1">{}</Grid></Grid></Border>"#,
                Kit::first_line(&Kit::icon(fact.icon, 16, "$text2$"), &fact.title, 13, 12),
                Kit::text(&fact.text, 12, "$text2$", r#"TextWrapping="Wrap""#)
            )
        })
        .collect();
    let facts = Out::default().finish(
        "Facts",
        format!(
            r#"<Grid ColumnSpacing="14"><Grid.ColumnDefinitions><ColumnDefinition/><ColumnDefinition/><ColumnDefinition/></Grid.ColumnDefinitions>{facts}</Grid>"#
        ),
    );
    PageMarkup {
        key,
        regions: vec![header, zone, facts],
        grow: Some(1),
    }
}

fn check_icon(level: CheckLevel) -> String {
    match level {
        CheckLevel::Ok => Kit::icon("ok", 14, "$accent$"),
        CheckLevel::Info => Kit::icon("info", 14, "$text3$"),
        CheckLevel::Warn => Kit::icon("alert", 14, "$warn$"),
        CheckLevel::Bad => Kit::icon("error", 14, "$danger$"),
    }
}

pub fn review(context: &PageContext, view: &ReviewView, key: String) -> PageMarkup {
    let mut out = Out::default();
    let cancel = Kit::button(
        &mut out,
        "Cancel",
        None,
        Look::Default,
        false,
        true,
        Command::CancelImport,
        "",
    );
    let import = Kit::button(
        &mut out,
        &view.import_label,
        Some("check"),
        Look::Primary,
        false,
        view.can_import,
        Command::Import,
        "",
    );
    let header = context.header(
        &mut out,
        &view.title,
        Some(&view.subtitle),
        &format!("{cancel}{import}"),
    );
    let header = out.finish("Header", header);

    let mut notes = String::new();
    if let Some(failure) = &view.failure {
        notes.push_str(&Kit::infobar(
            Tone::Error,
            &InfoBar::new("The import stopped", failure),
            "",
        ));
    }
    if let Some(note) = &view.note {
        notes.push_str(&Kit::text(note, 12, "$text3$", r#"TextWrapping="Wrap""#));
    }
    let notes = Out::default().finish(
        "Note",
        format!(r#"<StackPanel Spacing="8">{notes}</StackPanel>"#),
    );

    let mut out = Out::default();
    let files: String = view
        .files
        .iter()
        .map(|file| {
            let badge = match &file.replace {
                Some(name) => Kit::badge(&format!("Replace {name}"), Tone::Accent),
                None => Kit::badge("New profile", Tone::Accent),
            };
            let name_field = if file.replace.is_some() {
                format!(
                    r#"<Border Width="260" Height="32" Background="$subtle$" CornerRadius="4" Padding="10,0" Tag="align:row">{}</Border>"#,
                    Kit::line(&file.name, 13, "$disabled$", "")
                )
            } else {
                Kit::field(&mut out, InputId::ReviewName(file.index), &file.name, "Profile name", None, Some(260), None, file.name_problem.is_some())
            };
            let hint = out.live(r#"VerticalAlignment="Center""#, |_| match &file.name_problem {
                Some(problem) => Kit::line(problem, 12, "$danger$", ""),
                None => Kit::line(&file.name_hint, 12, "$text3$", ""),
            });
            // The title and the sentence follow the name as it is typed.
            let head = out.live("", |_| {
                format!(
                    r#"<Grid ColumnSpacing="12"><Grid.ColumnDefinitions><ColumnDefinition Width="Auto"/><ColumnDefinition/><ColumnDefinition Width="Auto"/></Grid.ColumnDefinitions>{}<StackPanel Grid.Column="1"><StackPanel Orientation="Horizontal" Spacing="8" Tag="align:row">{}{}</StackPanel>{}</StackPanel><Grid Grid.Column="2" VerticalAlignment="Top">{badge}</Grid></Grid>"#,
                    Kit::icon("import", 24, "$text2$"),
                    Kit::swatch(file.colour),
                    Kit::line(if file.name.is_empty() { &file.file_name } else { &file.name }, 20, "$text$", r#" FontWeight="SemiBold""#),
                    Kit::text(&file.sentence, 13, "$text2$", r#"TextWrapping="Wrap""#)
                )
            });
            // The colours others use follow the typed name.
            let colours = out.live("", |part| {
                let colours: String = file
                .colours
                .iter()
                .map(|(pin, owner)| {
                    let name = part.bind(Event::Click, Command::ReviewColour(file.index, *pin));
                    let tip = owner
                        .as_ref()
                        .map_or_else(|| format!("Colour {}", pin.number()), |o| format!("Also used by {o}"));
                    let selected = *pin == file.colour;
                    format!(
                        r#"<Button x:Name="{name}" Width="26" Height="26" MinHeight="26" Padding="0" CornerRadius="6" Background="{}" BorderBrush="{}" ToolTipService.ToolTip="{}">{}</Button>"#,
                        if selected { "$accentSoft$" } else { "Transparent" },
                        if selected { "$accent$" } else { "Transparent" },
                        Kit::esc(&tip),
                        Kit::swatch(*pin)
                    )
                })
                .collect();
                format!(r#"<StackPanel Orientation="Horizontal" Spacing="4">{colours}</StackPanel>"#)
            });
            let label = |text: &str| Kit::line(text, 13, "$text2$", r#" Width="60""#);
            let summary = Kit::card(
                &format!(
                    r#"<StackPanel Spacing="12">{head}<StackPanel Orientation="Horizontal" Spacing="12" Tag="align:row">{}{name_field}{hint}</StackPanel><StackPanel Orientation="Horizontal" Spacing="12" Tag="align:row">{}{colours}{}</StackPanel></StackPanel>"#,
                    label("Name"),
                    label("Colour"),
                    Kit::line("Marks the profile in the nav, charts and logs; changeable later", 12, "$text3$", "")
                ),
                "16,14",
            );
            // What the name and the preview change, without a gap when there is nothing.
            let notes = out.live("", |_| {
                let reconnect = file
                    .reconnect
                    .as_ref()
                    .map(|bar| format!(r#"<Grid Margin="0,8,0,0">{}</Grid>"#, Kit::infobar(Tone::Warning, bar, "")))
                    .unwrap_or_default();
                let waiting = if file.waiting {
                    Kit::text("Reading the file…", 12, "$text3$", r#"Margin="0,8,0,0""#)
                } else {
                    String::new()
                };
                format!("<StackPanel>{reconnect}{waiting}</StackPanel>")
            });
            // The rows, and which expanders are open, follow the name as it is typed and the file
            // as it is read again.
            let details = out.live(r#"Margin="0,8,0,0""#, |part| {
                let values_body: String = file
                    .values
                    .iter()
                    .enumerate()
                    .map(|(i, (field, value, new))| {
                        let control = match new {
                            Some(new) => format!(
                                r#"<StackPanel Orientation="Horizontal" Spacing="8" Tag="align:row">{}{}{}</StackPanel>"#,
                                Kit::line(value, 13, "$text3$", ""),
                                Kit::line("→", 13, "$text3$", ""),
                                Kit::bold_line(new, 13)
                            ),
                            None => Kit::line(value, 13, if value.contains("present") || value.contains("encrypted") { "$text2$" } else { "$text$" }, r#" FontWeight="SemiBold""#),
                        };
                        Kit::xrow(part, field, "", &control, false, false, None, i == 0)
                    })
                    .collect();
                let id = format!("review:{}", file.index);
                let values = Kit::expander(
                    part,
                    &format!("{id}:values"),
                    &Kit::icon("list", 16, "$text2$"),
                    if file.replace.is_some() { "Changes" } else { "Values" },
                    if file.replace.is_some() {
                        "Only what differs from the stored profile; keys are compared, never shown"
                    } else {
                        "Read from the file; keys are never shown"
                    },
                    "",
                    &format!("{} rows", file.values.len()),
                    context.frame.open(&format!("{id}:values"), view.files.len() == 1 || file.replace.is_some()),
                    false,
                    &values_body,
                );
                let checks_body: String = file
                    .checks
                    .iter()
                    .enumerate()
                    .map(|(i, (level, title, detail))| {
                        // The icon in the 48 px before the texts, on the title's line.
                        let label = format!(
                            r#"<StackPanel Orientation="Horizontal" Spacing="14" Margin="-28,0,0,0" Tag="align:row">{}{}</StackPanel>"#,
                            check_icon(*level),
                            Kit::line(title, 13, "$text$", "")
                        );
                        Kit::xrow_with(part, &label, detail, "", false, false, None, i == 0)
                    })
                    .collect();
                let blocked = file.checks.iter().any(|c| c.0 == CheckLevel::Bad);
                let checks = Kit::expander(
                    part,
                    &format!("{id}:checks"),
                    &Kit::icon("shield", 16, "$text2$"),
                    "Checks against your other profiles",
                    "Name, tunnel, endpoint and DNS against the profiles you already have",
                    "",
                    &file.checks_summary,
                    context.frame.open(&format!("{id}:checks"), file.checks_open),
                    blocked,
                    &checks_body,
                );
                let ignored_body: String = file
                    .ignored
                    .iter()
                    .enumerate()
                    .map(|(i, (line, why))| {
                        let label = Kit::text(line, 12, "$text$", r#"FontFamily="Cascadia Mono, Consolas""#);
                        Kit::xrow_with(part, &label, why, "", false, false, None, i == 0)
                    })
                    .collect();
                let ignored = Kit::expander(
                    part,
                    &format!("{id}:ignored"),
                    &Kit::icon("eyeoff", 16, "$text2$"),
                    "Ignored lines",
                    "Lines SplitWire reads past, and why",
                    "",
                    &file.ignored.len().to_string(),
                    context.frame.open(&format!("{id}:ignored"), false),
                    false,
                    &ignored_body,
                );
                format!(r#"<StackPanel Spacing="4">{values}{checks}{ignored}</StackPanel>"#)
            });
            format!(
                r#"<StackPanel>{summary}{notes}{details}</StackPanel>"#
            )
        })
        .collect();
    let files = out.finish(
        "Files",
        format!(r#"<StackPanel Spacing="20">{files}</StackPanel>"#),
    );
    PageMarkup {
        key,
        regions: vec![header, notes, files],
        grow: None,
    }
}
