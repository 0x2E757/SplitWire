//! `logs\splitwire.log` read back for the Logs page's file view, in the one format the service's
//! log writes (`logs::LogLineText`). A line that does not parse is kept whole as a warning, so
//! nothing in the file is hidden.

use std::time::UNIX_EPOCH;

use crate::ipc::LogLine;
use crate::logs::{Level, LogLineText};

pub struct LogFile;

impl LogFile {
    /// The lines of `text`, oldest first.
    pub fn parse(text: &str) -> Vec<LogLine> {
        text.lines()
            .filter(|l| !l.trim().is_empty())
            .enumerate()
            .map(|(index, line)| {
                let seq = index as u64 + 1;
                match LogLineText::parse(line) {
                    Some(parsed) => LogLine {
                        seq,
                        time: parsed.time,
                        level: parsed.level,
                        profile: parsed.profile,
                        source: parsed.source,
                        message: parsed.message,
                    },
                    None => LogLine {
                        seq,
                        time: UNIX_EPOCH,
                        level: Level::Warn,
                        profile: None,
                        source: String::new(),
                        message: line.to_owned(),
                    },
                }
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::clock::ManualClock;
    use crate::logs::{Log, LogFile as ServiceLogFile};
    use crate::presentation::transport::demo::{DemoTransport, Scenario};
    use std::sync::Arc;
    use std::time::Duration;

    #[test]
    fn reads_the_demo_file_back() {
        let lines = LogFile::parse(&DemoTransport::log_file(Scenario::Normal));
        assert_eq!(lines[0].level, Level::Warn);
        assert_eq!(lines[0].source, "nrpt");
        assert_eq!(lines[0].profile, None);
        assert_eq!(lines[1].profile.as_deref(), Some("Contoso"));
        let last = lines.last().unwrap();
        assert_eq!(
            last.time,
            Scenario::at(11, 26, 40) + Duration::from_millis(120)
        );
        assert!(lines.iter().all(|l| l.level != Level::Info));
    }

    #[test]
    fn odd_lines_stay() {
        let lines = LogFile::parse("random text\n\n2026-10-19 12:00:00 ERROR AcmeCorp dns: x\n");
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].message, "random text");
        assert_eq!(lines[1].seq, 2);
        assert_eq!(lines[1].level, Level::Warn);
    }

    /// The service's log writes the file; the window reads every line back as its entry.
    #[test]
    fn reads_what_the_service_log_writes() {
        let folder = std::env::temp_dir().join(format!("splitwire-logboth-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&folder);
        std::fs::create_dir_all(&folder).unwrap();
        let clock = Arc::new(ManualClock::new(Scenario::at(9, 14, 11)));
        let log = Log::new(10_000, clock.clone(), false);
        log.write_to(ServiceLogFile::in_folder(&folder));
        log.with_source("nrpt")
            .warn("Group Policy defines NRPT rules");
        clock.advance(Duration::from_millis(1_060));
        log.with_profile("AcmeCorp")
            .with_source("dns")
            .error("upstream 10.66.0.53: no answer\nfor 30 s");
        log.info("not in the file");
        log.with_profile("HomeLab")
            .warn("profile.conf line 2: mtu = 9000");
        let text = std::fs::read_to_string(folder.join(ServiceLogFile::NAME)).unwrap();
        let lines = LogFile::parse(&text);
        let written = log.since(0, 10);
        let kept: Vec<_> = written.iter().filter(|e| e.level >= Level::Warn).collect();
        assert_eq!(lines.len(), kept.len());
        for (line, entry) in lines.iter().zip(kept) {
            assert_eq!((line.time, line.level), (entry.time, entry.level));
            assert_eq!(line.profile.as_deref(), entry.profile.as_deref());
            assert_eq!(line.source, entry.source);
            assert_eq!(line.message, entry.message.replace('\n', " "));
        }
        let _ = std::fs::remove_dir_all(folder);
    }
}
