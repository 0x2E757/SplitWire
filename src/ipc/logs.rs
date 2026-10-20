use std::time::SystemTime;

use super::domain_wire::DomainWire;
use super::{Reader, WireError, Writer};
use crate::logs::{Level, LogEntry};

/// A log line on the wire: `LogEntry` with an owned source, since a decoded line cannot borrow a
/// `'static` one.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LogLine {
    pub seq: u64,
    pub time: SystemTime,
    pub level: Level,
    pub profile: Option<String>,
    pub source: String,
    pub message: String,
}

impl From<&LogEntry> for LogLine {
    fn from(entry: &LogEntry) -> Self {
        Self {
            seq: entry.seq,
            time: entry.time,
            level: entry.level,
            profile: entry.profile.as_deref().map(str::to_owned),
            source: entry.source.to_owned(),
            message: entry.message.clone(),
        }
    }
}

impl LogLine {
    fn write(&self, w: &mut Writer) {
        w.u64(self.seq);
        w.time(self.time);
        w.choice(&self.level, &DomainWire::LEVELS);
        w.opt_str(self.profile.as_ref());
        w.str(&self.source);
        w.str(&self.message);
    }

    fn read(r: &mut Reader) -> Result<Self, WireError> {
        Ok(Self {
            seq: r.u64()?,
            time: r.time()?,
            level: r.choice(&DomainWire::LEVELS)?,
            profile: r.opt_string()?,
            source: r.string()?,
            message: r.string()?,
        })
    }
}

/// The answer to `Logs{after}`: the lines after `after`, oldest first.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LogsPage {
    pub entries: Vec<LogLine>,
    /// The newest line's number, for the next `after`.
    pub last_seq: u64,
}

impl LogsPage {
    pub const MAX_ENTRIES: usize = 2000;

    pub fn write(&self, w: &mut Writer) {
        w.list(&self.entries, LogLine::write);
        w.u64(self.last_seq);
    }

    pub fn read(r: &mut Reader) -> Result<Self, WireError> {
        Ok(Self {
            entries: r.list(LogLine::read)?,
            last_seq: r.u64()?,
        })
    }
}
