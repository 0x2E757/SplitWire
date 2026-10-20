//! Checks of every page's markup without XAML: balanced elements, quoted attributes, every
//! placeholder filled, every bound element named. `--verify-pages` then loads the same markup
//! through XamlReader.

use super::{Frame, Palette, Region};
use crate::presentation::WidthClass;
use crate::presentation::showcase::Showcase;
use crate::presentation::transport::demo::Scenario;

/// The problems of one document: unbalanced or unquoted markup.
fn well_formed(markup: &str) -> Result<(), String> {
    let mut stack: Vec<&str> = Vec::new();
    let mut rest = markup;
    while let Some(start) = rest.find('<') {
        rest = &rest[start + 1..];
        if let Some(comment) = rest.strip_prefix("!--") {
            let end = comment.find("-->").ok_or("an unclosed comment")?;
            rest = &comment[end + 3..];
            continue;
        }
        let end = Self_::tag_end(rest).ok_or("an unclosed tag")?;
        let tag = &rest[..end];
        rest = &rest[end + 1..];
        if let Some(name) = tag.strip_prefix('/') {
            let open = stack
                .pop()
                .ok_or_else(|| format!("</{name}> closes nothing"))?;
            if open != name.trim() {
                return Err(format!("<{open}> closed by </{name}>"));
            }
            continue;
        }
        let name = tag.split([' ', '/']).next().unwrap_or_default();
        if !tag.matches('"').count().is_multiple_of(2) {
            return Err(format!("<{name}> has an unbalanced quote"));
        }
        if !tag.ends_with('/') {
            stack.push(name);
        }
    }
    match stack.pop() {
        Some(open) => Err(format!("<{open}> is never closed")),
        None => Ok(()),
    }
}

/// A multi-line TextBox sets `AcceptsReturn` before `Text`: XAML sets attributes in order, and a
/// TextBox without it keeps only the first line of its text.
fn multi_line_fields(markup: &str) -> Result<(), String> {
    for (start, _) in markup.match_indices("<TextBox ") {
        let tag = &markup[start..];
        let tag = &tag[..Self_::tag_end(tag).unwrap_or(tag.len())];
        if let (Some(accepts), Some(text)) = (tag.find(" AcceptsReturn="), tag.find(" Text=\""))
            && text < accepts
        {
            return Err("a TextBox sets Text before AcceptsReturn".into());
        }
    }
    Ok(())
}

struct Self_;

impl Self_ {
    /// The `>` ending a tag, outside quotes.
    fn tag_end(text: &str) -> Option<usize> {
        let mut quoted = false;
        for (i, c) in text.char_indices() {
            match c {
                '"' => quoted = !quoted,
                '>' if !quoted => return Some(i),
                _ => {}
            }
        }
        None
    }
}

fn check(region: &Region, palette: Palette, label: &str) -> Result<usize, String> {
    let markup = palette.apply(&region.loadable());
    well_formed(&markup).map_err(|e| format!("{label} {}: {e}", region.host))?;
    multi_line_fields(&markup).map_err(|e| format!("{label} {}: {e}", region.host))?;
    if let Some(at) = markup.find('§') {
        return Err(format!(
            "{label} {}: unfilled field near {}",
            region.host,
            &markup[at..(at + 40).min(markup.len())]
        ));
    }
    for (index, _) in markup.match_indices('$') {
        let token: String = markup[index + 1..]
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric())
            .collect();
        if !token.is_empty() && markup[index + 1 + token.len()..].starts_with('$') {
            return Err(format!("{label} {}: unknown token ${token}$", region.host));
        }
    }
    for name in region.names() {
        if !markup.contains(&format!(r#"x:Name="{name}""#)) {
            return Err(format!(
                "{label} {}: {name} is bound but not named",
                region.host
            ));
        }
    }
    let mut count = 1;
    for child in &region.children {
        count += check(child, palette, label)?;
    }
    Ok(count)
}

#[test]
fn every_page_of_every_scenario_is_well_formed() {
    let mut regions = 0;
    for palette in Palette::BOTH {
        well_formed(&palette.apply(&Frame::skeleton(WidthClass::Full))).unwrap();
        for scenario in Scenario::ALL {
            for (label, view) in Showcase::views(scenario) {
                for width in WidthClass::ALL {
                    for region in Frame::regions(&view, width) {
                        regions += check(&region, palette, &label).unwrap();
                    }
                }
            }
        }
    }
    assert!(regions > 1000, "{regions}");
}

/// The page checks reach traffic charts with their series, so XamlReader loads both halves' colours.
#[test]
fn the_showcase_has_charts_with_both_halves() {
    fn drawn(region: &Region) -> bool {
        region.markup.contains(r#"Stroke="$total$""#) || region.children.iter().any(drawn)
    }
    let charts = Showcase::views(Scenario::ALL[0])
        .iter()
        .flat_map(|(_, view)| Frame::regions(view, WidthClass::Full))
        .filter(drawn)
        .count();
    assert!(charts > 0);
}

#[test]
fn the_checker_finds_broken_markup() {
    assert!(well_formed("<A><B/></A>").is_ok());
    assert!(well_formed(r#"<A x="1>2"/>"#).is_ok());
    assert!(well_formed("<A><B></A>").is_err());
    assert!(well_formed(r#"<A x="1/>"#).is_err());
    assert!(well_formed("<A>").is_err());
    assert!(multi_line_fields(r#"<TextBox AcceptsReturn="True" Text="a"/>"#).is_ok());
    assert!(multi_line_fields(r#"<TextBox Text="a" PlaceholderText="b"/>"#).is_ok());
    assert!(multi_line_fields(r#"<TextBox Text="a" AcceptsReturn="True"/>"#).is_err());
}

/// A Routes card without a caption has no empty second line, so its name line is centred.
#[test]
fn a_routes_card_without_a_caption_has_no_empty_line() {
    fn markup(region: &Region) -> String {
        region
            .children
            .iter()
            .fold(region.markup.clone(), |all, child| all + &markup(child))
    }
    let (_, mut view) = Showcase::views(Scenario::Normal)
        .into_iter()
        .find(|(label, _)| label == "normal:routes")
        .unwrap();
    let crate::presentation::PageView::Routes(routes) = &mut view.content else {
        panic!("routes")
    };
    let caption = std::mem::take(&mut routes.cards[0].caption);
    let page: String = Frame::regions(&view, WidthClass::Full)
        .iter()
        .map(markup)
        .collect();
    assert!(!caption.is_empty() && !page.contains(&caption));
    assert!(!page.contains(r#"<TextBlock Text="" "#));
}
