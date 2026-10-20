//! The focused field across a reload of its region. Typing reloads no region around a field (live
//! parts change instead), but a region may still reload under a focused field when the service's
//! state changes it. The field's focus, caret and selection are then taken before the reload and
//! given to the new TextBox once it can take them: a control just put into its host joins the live
//! tree only at the next layout, so the frame loop tries again on the next frames.

use super::xaml::Xaml;
use crate::platform::com::Com;

/// The field that has the focus: its element name and which TextBox it is, so the LostFocus of a
/// TextBox a reload replaced does not clear the record its successor wrote.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Focused {
    pub name: String,
    pub element: usize,
}

impl Focused {
    pub fn new(name: &str, element: &Com) -> Self {
        Self {
            name: name.to_owned(),
            element: element.raw() as usize,
        }
    }
}

/// A focus waiting to be given back to a reloaded field.
pub struct Restore {
    pub name: String,
    selection: (i32, i32),
    tries: u8,
}

impl Restore {
    /// About two dozen frames: layout comes long before.
    const TRIES: u8 = 24;

    /// The focus of `element`, about to be replaced.
    pub fn take(name: &str, element: &Com) -> Self {
        Self {
            name: name.to_owned(),
            selection: Xaml::selection(element).unwrap_or((0, 0)),
            tries: Self::TRIES,
        }
    }

    /// Puts the caret and selection back into the new field and focuses it; false while the focus
    /// should be tried again on a later frame. The selection goes in first, so it is there however
    /// the focus comes back, and again once the focus did.
    pub fn give(&mut self, element: Option<&Com>) -> bool {
        let Some(element) = element else {
            return true;
        };
        if self.tries == Self::TRIES {
            let _ = Xaml::select(element, self.selection);
        }
        if Xaml::focus(element).unwrap_or(false) {
            let _ = Xaml::select(element, self.selection);
            return true;
        }
        self.tries = self.tries.saturating_sub(1);
        self.tries == 0
    }
}
