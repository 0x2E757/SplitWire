use super::domain_wire::DomainWire;
use super::settings_wire::SettingsWire;
use super::{Reader, WireError, Writer};
use crate::config::SettingProblem;
use crate::list::TextProblem;

/// Why a request was refused, with its line numbers where it has them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RefusedProblem {
    /// A list edit or save: both numbers.
    List(TextProblem),
    /// A settings save.
    Setting(SettingProblem),
    /// Anything else, such as an import's blocking problem.
    Text(String),
}

impl RefusedProblem {
    fn write(&self, w: &mut Writer) {
        match self {
            Self::List(problem) => {
                w.u64(0);
                DomainWire::text_problem(problem, w);
            }
            Self::Setting(problem) => {
                w.u64(1);
                SettingsWire::problem(problem, w);
            }
            Self::Text(text) => {
                w.u64(2);
                w.str(text);
            }
        }
    }

    fn read(r: &mut Reader) -> Result<Self, WireError> {
        Ok(match r.u64()? {
            0 => Self::List(DomainWire::read_text_problem(r)?),
            1 => Self::Setting(SettingsWire::read_problem(r)?),
            2 => Self::Text(r.string()?),
            _ => return Err(WireError::Invalid("an unknown problem kind")),
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ErrorCode {
    /// The request could not be decoded or makes no sense.
    BadRequest,
    UnknownProfile,
    /// Nothing was changed.
    Refused(Vec<RefusedProblem>),
    /// The files changed since `base_rev`: "the lists changed on disk; reloaded".
    StaleRev,
}

/// `Error{code, message}`: any request may get it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ErrorReply {
    pub code: ErrorCode,
    pub message: String,
}

impl ErrorReply {
    pub fn write(&self, w: &mut Writer) {
        match &self.code {
            ErrorCode::BadRequest => w.u64(0),
            ErrorCode::UnknownProfile => w.u64(1),
            ErrorCode::Refused(problems) => {
                w.u64(2);
                w.list(problems, RefusedProblem::write);
            }
            ErrorCode::StaleRev => w.u64(3),
        }
        w.str(&self.message);
    }

    pub fn read(r: &mut Reader) -> Result<Self, WireError> {
        let code = match r.u64()? {
            0 => ErrorCode::BadRequest,
            1 => ErrorCode::UnknownProfile,
            2 => ErrorCode::Refused(r.list(RefusedProblem::read)?),
            3 => ErrorCode::StaleRev,
            _ => return Err(WireError::Invalid("an unknown error code")),
        };
        Ok(Self {
            code,
            message: r.string()?,
        })
    }
}
