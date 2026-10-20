//! Each page as regions: a header, then the page's blocks. A page is a ScrollViewer
//! of regions, or a grid whose one region takes the remaining height.

mod add;
mod dialogs;
mod lists;
mod logs;
mod profile;
mod route_groups;
mod routes;
mod settings;
mod status;

pub use dialogs::Dialogs;

use super::Kit;
use super::region::{Out, Region};
use crate::presentation::{Command, FrameView, PageView, WidthClass};

/// What every page's markup needs from the frame: the width class and the bell.
pub struct PageContext<'a> {
    pub frame: &'a FrameView,
    pub width: WidthClass,
}

impl PageContext<'_> {
    pub fn compact(&self) -> bool {
        self.width.compact()
    }

    /// The header region: title, subtitle, end controls, the bell.
    pub fn header(&self, out: &mut Out, title: &str, subtitle: Option<&str>, end: &str) -> String {
        let subtitle = subtitle.filter(|_| !self.compact()).unwrap_or_default();
        self.compose(out, title, subtitle, "", end)
    }

    /// A header with markup after the title instead of a subtitle.
    pub fn header_with(&self, out: &mut Out, title: &str, middle: &str, end: &str) -> String {
        self.compose(out, title, "", middle, end)
    }

    /// A subtitle counts what the service does (routes, profiles): the title and subtitle are a
    /// live part, so a new count reloads neither the bell nor the controls beside it.
    fn compose(
        &self,
        out: &mut Out,
        title: &str,
        subtitle: &str,
        middle: &str,
        end: &str,
    ) -> String {
        let bell = Kit::bell(out, self.frame.badge, &self.frame.bell, &|id| {
            Some(Command::FollowAlert(id.to_owned()))
        });
        let heading = out.live(r#"VerticalAlignment="Center""#, |_| {
            Kit::titled(title, 28, subtitle, "$text2$", "")
        });
        Kit::header(&heading, middle, end, &bell)
    }
}

/// A page: its regions in order; `grow` is the one that takes the remaining height.
pub struct PageMarkup {
    pub key: String,
    pub regions: Vec<Region>,
    pub grow: Option<usize>,
}

impl PageMarkup {
    /// The page region: a skeleton with one host per region, its regions as children.
    pub fn region(self, padding: &str) -> Region {
        let hosts: String = self
            .regions
            .iter()
            .enumerate()
            .map(|(i, r)| {
                let stretch = if Some(i) == self.grow {
                    r#" VerticalContentAlignment="Stretch""#
                } else {
                    ""
                };
                format!(
                    r#"<ContentControl x:Name="{}" Grid.Row="{i}" HorizontalContentAlignment="Stretch"{stretch}/>"#,
                    r.host
                )
            })
            .collect();
        let markup = match self.grow {
            None => format!(
                r#"<ScrollViewer x:Name="PageScroll" Tag="{}" VerticalScrollBarVisibility="Auto" HorizontalScrollBarVisibility="Disabled"><StackPanel Spacing="14" Padding="{padding}">{hosts}</StackPanel></ScrollViewer>"#,
                Kit::esc(&self.key)
            ),
            Some(grow) => {
                let rows: String = (0..self.regions.len())
                    .map(|i| {
                        if i == grow {
                            r#"<RowDefinition/>"#.to_owned()
                        } else {
                            r#"<RowDefinition Height="Auto"/>"#.to_owned()
                        }
                    })
                    .collect();
                format!(
                    r#"<Grid x:Name="PageScroll" Tag="{}" Padding="{padding}" RowSpacing="14"><Grid.RowDefinitions>{rows}</Grid.RowDefinitions>{hosts}</Grid>"#,
                    Kit::esc(&self.key)
                )
            }
        };
        let mut out = Out::default();
        for region in self.regions {
            out.child(region);
        }
        out.finish("PageHost", markup)
    }
}

/// The regions of the page shown.
pub fn page(context: &PageContext) -> PageMarkup {
    let key = context.frame.page.key();
    match &context.frame.content {
        PageView::ServiceDown(view) => status::service_down(context, view, key),
        PageView::Status(view) => status::page(context, view, key),
        PageView::Add(view) => add::page(context, view, key),
        PageView::Profile(view) => profile::page(context, view, key),
        PageView::Routes(view) => routes::page(context, view, key),
        PageView::Lists(view) => lists::page(context, view, key),
        PageView::Logs(view) => logs::page(context, view, key),
        PageView::Settings(view) => settings::page(context, view, key),
        PageView::Review(view) => add::review(context, view, key),
        PageView::Loading(title) => {
            let mut out = Out::default();
            let header = context.header(&mut out, title, None, "");
            PageMarkup {
                key,
                regions: vec![out.finish("Header", header)],
                grow: None,
            }
        }
    }
}
