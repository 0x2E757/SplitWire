use std::time::SystemTime;

use crate::clock::format_local;

/// Times and amounts as the command line prints them: local times, short durations and rates.
#[derive(Clone, Copy)]
pub struct Times {
    pub now: SystemTime,
    /// Seconds local time is ahead of UTC.
    pub offset_secs: i64,
}

impl Times {
    pub fn at(&self, time: SystemTime) -> String {
        format_local(time, self.offset_secs)
    }

    /// `45 s`, `12 min`, `3 h 5 min`, `2 d 4 h`.
    pub fn span(secs: u64) -> String {
        match secs {
            0..60 => format!("{secs} s"),
            60..3600 => format!("{} min", secs / 60),
            3600..86_400 => match secs % 3600 / 60 {
                0 => format!("{} h", secs / 3600),
                min => format!("{} h {min} min", secs / 3600),
            },
            _ => format!("{} d {} h", secs / 86_400, secs % 86_400 / 3600),
        }
    }

    /// `512 B/s`, `2.0 KB/s`, `1.3 MB/s`.
    pub fn rate(bytes_per_sec: u64) -> String {
        format!("{}/s", Self::bytes(bytes_per_sec))
    }

    pub fn bytes(bytes: u64) -> String {
        const UNITS: [&str; 4] = ["KB", "MB", "GB", "TB"];
        if bytes < 1024 {
            return format!("{bytes} B");
        }
        let mut value = bytes as f64 / 1024.0;
        let mut unit = 0;
        while value >= 1024.0 && unit < UNITS.len() - 1 {
            value /= 1024.0;
            unit += 1;
        }
        format!("{value:.1} {}", UNITS[unit])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, UNIX_EPOCH};

    #[test]
    fn prints_spans_rates_and_local_times() {
        assert_eq!(Times::span(45), "45 s");
        assert_eq!(Times::span(720), "12 min");
        assert_eq!(Times::span(3 * 3600), "3 h");
        assert_eq!(Times::span(3 * 3600 + 300), "3 h 5 min");
        assert_eq!(Times::span(2 * 86_400 + 4 * 3600), "2 d 4 h");
        assert_eq!(Times::rate(512), "512 B/s");
        assert_eq!(Times::rate(2048), "2.0 KB/s");
        assert_eq!(Times::bytes(3 << 30), "3.0 GB");
        let times = Times {
            now: UNIX_EPOCH,
            offset_secs: 3 * 3600,
        };
        let at = UNIX_EPOCH + Duration::from_secs(1_792_452_600);
        assert_eq!(times.at(at), "2026-10-20 02:30:00");
    }
}
