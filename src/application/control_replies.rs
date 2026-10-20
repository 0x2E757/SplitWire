use super::wire_attempts::WireAttempts;
use super::{CoordinatorError, ListRefusal, ProfileError, SaveRefused};
use crate::alerts::DismissRefused;
use crate::ipc::{ErrorCode, ErrorReply, RefusedProblem, Response, WireError};

/// The `Error{code, message}` answers, one per way a request can fail, and the
/// failed Connect with its step table.
pub struct ControlReplies;

impl ControlReplies {
    const NOTHING_WRITTEN: &'static str = "nothing was written";

    pub fn bad_request(message: impl Into<String>) -> Response {
        Self::error(ErrorCode::BadRequest, message.into())
    }

    pub fn undecodable(error: &WireError) -> Response {
        Self::bad_request(error.to_string())
    }

    pub fn unknown_profile(name: &str) -> Response {
        Self::error(
            ErrorCode::UnknownProfile,
            format!("no profile named {name}"),
        )
    }

    pub fn refused(problems: Vec<RefusedProblem>, message: impl Into<String>) -> Response {
        Self::error(ErrorCode::Refused(problems), message.into())
    }

    pub fn coordinator(error: CoordinatorError, name: &str) -> Response {
        match error {
            CoordinatorError::UnknownProfile(_) => Self::unknown_profile(name),
            CoordinatorError::ConnectFailed(attempt) => Response::ConnectFailed {
                name: name.to_owned(),
                attempt: WireAttempts::attempt(&attempt),
            },
        }
    }

    pub fn list(error: ListRefusal) -> Response {
        match error {
            ListRefusal::Stale => Self::error(ErrorCode::StaleRev, error.to_string()),
            ListRefusal::UnknownProfile(name) => Self::unknown_profile(&name),
            ListRefusal::Problems(problems) => Self::refused(
                problems.into_iter().map(RefusedProblem::List).collect(),
                Self::NOTHING_WRITTEN,
            ),
            ListRefusal::Io(error) => Self::refused(
                vec![RefusedProblem::Text(error.to_string())],
                Self::NOTHING_WRITTEN,
            ),
        }
    }

    pub fn profile(error: ProfileError) -> Response {
        match error {
            ProfileError::UnknownProfile(name) => Self::unknown_profile(&name),
            ProfileError::Refused(problems) => Self::refused(
                problems.into_iter().map(RefusedProblem::Text).collect(),
                Self::NOTHING_WRITTEN,
            ),
            ProfileError::Io(error) => Self::refused(
                vec![RefusedProblem::Text(error.to_string())],
                Self::NOTHING_WRITTEN,
            ),
        }
    }

    pub fn settings(error: SaveRefused) -> Response {
        match error {
            SaveRefused::Stale => Self::error(
                ErrorCode::StaleRev,
                "the settings changed on disk; reloaded".into(),
            ),
            SaveRefused::Problems(problems) => Self::refused(
                problems.into_iter().map(RefusedProblem::Setting).collect(),
                Self::NOTHING_WRITTEN,
            ),
            SaveRefused::Io(error) => Self::refused(
                vec![RefusedProblem::Text(error.to_string())],
                Self::NOTHING_WRITTEN,
            ),
        }
    }

    pub fn dismiss(error: DismissRefused) -> Response {
        match error {
            DismissRefused::Unknown => Self::bad_request(error.to_string()),
            DismissRefused::Blocking => {
                Self::refused(vec![RefusedProblem::Text(error.to_string())], "")
            }
        }
    }

    fn error(code: ErrorCode, message: String) -> Response {
        Response::Error(ErrorReply { code, message })
    }
}
