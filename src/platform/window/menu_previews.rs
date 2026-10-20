//! The profile menus of every demo scenario as pictures and alignment checks: a flyout opens only
//! in a shown window, so `--render-pages` and `--verify-pages` lay out [`Menu::preview`], the same
//! items in a card drawn as the menu's presenter.

use super::markup::{Menu, Out, Region, Xml};
use crate::presentation::showcase::Driver;
use crate::presentation::transport::demo::Scenario;

pub struct MenuPreviews;

impl MenuPreviews {
    /// The size a menu is laid out at: the menu and the margin around it.
    pub const SIZE: (f64, f64) = (320.0, 260.0);
    /// The one host of the skeleton.
    pub const HOST: &'static str = "Host";

    pub fn skeleton() -> String {
        Xml::root(&format!(
            r#"<ContentControl x:Name="{}" HorizontalContentAlignment="Stretch" VerticalContentAlignment="Stretch"/>"#,
            Self::HOST
        ))
    }

    /// `(label, region)` for every profile menu of every scenario: `normal:menu-AcmeCorp`.
    pub fn all() -> Vec<(String, Region)> {
        Scenario::ALL
            .into_iter()
            .flat_map(|scenario| {
                Driver::new(scenario)
                    .session
                    .view()
                    .menus
                    .into_iter()
                    .map(move |menu| {
                        let mut out = Out::default();
                        let markup = Menu::preview(&mut out, &menu);
                        (
                            format!("{}:menu-{}", scenario.key(), menu.name),
                            out.finish(Self::HOST, markup),
                        )
                    })
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_scenario_with_profiles_has_its_menus() {
        let previews = MenuPreviews::all();
        assert!(
            previews
                .iter()
                .any(|(label, _)| label == "normal:menu-AcmeCorp")
        );
        for (label, region) in &previews {
            assert_eq!(region.names().len(), 4, "{label}");
        }
    }
}
