//! Alerts: raised and cleared by conditions, dismissed and restored by the user, with today's
//! history; their wording; and the tray state computed from them.

#[allow(clippy::module_inception)]
mod alert;
mod alert_kind;
mod alert_store;
mod alert_texts;
mod tray_state;

pub use alert::{Alert, AlertAction, AlertRaise, Suggestion};
pub use alert_kind::{AlertKind, Severity};
pub use alert_store::{AlertStore, DismissRefused};
pub use alert_texts::AlertTexts;
pub use tray_state::TrayState;
