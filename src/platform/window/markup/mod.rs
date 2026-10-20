//! The window's markup: XAML text built from the view models, with `$token$` colours that the
//! palette fills per theme. Pure, so every page is checked without a window.

mod align;
mod chart;
#[cfg(test)]
mod check;
mod frame;
mod icons;
mod kit;
mod line_breaks;
mod menu;
mod nav;
mod pages;
mod palette;
mod region;
mod styles;
#[cfg(test)]
mod time_check;
#[cfg(test)]
mod typing_check;
mod xml;

pub use align::Align;
pub use frame::Frame;
pub use icons::Icons;
pub use kit::{Kit, Look, Tone};
pub use line_breaks::LineBreaks;
pub use menu::Menu;
pub use palette::Palette;
pub use region::{Bind, Event, Out, Region};
pub use styles::Styles;
pub use xml::Xml;
