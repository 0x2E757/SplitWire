//! One view model per page, each built from what the engine answered.

mod add;
mod lists;
mod logs;
mod profile;
mod review;
mod route_groups;
mod routes;
mod settings;
mod status;

pub use add::{AddView, DropState, Fact};
pub use lists::{ListGroupView, ListRowView, ListsPageView, ListsSource, ListsTextView};
pub use logs::{LogRowView, LogsPageView, LogsSource, LogsUi};
pub use profile::{BlockedSettings, ProfilePageView, ProfileSource, StripView};
pub use review::{ReviewFileView, ReviewView};
pub use route_groups::{GroupIcon, RouteCellView, RouteGroupView, RouteGroups};
pub use routes::{RouteCardView, RoutesPageView, RoutesSource};
pub use settings::{
    Badge, ExpanderView, FieldView, ProfileExpander, SettingsPageView, SettingsSource,
};
pub use status::{BigCard, ServiceDownView, SlimCard, StatusPageView, StatusSource};

use super::StateText;
use crate::ipc::ProfileSummary;

/// A statistic: label, value and a quiet caption.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Stat {
    pub label: String,
    pub value: String,
    pub caption: String,
}

impl Stat {
    pub fn new(label: &str, value: &str, caption: &str) -> Self {
        Self {
            label: label.into(),
            value: value.into(),
            caption: caption.into(),
        }
    }
}

/// A profile's Connect switch: on while connected or connecting, off and disabled while it
/// cannot connect; its tooltip names what a click does.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SwitchView {
    pub name: String,
    pub on: bool,
    pub enabled: bool,
    pub tip: String,
}

impl SwitchView {
    pub fn of(profile: &ProfileSummary, global_invalid: bool) -> Self {
        let on = StateText::switched_on(&profile.state);
        Self {
            name: profile.name.clone(),
            on,
            enabled: on || StateText::can_connect(&profile.state, global_invalid),
            tip: format!(
                "{} {}",
                if on { "Disconnect" } else { "Connect" },
                profile.name
            ),
        }
    }
}

/// A page-level message (an InfoBar-like Border).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InfoBar {
    pub title: String,
    pub message: String,
}

impl InfoBar {
    pub fn new(title: &str, message: &str) -> Self {
        Self {
            title: title.into(),
            message: message.into(),
        }
    }
}
