//! What the window remembers between frames: the page, open expanders, typed text, the pending
//! settings, the import and the logs filters.

use std::collections::{HashMap, HashSet};
use std::time::Instant;

use crate::list::TextProblem;
use crate::presentation::pages::LogsUi;
use crate::presentation::{ImportBatch, InputId, ListsMode, Page, SettingsEdit};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Dialog {
    /// Remove <name>? (a confirm dialog).
    Remove(String),
    /// Edit <name>'s list as text; the text is the `ListDialog` input.
    ListText {
        profile: String,
        error: Option<String>,
    },
}

pub struct UiState {
    pub page: Page,
    pub nav_overlay: bool,
    pub visible: bool,
    /// Ids whose expander or group differs from its default (open ↔ closed).
    pub flipped: HashSet<String>,
    /// Typed text by element name.
    pub inputs: HashMap<String, String>,
    pub dialog: Option<Dialog>,
    pub lists_mode: ListsMode,
    /// The combined text being edited, while it differs from the service's.
    pub lists_draft: Option<String>,
    pub lists_refused: Vec<TextProblem>,
    pub logs: LogsUi,
    pub settings: SettingsEdit,
    pub import: Option<ImportBatch>,
    /// The profile a Replace .conf… picks a file for.
    pub replace: Option<String>,
    /// Files are dragged over the drop zone; their names when known.
    pub dragging: Option<Vec<String>>,
    pub toast: Option<(String, Instant)>,
}

impl Default for UiState {
    fn default() -> Self {
        Self {
            page: Page::Status,
            nav_overlay: false,
            visible: true,
            flipped: HashSet::new(),
            inputs: HashMap::new(),
            dialog: None,
            lists_mode: ListsMode::Rows,
            lists_draft: None,
            lists_refused: Vec::new(),
            logs: LogsUi::default(),
            settings: SettingsEdit::default(),
            import: None,
            replace: None,
            dragging: None,
            toast: None,
        }
    }
}

impl UiState {
    pub fn input(&self, id: &InputId) -> String {
        self.inputs.get(&id.element()).cloned().unwrap_or_default()
    }

    pub fn set_input(&mut self, id: &InputId, text: String) {
        self.inputs.insert(id.element(), text);
    }

    pub fn open(&self, id: &str, default: bool) -> bool {
        default != self.flipped.contains(id)
    }

    pub fn flip(&mut self, id: &str) {
        if !self.flipped.remove(id) {
            self.flipped.insert(id.to_owned());
        }
    }

    /// Opens `id` if it is closed by default.
    pub fn force_open(&mut self, id: &str, default: bool) {
        if !self.open(id, default) {
            self.flip(id);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expanders_flip_against_their_default() {
        let mut ui = UiState::default();
        assert!(ui.open("a", true));
        ui.flip("a");
        assert!(!ui.open("a", true));
        assert!(ui.open("a", false));
        ui.force_open("a", true);
        assert!(ui.open("a", true));
        ui.set_input(&InputId::LogFilter, "x".into());
        assert_eq!(ui.input(&InputId::LogFilter), "x");
    }
}
