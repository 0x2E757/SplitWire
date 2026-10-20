//! The profile page: its header, messages, chart and cards.

use super::route_groups::RouteTable;
use super::status::more;
use super::{PageContext, PageMarkup};
use crate::config::SettingsScope;
use crate::ipc::StepResult;
use crate::platform::window::markup::chart::Chart;
use crate::platform::window::markup::region::Out;
use crate::platform::window::markup::{Kit, Look, Tone};
use crate::presentation::pages::{InfoBar, ProfilePageView};
use crate::presentation::{Command, InputId, Page, SettingsEdit, StepTableView};

fn step_table(out: &mut Out, table: &StepTableView) -> String {
    let widths = ["24", "72", "1.6*", "92", "1.3*"];
    let rows: String = table
        .rows
        .iter()
        .map(|row| {
            let (tone, icon) = match row.result {
                StepResult::Done => (Tone::Accent, "check"),
                StepResult::Undone => (Tone::Neutral, "reset"),
                StepResult::Skipped => (Tone::Neutral, "chevr"),
                StepResult::Failed => (Tone::Error, "error"),
                StepResult::NotRun => (Tone::Neutral, ""),
            };
            let label = StepTableView::result_label(row.result);
            let badge = if icon.is_empty() {
                Kit::line(label, 11, "$text3$", "")
            } else {
                let color = match tone {
                    Tone::Accent => "$accent$",
                    Tone::Error => "$danger$",
                    _ => "$text2$",
                };
                Kit::badge_with(label, tone, &Kit::icon(icon, 12, color))
            };
            Kit::row(
                &widths,
                &[
                    Kit::line(&row.number.to_string(), 13, "$text3$", ""),
                    Kit::bold_line(row.id, 13),
                    Kit::line(row.what, 13, "$text2$", ""),
                    badge,
                    Kit::line(&row.detail, 13, "$text3$", ""),
                ],
                if row.result == StepResult::Failed {
                    "$errBg$"
                } else {
                    "Transparent"
                },
                "8,0,12,0",
            )
        })
        .collect();
    let copy = Kit::button(
        out,
        "Copy details",
        Some("copy"),
        Look::Default,
        true,
        true,
        Command::Copy(table.details.clone()),
        "",
    );
    Kit::card(
        &format!(
            r#"<StackPanel><Grid MinHeight="40" Padding="16,4,12,4" ColumnSpacing="12"><Grid.ColumnDefinitions><ColumnDefinition/><ColumnDefinition Width="Auto"/></Grid.ColumnDefinitions>{}<Grid Grid.Column="1">{copy}</Grid></Grid>{}<StackPanel Padding="8,4">{rows}</StackPanel>{}</StackPanel>"#,
            Kit::titled("Last Connect", 15, &table.caption, "$text3$", ""),
            Kit::table_header(
                &widths,
                &["#", "Step", "What it does", "Result", "Detail"],
                "16,0,12,0"
            ),
            Kit::table_footer(&table.footer, "")
        ),
        "0",
    )
}

fn cell(label: &str, value: &str, extra: &str) -> String {
    format!(
        r#"<StackPanel VerticalAlignment="Center">{}<StackPanel Orientation="Horizontal" Spacing="6" Tag="align:row">{}{extra}</StackPanel></StackPanel>"#,
        Kit::text(label, 12, "$text2$", ""),
        Kit::bold_line(value, 13)
    )
}

pub fn page(context: &PageContext, view: &ProfilePageView, key: String) -> PageMarkup {
    let name = view.name.clone();
    let mut regions = Vec::new();

    let mut out = Out::default();
    let toggle = Kit::toggle(
        &mut out,
        view.switch.on,
        view.switch.enabled,
        Command::SetConnected(name.clone(), !view.switch.on),
        "Connect",
        &view.switch.tip,
    );
    let toggle = super::status::stamped(&toggle, context);
    let end = format!(
        "{}{toggle}{}",
        Kit::live_pill(&mut out, &view.pill),
        more(&mut out, context, &name, false)
    );
    let header = context.header(&mut out, &name, Some(&view.subtitle), &end);
    regions.push(out.finish("Header", header));

    let mut out = Out::default();
    let mut messages = String::new();
    if let Some(bar) = &view.global_message {
        let open = Kit::button(
            &mut out,
            "Open Settings",
            None,
            Look::Default,
            true,
            true,
            Command::Go(Page::Settings),
            "",
        );
        messages.push_str(&Kit::infobar(Tone::Error, bar, &open));
    }
    if let Some(failure) = &view.failure {
        let retry = Kit::button(
            &mut out,
            "Retry",
            Some("reset"),
            Look::Default,
            true,
            true,
            Command::SetConnected(name.clone(), true),
            "",
        );
        messages.push_str(&Kit::infobar(
            Tone::Error,
            &InfoBar::new(&failure.title, &failure.message),
            &retry,
        ));
    }
    let alerts: String = view
        .alerts
        .iter()
        .map(|a| Kit::alert(&mut out, a, true, Some(Command::FollowAlert(a.id.clone()))))
        .collect();
    if !alerts.is_empty() {
        messages.push_str(&format!(r#"<StackPanel Spacing="4">{alerts}</StackPanel>"#));
    }
    if let Some(failure) = &view.failure {
        messages.push_str(&step_table(&mut out, failure));
    }
    regions.push(out.finish(
        "Messages",
        format!(r#"<StackPanel Spacing="14">{messages}</StackPanel>"#),
    ));

    let out = Out::default();
    let strip = &view.strip;
    let rtt = strip
        .rtt
        .as_ref()
        .map(|r| Kit::line(r, 12, "$text3$", ""))
        .unwrap_or_default();
    let cells = [
        cell("Handshake", &strip.handshake, ""),
        cell("Receive", &strip.receive, ""),
        cell("Send", &strip.send, ""),
        cell("Upstream DNS", &strip.upstream, &rtt),
    ];
    let shown = if context.compact() {
        &cells[..3]
    } else {
        &cells[..]
    };
    let columns: String =
        std::iter::repeat_n(r#"<ColumnDefinition Width="Auto"/>"#, shown.len()).collect();
    let cells: String = shown
        .iter()
        .enumerate()
        .map(|(i, c)| format!(r#"<Grid Grid.Column="{}">{c}</Grid>"#, i + 2))
        .collect();
    let strip_markup = Kit::card(
        &format!(
            r#"<Grid ColumnSpacing="24"><Grid.ColumnDefinitions><ColumnDefinition Width="Auto"/><ColumnDefinition/>{columns}</Grid.ColumnDefinitions>{}<StackPanel Grid.Column="1" VerticalAlignment="Center">{}{}</StackPanel>{cells}</Grid>"#,
            Kit::icon(
                "tunnel",
                20,
                if strip.connected {
                    "$accent$"
                } else {
                    "$text3$"
                }
            ),
            Kit::bold(&strip.endpoint, 13),
            Kit::text(&strip.route, 12, "$text3$", "")
        ),
        "16,12",
    );
    regions.push(out.finish("Strip", strip_markup));

    let height = if view.chart.series.is_some() {
        210
    } else {
        150
    };
    regions.push(Out::default().finish(
        "Chart",
        Chart::card(
            &view.chart_title,
            &view.chart_note,
            &view.chart,
            view.colour,
            height,
        ),
    ));

    let mut out = Out::default();
    let scope = SettingsScope::Profile(name.clone());
    let field = |key: &str| SettingsEdit::field(scope.clone(), key);
    let blocked = match &view.blocked {
        Some(blocked) => {
            let boot = Kit::toggle(
                &mut out,
                blocked.boot,
                true,
                Command::SettingToggle(field("connect_at_boot")),
                "",
                "",
            );
            let upstream = Kit::field(
                &mut out,
                InputId::Setting(field("upstream")),
                &blocked.upstream.value,
                &blocked.upstream.placeholder,
                None,
                Some(200),
                None,
                blocked.upstream.problem.is_some(),
            );
            let mut mtu = Kit::field(
                &mut out,
                InputId::Setting(field("mtu")),
                &blocked.mtu.value,
                &blocked.mtu.placeholder,
                None,
                Some(110),
                None,
                blocked.mtu.problem.is_some(),
            );
            if let Some(fix) = &blocked.mtu.fix {
                mtu.push_str(&Kit::button(
                    &mut out,
                    &format!("Use {fix}"),
                    None,
                    Look::Default,
                    true,
                    true,
                    Command::SettingUse(field("mtu"), fix.clone()),
                    "",
                ));
            }
            let code: String = blocked
                .lines
                .iter()
                .map(|(line, text, bad, comment)| {
                    let (background, color) = match (bad, comment) {
                        (true, _) => ("$errBg$", "$danger$"),
                        (false, true) => ("Transparent", "$text3$"),
                        _ => ("Transparent", "$text$"),
                    };
                    format!(
                        r#"<Grid Background="{background}" Tag="align:row"><Grid.ColumnDefinitions><ColumnDefinition Width="36"/><ColumnDefinition/></Grid.ColumnDefinitions><TextBlock Text="{line}" FontFamily="Cascadia Mono, Consolas" FontSize="12" Foreground="$text3$" TextAlignment="Right" Margin="0,0,10,0"/><TextBlock Grid.Column="1" Text="{}" FontFamily="Cascadia Mono, Consolas" FontSize="12" Foreground="{color}"/></Grid>"#,
                        Kit::esc(text)
                    )
                })
                .collect();
            format!(
                r#"<StackPanel Spacing="4">{}{}{}{}<StackPanel Orientation="Horizontal" Spacing="8" Margin="0,6,0,4" Tag="align:row">{}{}</StackPanel><Border Background="$ctrl$" BorderBrush="$ctrlBorder$" BorderThickness="1" CornerRadius="4" Padding="0,6"><StackPanel>{code}</StackPanel></Border></StackPanel>"#,
                Kit::text(
                    "Profile settings",
                    14,
                    "$text$",
                    r#"FontWeight="SemiBold" Margin="0,10,0,2""#
                ),
                Kit::scard(
                    &mut out,
                    &Kit::icon("power", 16, "$text2$"),
                    "Connect at boot",
                    &format!("Restore {name} when the service starts"),
                    None,
                    &boot,
                    false
                ),
                Kit::scard(
                    &mut out,
                    &Kit::icon("globe", 16, "$text2$"),
                    "Upstream DNS",
                    "Empty: the profile’s DNS",
                    blocked.upstream.problem.as_deref(),
                    &upstream,
                    blocked.upstream.problem.is_some()
                ),
                Kit::scard(
                    &mut out,
                    &Kit::icon("layout", 16, "$text2$"),
                    "MTU",
                    "Empty: the profile’s MTU; 1280–1500",
                    blocked.mtu.problem.as_deref(),
                    &mtu,
                    blocked.mtu.problem.is_some()
                ),
                Kit::icon("file", 14, "$text2$"),
                Kit::line(&blocked.file, 12, "$text2$", "")
            )
        }
        None => String::new(),
    };
    regions.push(out.finish("Blocked", blocked));

    let mut tools = Out::default();
    let add_field = Kit::field(
        &mut tools,
        InputId::ProfileAdd,
        &view.add_value,
        &format!("Add a domain or IPv4 address to {name}"),
        Some("plus"),
        None,
        None,
        view.add_verdict.bad(),
    );
    // The verdict enables Add as the entry is typed.
    let add = tools.live(r#"Grid.Column="1""#, |part| {
        Kit::button(
            part,
            "Add",
            None,
            Look::Default,
            false,
            !view.add_verdict.blocks(),
            Command::AddEntry(name.clone()),
            "",
        )
    });
    let search = Kit::field(
        &mut tools,
        InputId::ProfileSearch,
        &context.frame.input(&InputId::ProfileSearch),
        "Address or name",
        Some("search"),
        Some(220),
        None,
        false,
    );
    let edit = Kit::button(
        &mut tools,
        "Edit as text",
        Some("edit"),
        Look::Default,
        false,
        true,
        Command::EditListAsText(name.clone()),
        "",
    );
    let hint = view
        .add_verdict
        .message()
        .map(|m| {
            Kit::text(
                &format!("{}: {m}", view.add_value.trim()),
                12,
                if view.add_verdict.bad() {
                    "$danger$"
                } else {
                    "$text2$"
                },
                r#"Margin="16,0,16,6""#,
            )
        })
        .unwrap_or_default();
    let mut body = Out::default();
    let groups = RouteTable {
        profile: &name,
        groups: &view.groups,
        removable: true,
    }
    .markup(&mut body, context);
    let tools = tools.finish(
        "ListTools",
        format!(
            r#"<Grid Padding="16,8,12,8" ColumnSpacing="12" Tag="align:row"><Grid.ColumnDefinitions><ColumnDefinition MaxWidth="420"/><ColumnDefinition Width="Auto"/><ColumnDefinition Width="*"/><ColumnDefinition Width="Auto"/><ColumnDefinition Width="Auto"/></Grid.ColumnDefinitions>{add_field}{add}<Grid Grid.Column="3">{search}</Grid><Grid Grid.Column="4">{edit}</Grid></Grid>"#
        ),
    );
    // The routes' ages change every second; the fields stay in their own region.
    let body = body.finish(
        "ListBody",
        format!(
            r#"<StackPanel>{hint}{}<StackPanel Padding="8,4">{groups}</StackPanel>{}</StackPanel>"#,
            RouteTable::header(context.compact()),
            Kit::table_footer(&view.footer, "")
        ),
    );
    let mut out = Out::default();
    out.child(tools);
    out.child(body);
    let list = Kit::card(
        r#"<StackPanel><ContentControl x:Name="ListTools" HorizontalContentAlignment="Stretch"/><ContentControl x:Name="ListBody" HorizontalContentAlignment="Stretch"/></StackPanel>"#,
        "0",
    );
    regions.push(out.finish("List", list));

    let mut out = Out::default();
    let settings = if view.blocked.is_some() {
        String::new()
    } else {
        let boot = Kit::toggle(
            &mut out,
            view.boot,
            true,
            Command::SettingToggle(field("connect_at_boot")),
            "",
            "",
        );
        let upstream = Kit::field(
            &mut out,
            InputId::Setting(field("upstream")),
            &view.upstream.value,
            &view.upstream.placeholder,
            None,
            Some(200),
            None,
            view.upstream.problem.is_some(),
        );
        let mtu = Kit::field(
            &mut out,
            InputId::Setting(field("mtu")),
            &view.mtu.value,
            &view.mtu.placeholder,
            None,
            Some(110),
            None,
            view.mtu.problem.is_some(),
        );
        let replace = Kit::button(
            &mut out,
            "Replace .conf…",
            Some("import"),
            Look::Default,
            false,
            true,
            Command::PickFiles {
                replace: Some(name.clone()),
            },
            "",
        );
        let remove = Kit::button(
            &mut out,
            "Remove…",
            Some("trash"),
            Look::Danger,
            false,
            true,
            Command::AskRemove(name.clone()),
            "",
        );
        let body = format!(
            "{}{}{}{}{}",
            Kit::xrow(
                &mut out,
                "Connect at boot",
                &format!("Restore {name} when the service starts"),
                &boot,
                false,
                false,
                None,
                true
            ),
            Kit::xrow(
                &mut out,
                "Upstream DNS",
                "Empty: the profile’s DNS; 1–4 IPv4[:port]",
                &upstream,
                view.upstream.changed,
                view.upstream.problem.is_some(),
                view.upstream.problem.as_deref(),
                false
            ),
            Kit::xrow(
                &mut out,
                "MTU",
                "Empty: the profile’s MTU; 1280–1500",
                &mtu,
                view.mtu.changed,
                view.mtu.problem.is_some(),
                view.mtu.problem.as_deref(),
                false
            ),
            Kit::xrow(
                &mut out,
                "WireGuard configuration",
                &format!(
                    "Imported {}. Replacing keeps the list and these settings.",
                    view.imported
                ),
                &replace,
                false,
                false,
                None,
                false
            ),
            Kit::xrow(
                &mut out,
                "Remove this profile",
                "Disconnects it and deletes its keys, list and settings.",
                &remove,
                false,
                false,
                None,
                false
            ),
        );
        Kit::expander(
            &mut out,
            &format!("profile:settings:{name}"),
            &Kit::icon("gear", 16, "$text2$"),
            "Profile settings",
            "Changes apply at the next Connect",
            "",
            &view.settings_summary,
            view.settings_open,
            false,
            &body,
        )
    };
    regions.push(out.finish("Settings", settings));

    PageMarkup {
        key,
        regions,
        grow: None,
    }
}
