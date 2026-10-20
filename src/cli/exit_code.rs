use crate::ipc::{ErrorCode, ErrorReply};

/// What `SplitWire.exe` exits with.
pub struct ExitCode;

impl ExitCode {
    pub const OK: i32 = 0;
    pub const ERROR: i32 = 1;
    pub const USAGE: i32 = 2;
    /// No engine serves the pipe.
    pub const NOT_REACHABLE: i32 = 3;
    pub const ADMIN_REQUIRED: i32 = 4;
    /// A problem refused the request; it is printed with its lines.
    pub const REFUSED: i32 = 5;
    pub const UNKNOWN_PROFILE: i32 = 6;

    /// The code of an `Error` answer.
    pub fn of(reply: &ErrorReply) -> i32 {
        match reply.code {
            ErrorCode::BadRequest => Self::ERROR,
            ErrorCode::UnknownProfile => Self::UNKNOWN_PROFILE,
            ErrorCode::Refused(_) | ErrorCode::StaleRev => Self::REFUSED,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_every_error_code() {
        let reply = |code| ErrorReply {
            code,
            message: String::new(),
        };
        assert_eq!(ExitCode::of(&reply(ErrorCode::BadRequest)), 1);
        assert_eq!(ExitCode::of(&reply(ErrorCode::UnknownProfile)), 6);
        assert_eq!(ExitCode::of(&reply(ErrorCode::Refused(Vec::new()))), 5);
        assert_eq!(ExitCode::of(&reply(ErrorCode::StaleRev)), 5);
    }
}
