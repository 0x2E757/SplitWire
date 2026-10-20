//! Status and the service-down state.

use super::{PageContext, PageMarkup};
use crate::platform::window::markup::chart::Chart;
use crate::platform::window::markup::menu::Menu;
use crate::platform::window::markup::region::{Event, Out};
use crate::platform::window::markup::{Kit, Look, Tone};
use crate::presentation::pages::{
    BigCard, InfoBar, ServiceDownView, SlimCard, Stat, StatusPageView, SwitchView,
};
use crate::presentation::{AlertView, Command, Page, Pill};
use crate::profile::Pin;

fn alerts(out: &mut Out, alerts: &[AlertView]) -> String {
    alerts
        .iter()
        .map(|a| Kit::alert(out, a, true, Some(Command::FollowAlert(a.id.clone()))))
        .collect()
}

/// A card's name: the swatch and the name, a link to the profile page.
fn name_button(out: &mut Out, pin: Pin, name: &str, size: u32) -> String {
    let element = out.bind(Event::Click, Command::Go(Page::Profile(name.to_owned())));
    format!(
        r#"<Button x:Name="{element}" Background="Transparent" BorderThickness="0" Padding="0" MinHeight="0"><StackPanel Orientation="Horizontal" Spacing="8" Tag="align:row">{}{}</StackPanel></Button>"#,
        Kit::swatch(pin),
        Kit::line(name, size, "$text$", r#" FontWeight="SemiBold""#)
    )
}

/// The switch and the "…" menu at a card's end.
fn controls(out: &mut Out, context: &PageContext, switch: &SwitchView) -> String {
    let toggle = Kit::toggle(
        out,
        switch.on,
        switch.enabled,
        Command::SetConnected(switch.name.clone(), !switch.on),
        "",
        &switch.tip,
    );
    let more = more(out, context, &switch.name, true);
    format!("{}{more}", stamped(&toggle, context))
}

/// A Connect switch carries the session's switch revision, so a refused Connect reloads it.
pub fn stamped(toggle: &str, context: &PageContext) -> String {
    toggle.replacen(
        "<ToggleSwitch ",
        &format!(r#"<ToggleSwitch Tag="s{}" "#, context.frame.switches),
        1,
    )
}

/// The "…" button with the profile menu.
pub fn more(out: &mut Out, context: &PageContext, name: &str, small: bool) -> String {
    let menu = context.frame.menus.iter().find(|m| m.name == name);
    let flyout = menu
        .map(|m| Menu::flyout(out, m, "Button.Flyout"))
        .unwrap_or_default();
    let element = out.name();
    Kit::button_named(
        &element,
        "",
        Some("more"),
        Look::Subtle,
        small,
        true,
        "Profile menu",
        &flyout,
    )
}

fn toolbar(left: &str, right: &str) -> String {
    format!(
        r#"<Grid MinHeight="48" Padding="16,4,12,4" ColumnSpacing="12"><Grid.ColumnDefinitions><ColumnDefinition Width="Auto"/><ColumnDefinition/><ColumnDefinition Width="Auto"/></Grid.ColumnDefinitions><StackPanel Orientation="Horizontal" Spacing="12" VerticalAlignment="Center" Tag="align:row">{left}</StackPanel><StackPanel Grid.Column="2" Orientation="Horizontal" Spacing="12" VerticalAlignment="Center" Tag="align:row">{right}</StackPanel></Grid>"#
    )
}

/// A card's parts that change on their own are child regions: the header line, the stats, the
/// chart and the alerts. A second's new numbers then reload only what shows them, not the switch
/// or an open menu.
fn child(out: &mut Out, host: String, markup: String, build: Out) -> String {
    out.child(build.finish(&host, markup));
    format!(r#"<ContentControl x:Name="{host}" HorizontalContentAlignment="Stretch"/>"#)
}

fn alerts_part(out: &mut Out, host: String, list: &[AlertView], padding: &str) -> String {
    let mut part = Out::default();
    let markup = alerts(&mut part, list);
    let markup = if markup.is_empty() {
        String::new()
    } else {
        format!(r#"<StackPanel Spacing="4" Padding="{padding}">{markup}</StackPanel>"#)
    };
    child(out, host, markup, part)
}

fn big(out: &mut Out, context: &PageContext, card: &BigCard, index: usize) -> String {
    let mut head = Out::default();
    let name = name_button(&mut head, card.colour, &card.name, 15);
    let note = card
        .note
        .as_ref()
        .map(|n| Kit::line(n, 12, "$text3$", ""))
        .unwrap_or_default();
    let left = format!("{name}{}", Kit::pill(&card.pill));
    let right = format!("{note}{}", controls(&mut head, context, &card.switch));
    let head = child(out, format!("Head{index}"), toolbar(&left, &right), head);
    let stats = child(
        out,
        format!("Stats{index}"),
        Kit::spread_stats(&card.stats, context.compact()),
        Out::default(),
    );
    let chart = child(
        out,
        format!("Chart{index}"),
        Chart::markup(&card.chart, card.colour, 150),
        Out::default(),
    );
    let alerts = alerts_part(out, format!("Alerts{index}"), &card.alerts, "16,8,16,0");
    Kit::card(
        &format!(
            r#"<StackPanel>{head}<StackPanel Padding="16,2,16,0" Spacing="4">{stats}{chart}</StackPanel>{alerts}</StackPanel>"#
        ),
        "0,0,0,12",
    )
}

fn slim(out: &mut Out, context: &PageContext, card: &SlimCard, index: usize) -> String {
    let mut head = Out::default();
    let name = name_button(&mut head, card.colour, &card.name, 14);
    let action = card
        .action
        .as_ref()
        .map(|(label, command)| {
            Kit::button(
                &mut head,
                label,
                None,
                Look::Default,
                true,
                true,
                command.clone(),
                "",
            )
        })
        .unwrap_or_default();
    let left = format!("{name}{}", Kit::pill(&card.pill));
    let right = format!("{action}{}", controls(&mut head, context, &card.switch));
    let head = child(out, format!("Head{index}"), toolbar(&left, &right), head);
    let alerts = alerts_part(out, format!("Alerts{index}"), &card.alerts, "16,0,16,10");
    Kit::card(&format!("<StackPanel>{head}{alerts}</StackPanel>"), "0")
}

pub fn page(context: &PageContext, view: &StatusPageView, key: String) -> PageMarkup {
    let mut out = Out::default();
    let add = Kit::button(
        &mut out,
        "Add profile",
        Some("plus"),
        Look::Default,
        false,
        true,
        Command::Go(Page::Add),
        "",
    );
    let header = context.header(&mut out, "Status", Some(&view.subtitle), &add);
    let header = out.finish("Header", header);
    let stats = Out::default().finish("Stats", Kit::stats(&view.stats, context.compact()));
    let mut out = Out::default();
    let global = format!(
        r#"<StackPanel Spacing="4">{}</StackPanel>"#,
        alerts(&mut out, &view.alerts)
    );
    let global = out.finish("Alerts", global);
    let mut out = Out::default();
    let mut cards = String::new();
    for (i, card) in view.big.iter().enumerate() {
        cards.push_str(&big(&mut out, context, card, i));
    }
    for (i, card) in view.slim.iter().enumerate() {
        cards.push_str(&slim(&mut out, context, card, view.big.len() + i));
    }
    let cards = out.finish(
        "Cards",
        format!(r#"<StackPanel Spacing="14">{cards}</StackPanel>"#),
    );
    PageMarkup {
        key,
        regions: vec![header, stats, global, cards],
        grow: None,
    }
}

pub fn service_down(context: &PageContext, view: &ServiceDownView, key: String) -> PageMarkup {
    let mut out = Out::default();
    let pill = Kit::pill(&Pill::new(
        crate::presentation::Dot::Error,
        "Service not running",
        None,
    ));
    let header = context.header(&mut out, "Status", view.subtitle.as_deref(), &pill);
    let header = out.finish("Header", header);
    let mut out = Out::default();
    let copy = Kit::button(
        &mut out,
        "Copy command",
        Some("copy"),
        Look::Default,
        true,
        true,
        Command::Copy(view.command.clone()),
        "",
    );
    let bar = Kit::infobar(
        Tone::Error,
        &InfoBar::new(
            "SplitWire service is not running",
            &format!(
                "Start the SplitWire service in Services, or run {} in an administrator terminal.",
                view.command
            ),
        ),
        &copy,
    );
    let bar = out.finish("Message", bar);
    let dash = |label: &str| Stat::new(label, "—", "");
    let stats = Out::default().finish(
        "Stats",
        Kit::stats(
            &[
                dash("Connected"),
                dash("Queries"),
                dash("Learned routes"),
                dash("NRPT"),
            ],
            context.compact(),
        ),
    );
    let empty = Kit::card(
        &format!(
            r#"<StackPanel Padding="0,48,0,32">{}<StackPanel Orientation="Horizontal" Spacing="8" HorizontalAlignment="Center" Tag="align:row"><ProgressRing IsActive="True" Width="16" Height="16" Foreground="$accent$"/>{}</StackPanel></StackPanel>"#,
            Kit::empty(
                Some(("server", "$text2$")),
                "No answer from the service",
                "Profiles, routes, lists and alerts come from the service. Without the service there are no tunnels, routes or NRPT rules; when it starts again it restores the profiles that were connected."
            ),
            Kit::line("Checking again every second", 12, "$text3$", "")
        ),
        "0",
    );
    let empty = Out::default().finish("Empty", empty);
    PageMarkup {
        key,
        regions: vec![header, bar, stats, empty],
        grow: None,
    }
}
