//! Routes (view-only): totals, a search, one summary card per profile opening in place.

use super::route_groups::RouteTable;
use super::status::more;
use super::{PageContext, PageMarkup};
use crate::platform::window::markup::region::{Event, Out};
use crate::platform::window::markup::{Align, Kit, Look, Tone};
use crate::presentation::pages::RoutesPageView;
use crate::presentation::{Command, InputId, Page};

/// The width of a card's small "…" button.
const MENU: u32 = 26;

pub fn page(context: &PageContext, view: &RoutesPageView, key: String) -> PageMarkup {
    let mut out = Out::default();
    let header = context.header(&mut out, "Routes", Some(&view.subtitle), "");
    let header = out.finish("Header", header);

    let mut out = Out::default();
    let message = view
        .message
        .as_ref()
        .map(|bar| {
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
            Kit::infobar(Tone::Error, bar, &open)
        })
        .unwrap_or_default();
    let message = out.finish("Message", message);

    let mut out = Out::default();
    let search = Kit::field(
        &mut out,
        InputId::RoutesSearch,
        &context.frame.input(&InputId::RoutesSearch),
        "Search by address or name",
        Some("search"),
        Some(280),
        None,
        false,
    );
    let tools = out.finish(
        "Tools",
        format!(r#"<Grid><Grid HorizontalAlignment="Right">{search}</Grid></Grid>"#),
    );

    let mut out = Out::default();
    let cards: String = if let Some(text) = &view.no_match {
        Kit::card(&Kit::empty(None, "No matches", text), "0")
    } else {
        view.cards
            .iter()
            .map(|card| {
                let toggle = out.bind(Event::Click, Command::Toggle(format!("routes:{}", card.name)));
                let numbers: String = card
                    .numbers
                    .iter()
                    .filter(|(label, _)| !(context.compact() && label == "Last learned"))
                    .map(|(label, value)| {
                        format!(
                            r#"<StackPanel MinWidth="64" Tag="align:block">{}{}</StackPanel>"#,
                            Kit::text(label, 12, "$text2$", ""),
                            Kit::text(value, 15, "$text$", r#"FontWeight="SemiBold""#)
                        )
                    })
                    .collect();
                let menu = more(&mut out, context, &card.name, true);
                // The whole header opens and closes the card: the toggle lies under it all, and
                // the menu's button, drawn over the toggle's empty last column, clicks on its own.
                let head = format!(
                    r#"<Grid{}><Button x:Name="{toggle}" HorizontalAlignment="Stretch" VerticalAlignment="Stretch" HorizontalContentAlignment="Stretch" VerticalContentAlignment="Center" Background="Transparent" BorderThickness="0" Padding="12,10" MinHeight="0" CornerRadius="{}"><Grid ColumnSpacing="12" Tag="align:row"><Grid.ColumnDefinitions><ColumnDefinition/><ColumnDefinition Width="Auto"/><ColumnDefinition Width="{MENU}"/></Grid.ColumnDefinitions><Grid ColumnSpacing="10"><Grid.ColumnDefinitions><ColumnDefinition Width="Auto"/><ColumnDefinition/></Grid.ColumnDefinitions>{}<StackPanel Grid.Column="1" Spacing="4" Tag="align:block"><StackPanel Orientation="Horizontal" Spacing="8" Tag="align:row">{}{}</StackPanel>{}</StackPanel></Grid><StackPanel Grid.Column="1" Orientation="Horizontal" Spacing="20" VerticalAlignment="Center">{numbers}</StackPanel></Grid></Button><Grid Width="{MENU}" HorizontalAlignment="Right" VerticalAlignment="Center" Margin="0,0,12,0">{menu}</Grid></Grid>"#,
                    Align::COVER,
                    if card.open { "7,7,0,0" } else { "7" },
                    Kit::icon(if card.open { "chev" } else { "chevr" }, 12, "$text2$"),
                    Kit::pname(card.colour, &card.name),
                    Kit::pill(&card.pill),
                    // Without a caption the name line alone is centred, not left at the top.
                    if card.caption.is_empty() {
                        String::new()
                    } else {
                        Kit::line(&card.caption, 12, "$text3$", "")
                    }
                );
                let body = if card.open {
                    let groups = RouteTable {
                        profile: &card.name,
                        groups: &card.groups,
                        removable: false,
                    }
                    .markup(&mut out, context);
                    format!(
                        r#"<Border BorderBrush="$divider$" BorderThickness="0,1,0,0"><StackPanel>{}<StackPanel Padding="8,4">{groups}</StackPanel></StackPanel></Border>"#,
                        RouteTable::header(context.compact())
                    )
                } else {
                    String::new()
                };
                Kit::card(&format!("<StackPanel>{head}{body}</StackPanel>"), "0")
            })
            .collect()
    };
    let cards = out.finish(
        "Cards",
        format!(r#"<StackPanel Spacing="8">{cards}</StackPanel>"#),
    );
    PageMarkup {
        key,
        regions: vec![header, message, tools, cards],
        grow: None,
    }
}
