use std::io::Write;
use std::sync::{Arc, Mutex};

use super::{Level, LogBuffer, LogEntry, LogFile, LogLineText};
use crate::clock::Clock;

/// What every handle of one log shares.
struct Sink {
    buffer: Mutex<LogBuffer>,
    clock: Arc<dyn Clock>,
    echo: bool,
    /// The warnings and errors file, once the data folder is known.
    file: Mutex<Option<LogFile>>,
}

/// A cheap, cloneable handle every component logs through. A handle carries the source and
/// the profile its lines are tagged with; `with_source` and `with_profile` derive tagged handles
/// of the same log. Optionally echoes to stderr (foreground runs).
#[derive(Clone)]
pub struct Log {
    sink: Arc<Sink>,
    source: &'static str,
    profile: Option<Arc<str>>,
}

impl Log {
    /// About 10 MB keeps a day of a busy service.
    pub const DEFAULT_BYTES: usize = 10 << 20;

    pub fn new(max_bytes: usize, clock: Arc<dyn Clock>, echo: bool) -> Self {
        Self {
            sink: Arc::new(Sink {
                buffer: Mutex::new(LogBuffer::new(max_bytes)),
                clock,
                echo,
                file: Mutex::new(None),
            }),
            source: "service",
            profile: None,
        }
    }

    /// A handle of the same log whose lines name `source` (`routes`, `dns`, `nrpt`…).
    pub fn with_source(&self, source: &'static str) -> Self {
        Self {
            source,
            ..self.clone()
        }
    }

    /// A handle of the same log whose lines belong to the profile `name`.
    pub fn with_profile(&self, name: &str) -> Self {
        Self {
            profile: Some(name.into()),
            ..self.clone()
        }
    }

    pub fn info(&self, message: impl Into<String>) {
        self.write(Level::Info, message.into());
    }

    pub fn warn(&self, message: impl Into<String>) {
        self.write(Level::Warn, message.into());
    }

    pub fn error(&self, message: impl Into<String>) {
        self.write(Level::Error, message.into());
    }

    /// From now on, warnings and errors also go to `file`.
    pub fn write_to(&self, file: LogFile) {
        *self.sink.file.lock().unwrap() = Some(file);
    }

    pub fn since(&self, after: u64, limit: usize) -> Vec<LogEntry> {
        self.sink.buffer.lock().unwrap().since(after, limit)
    }

    pub fn last_seq(&self) -> u64 {
        self.sink.buffer.lock().unwrap().last_seq()
    }

    /// True when some kept entry contains `needle` (tests and smoke checks).
    pub fn contains(&self, needle: &str) -> bool {
        self.since(0, usize::MAX)
            .iter()
            .any(|e| e.message.contains(needle))
    }

    fn write(&self, level: Level, message: String) {
        let now = self.sink.clock.now();
        let line =
            || LogLineText::format(now, level, self.profile.as_deref(), self.source, &message);
        if self.sink.echo {
            let _ = writeln!(std::io::stderr(), "{}", line());
        }
        if level >= Level::Warn
            && let Some(file) = self.sink.file.lock().unwrap().as_mut()
        {
            // A log that cannot be written has nowhere to report it; the buffer still has it.
            let _ = file.append(&line());
        }
        self.sink.buffer.lock().unwrap().push(LogEntry {
            seq: 0,
            time: now,
            level,
            profile: self.profile.clone(),
            source: self.source,
            message,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::clock::{ManualClock, unix_secs};
    use std::time::{Duration, UNIX_EPOCH};

    fn log() -> Log {
        let clock = Arc::new(ManualClock::new(UNIX_EPOCH + Duration::from_secs(42)));
        Log::new(10_000, clock, false)
    }

    #[test]
    fn stamps_entries_with_the_clock_and_level() {
        let log = log();
        log.warn("careful");
        let entries = log.since(0, 10);
        assert_eq!(entries[0].level, Level::Warn);
        assert_eq!(unix_secs(entries[0].time), 42);
        assert_eq!(
            (entries[0].source, entries[0].profile.clone()),
            ("service", None)
        );
        assert!(log.contains("care"));
    }

    #[test]
    fn warnings_and_errors_also_go_to_the_file() {
        let folder =
            std::env::temp_dir().join(format!("splitwire-logwrite-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&folder);
        std::fs::create_dir_all(&folder).unwrap();
        let log = log();
        log.info("before the file");
        log.write_to(LogFile::in_folder(&folder));
        log.info("only in memory");
        log.with_profile("AcmeCorp").warn("handshake stale");
        log.with_source("nrpt").error("write failed");
        let text = std::fs::read_to_string(folder.join(LogFile::NAME)).unwrap();
        assert_eq!(
            text,
            "1970-01-01 00:00:42.000Z WARN  AcmeCorp service: handshake stale\r\n\
             1970-01-01 00:00:42.000Z ERROR - nrpt: write failed\r\n"
        );
        // Every line of the file reads back as the entry it came from.
        let entries = log.since(0, 10);
        for (line, entry) in text.lines().zip(entries.iter().skip(2)) {
            let parsed = LogLineText::parse(line).unwrap();
            assert_eq!((parsed.time, parsed.level), (entry.time, entry.level));
            assert_eq!(parsed.profile.as_deref(), entry.profile.as_deref());
            assert_eq!(
                (parsed.source.as_str(), parsed.message.as_str()),
                (entry.source, entry.message.as_str())
            );
        }
        std::fs::remove_dir_all(&folder).unwrap();
    }

    #[test]
    fn derived_handles_tag_their_lines_and_share_the_buffer() {
        let log = log();
        let routes = log.with_profile("AcmeCorp").with_source("routes");
        routes.info("route added 10.0.0.0/8 (static)");
        log.with_source("nrpt").error("write failed");
        let entries = log.since(0, 10);
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].profile.as_deref(), Some("AcmeCorp"));
        assert_eq!(entries[0].source, "routes");
        assert_eq!(
            (entries[1].profile.as_deref(), entries[1].source),
            (None, "nrpt")
        );
        assert_eq!(routes.last_seq(), 2);
    }
}
