use std::time::{Duration, SystemTime, UNIX_EPOCH};

use super::Level;
use crate::clock::{days_from_civil, format_civil, unix_secs};

/// One parsed line of `logs\splitwire.log`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LogFileLine {
    pub time: SystemTime,
    pub level: Level,
    pub profile: Option<String>,
    pub source: String,
    pub message: String,
}

/// The one format of a log line, written by the service's log (the file and the console echo) and
/// read back by the window's Logs page: `2026-10-19 09:14:11.060Z WARN  - nrpt: message`, the
/// time in UTC with milliseconds, the level padded to 5, the profile or `-`, the source, the
/// message on the same line.
pub struct LogLineText;

impl LogLineText {
    pub fn format(
        time: SystemTime,
        level: Level,
        profile: Option<&str>,
        source: &str,
        message: &str,
    ) -> String {
        let millis = time
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.subsec_millis());
        // A line break would split the entry into lines the reader cannot place.
        let message = message.replace(['\r', '\n'], " ");
        format!(
            "{}.{millis:03}Z {level:<5} {} {source}: {message}",
            format_civil(unix_secs(time)),
            profile.unwrap_or("-")
        )
    }

    /// The line's parts, `None` when it is not in the format.
    pub fn parse(line: &str) -> Option<LogFileLine> {
        let (date, rest) = line.split_once(' ')?;
        let (clock, rest) = rest.split_once(' ')?;
        let (level, rest) = rest.trim_start().split_once(' ')?;
        let (profile, rest) = rest.trim_start().split_once(' ')?;
        let (source, message) = rest.split_once(": ")?;
        let level = match level {
            "INFO" => Level::Info,
            "WARN" => Level::Warn,
            "ERROR" => Level::Error,
            _ => return None,
        };
        Some(LogFileLine {
            time: Self::time(date, clock)?,
            level,
            profile: (profile != "-").then(|| profile.to_owned()),
            source: source.to_owned(),
            message: message.trim_end_matches('\r').to_owned(),
        })
    }

    fn time(date: &str, clock: &str) -> Option<SystemTime> {
        let number = |text: &str| text.parse::<i64>().ok();
        let mut date = date.split('-');
        let (year, month, day) = (
            number(date.next()?)?,
            number(date.next()?)?,
            number(date.next()?)?,
        );
        let (clock, millis) = clock.strip_suffix('Z')?.split_once('.')?;
        let mut parts = clock.split(':');
        let (hour, minute, second) = (
            number(parts.next()?)?,
            number(parts.next()?)?,
            number(parts.next()?)?,
        );
        let secs = days_from_civil(year, month, day) * 86_400 + hour * 3600 + minute * 60 + second;
        let millis = number(millis)?.clamp(0, 999);
        u64::try_from(secs)
            .ok()
            .map(|s| UNIX_EPOCH + Duration::from_secs(s) + Duration::from_millis(millis as u64))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_line_reads_back_as_written() {
        // 2026-10-19 09:14:11.060Z.
        let at = UNIX_EPOCH + Duration::from_millis(1_792_401_251_060);
        let line = LogLineText::format(at, Level::Warn, None, "nrpt", "a rule: kept\r\nagain");
        assert_eq!(
            line,
            "2026-10-19 09:14:11.060Z WARN  - nrpt: a rule: kept  again"
        );
        let parsed = LogLineText::parse(&line).unwrap();
        assert_eq!(parsed.time, at);
        assert_eq!(parsed.level, Level::Warn);
        assert_eq!(parsed.profile, None);
        assert_eq!(parsed.source, "nrpt");
        assert_eq!(parsed.message, "a rule: kept  again");
        let error = LogLineText::format(at, Level::Error, Some("AcmeCorp"), "dns", "x");
        let parsed = LogLineText::parse(&error).unwrap();
        assert_eq!(
            (parsed.level, parsed.profile.as_deref()),
            (Level::Error, Some("AcmeCorp"))
        );
    }

    #[test]
    fn other_text_is_not_a_line() {
        for text in [
            "random text",
            "2026-10-19 09:14:11Z WARN  - nrpt: no milliseconds",
            "2026-10-19 09:14:11.060 WARN  - nrpt: local time",
            "2026-10-19 09:14:11.060Z NOTE  - nrpt: level",
        ] {
            assert_eq!(LogLineText::parse(text), None, "{text}");
        }
    }
}
