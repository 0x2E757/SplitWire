//! A region: markup loaded into one named host, the events its elements raise and the text fields
//! the window reads. Pages are lists of regions; a region is reloaded only when its markup changed.

use crate::presentation::{Command, InputId};

/// What an element raises.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Event {
    /// A Button or HyperlinkButton click.
    Click,
    /// A ToggleSwitch was switched.
    Toggled,
    /// A MenuFlyoutItem was chosen.
    MenuClick,
    /// Enter in a TextBox.
    Enter,
    /// A navigation item: click, plus dragging to reorder its profile.
    NavDrag,
    /// The drop zone: files dragged over and dropped.
    Drop,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Bind {
    pub element: String,
    pub event: Event,
    pub command: Command,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Region {
    pub host: String,
    /// Markup with `§name§` where a text field's current text goes.
    pub markup: String,
    pub binds: Vec<Bind>,
    pub inputs: Vec<(InputId, String)>,
    /// Regions whose hosts are inside this one; reloaded with it.
    pub children: Vec<Region>,
}

impl Region {
    /// The markup ready to load: the namespaces and the fields' texts in place, in a TextBox's form.
    pub fn loadable(&self) -> String {
        let mut markup = self.markup.clone();
        for (id, text) in &self.inputs {
            markup = markup.replace(
                &format!("§{}§", id.element()),
                &super::Xml::escape(&super::LineBreaks::shown(text)),
            );
        }
        markup
    }

    /// Every element the window looks up in this region.
    pub fn names(&self) -> Vec<String> {
        self.binds
            .iter()
            .map(|b| b.element.clone())
            .chain(self.inputs.iter().map(|(id, _)| id.element()))
            .chain(self.children.iter().map(|c| c.host.clone()))
            .collect()
    }
}

/// Collects the binds, fields and child regions while a region's markup is written.
#[derive(Default)]
pub struct Out {
    binds: Vec<Bind>,
    inputs: Vec<(InputId, String)>,
    children: Vec<Region>,
    next: usize,
}

impl Out {
    /// A unique element name bound to `command` for `event`.
    pub fn bind(&mut self, event: Event, command: Command) -> String {
        self.next += 1;
        let element = format!("E{}", self.next);
        self.binds.push(Bind {
            element: element.clone(),
            event,
            command,
        });
        element
    }

    /// A unique element name with nothing bound to it.
    pub fn name(&mut self) -> String {
        self.next += 1;
        format!("E{}", self.next)
    }

    /// Binds one more event to an element already named.
    pub fn also(&mut self, element: &str, event: Event, command: Command) {
        self.binds.push(Bind {
            element: element.into(),
            event,
            command,
        });
    }

    /// A text field: its element name and the `§name§` its text goes into.
    pub fn input(&mut self, id: InputId, text: &str) -> (String, String) {
        let element = id.element();
        let token = format!("§{element}§");
        self.inputs.push((id, text.to_owned()));
        (element, token)
    }

    pub fn child(&mut self, region: Region) {
        self.children.push(region);
    }

    /// A part that changes while a field is typed into (a verdict's colour, a count, an Add
    /// button's state): a child region in a ContentControl of its own, so a change reloads the
    /// part and not the region around the field, which would take the focus away. `attributes`
    /// place the host; `build` writes the part with its own binds.
    pub fn live(&mut self, attributes: &str, build: impl FnOnce(&mut Out) -> String) -> String {
        let host = self.name();
        let mut part = Out::default();
        let markup = build(&mut part);
        self.children.push(part.finish(&host, markup));
        format!(
            r#"<ContentControl x:Name="{host}" HorizontalContentAlignment="Stretch" VerticalContentAlignment="Stretch" {attributes}/>"#
        )
    }

    /// The region; empty markup becomes an empty Grid, so every host loads something. Binds and
    /// fields of parts that were left out (a closed expander's body) are dropped.
    pub fn finish(mut self, host: &str, markup: String) -> Region {
        let markup = if markup.trim().is_empty() {
            "<Grid/>".to_owned()
        } else {
            markup
        };
        let named = |name: &str| markup.contains(&format!("x:Name=\"{name}\""));
        self.binds.retain(|b| named(&b.element));
        self.inputs.retain(|(id, _)| named(&id.element()));
        self.children.retain(|c| named(&c.host));
        Region {
            host: host.into(),
            markup: super::Xml::root(&markup),
            binds: self.binds,
            inputs: self.inputs,
            children: self.children,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::presentation::Page;

    #[test]
    fn regions_name_their_elements_and_fill_their_fields() {
        let mut out = Out::default();
        let button = out.bind(Event::Click, Command::Go(Page::Logs));
        let (field, token) = out.input(InputId::LogFilter, "a<b");
        let region = out.finish(
            "Host",
            format!(r#"<StackPanel><Button x:Name="{button}"/><TextBox x:Name="{field}" Text="{token}"/></StackPanel>"#),
        );
        assert_eq!(region.names(), ["E1", "LogFilter"]);
        assert!(region.markup.starts_with("<StackPanel xmlns="));
        assert!(region.loadable().contains(r#"Text="a&lt;b""#));
        assert!(region.markup.contains("§LogFilter§"));
    }

    #[test]
    fn a_live_part_is_a_child_region_with_its_own_binds() {
        let mut out = Out::default();
        let (field, token) = out.input(InputId::ProfileAdd, "a");
        let live = out.live(r#"Grid.Column="1""#, |part| {
            let add = part.bind(Event::Click, Command::AddEntry("Lab".into()));
            format!(r#"<Button x:Name="{add}" IsEnabled="False"/>"#)
        });
        let region = out.finish(
            "Host",
            format!(r#"<Grid><TextBox x:Name="{field}" Text="{token}"/>{live}</Grid>"#),
        );
        assert!(region.binds.is_empty());
        assert!(region.markup.contains(r#"<ContentControl x:Name="E1""#));
        assert!(!region.markup.contains("IsEnabled"));
        let part = &region.children[0];
        assert_eq!(part.host, "E1");
        assert_eq!(part.binds[0].command, Command::AddEntry("Lab".into()));
    }
}
