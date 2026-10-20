//! The pipe's protocol: frames (`Frame`), their binary codec (`Writer`, `Reader`) and every
//! request and response with the data they carry. Messages reuse the domain types where
//! they exist (previews, settings and list problems, counters) and carry small types of their own
//! for what the engine reports. No response can hold key material; the one request that carries a
//! `.conf` holds it in a zeroed, redacted `ConfText`.

mod alert;
mod attempt;
mod conf_text;
mod domain_wire;
mod error_reply;
mod frame;
mod lists;
mod logs;
mod profile_detail;
mod profile_summary;
mod profile_wire;
mod reader;
mod request;
mod response;
mod routes;
mod settings;
mod settings_wire;
mod status;
mod traffic;
mod wire_error;
mod writer;

#[cfg(test)]
mod samples;
#[cfg(test)]
mod tests;

pub use alert::{ActionTarget, Alert, AlertAction, AlertKind, SettingField, Severity, Suggestion};
pub use attempt::{
    Attempt, AttemptKind, AttemptResult, AttemptSummary, ConnectStep, StepOutcome, StepResult,
};
pub use conf_text::ConfText;
pub use error_reply::{ErrorCode, ErrorReply, RefusedProblem};
pub use frame::Frame;
pub use lists::{LineRef, ListFile, ListLine, ListLineKind, ListsView};
pub use logs::{LogLine, LogsPage};
pub use profile_detail::ProfileDetail;
pub use profile_summary::{ProfileState, ProfileSummary};
pub use reader::Reader;
pub use request::{ProfileTarget, Request};
pub use response::Response;
pub use routes::{ProfileRoutes, RouteGroup, RouteGroupKey, RouteRow};
pub use settings::{Reconnect, ReconnectOutcome, ScopeSettings, SettingsSaved, SettingsView};
pub use status::{Counters, NrptStatus, Revisions, StatusReport, TrayState};
pub use traffic::{ProfileTraffic, TrafficSample};
pub use wire_error::WireError;
pub use writer::Writer;
