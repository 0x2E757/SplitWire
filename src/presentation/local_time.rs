//! Wall-clock texts in the user's time zone: `09:14`, `11:27:41`, `yesterday 22:10`,
//! `14.10.2026`. The offset comes from the platform, so this stays pure.

use std::time::{SystemTime, UNIX_EPOCH};

use crate::clock::{format_civil, unix_secs};

/// "Now" and the local offset from UTC the texts are written in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LocalTime {
    pub now: SystemTime,
    pub offset_secs: i64,
}

impl LocalTime {
    pub fn new(now: SystemTime, offset_secs: i64) -> Self {
        Self { now, offset_secs }
    }

    fn local(&self, at: SystemTime) -> u64 {
        (unix_secs(at) as i64 + self.offset_secs).max(0) as u64
    }

    /// `YYYY-MM-DD HH:MM:SS` in local time.
    fn civil(&self, at: SystemTime) -> String {
        format_civil(self.local(at))
    }

    /// `09:14`.
    pub fn clock(&self, at: SystemTime) -> String {
        self.civil(at)[11..16].to_owned()
    }

    /// `11:27:41`.
    pub fn clock_seconds(&self, at: SystemTime) -> String {
        self.civil(at)[11..19].to_owned()
    }

    /// `11:27:41.118`, for log lines.
    pub fn clock_millis(&self, at: SystemTime) -> String {
        let millis = at
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.subsec_millis());
        format!("{}.{millis:03}", self.clock_seconds(at))
    }

    /// `2026-10-19`.
    pub fn iso_date(&self, at: SystemTime) -> String {
        self.civil(at)[..10].to_owned()
    }

    /// `14.10.2026`.
    pub fn date(&self, at: SystemTime) -> String {
        let civil = self.civil(at);
        format!("{}.{}.{}", &civil[8..10], &civil[5..7], &civil[..4])
    }

    /// Days between the local dates of `at` and now; 0 is today.
    pub fn days_ago(&self, at: SystemTime) -> i64 {
        (self.local(self.now) / 86_400) as i64 - (self.local(at) / 86_400) as i64
    }

    /// `today 11:25`, `yesterday 22:10`, `17.10.2026 13:40`.
    pub fn day_clock(&self, at: SystemTime) -> String {
        match self.days_ago(at) {
            0 => format!("today {}", self.clock(at)),
            1 => format!("yesterday {}", self.clock(at)),
            _ => format!("{} {}", self.date(at), self.clock(at)),
        }
    }

    /// The title of a day separator: `Today 19.10.2026`, `Saturday 17.10.2026`.
    pub fn day_title(&self, at: SystemTime) -> String {
        // 1970-01-01 was a Thursday.
        const DAYS: [&str; 7] = [
            "Thursday",
            "Friday",
            "Saturday",
            "Sunday",
            "Monday",
            "Tuesday",
            "Wednesday",
        ];
        let name = if self.days_ago(at) == 0 {
            "Today"
        } else {
            DAYS[(self.local(at) / 86_400 % 7) as usize]
        };
        format!("{name} {}", self.date(at))
    }

    /// Seconds from `at` to now, 0 for a time in the future.
    pub fn age(&self, at: SystemTime) -> u64 {
        self.now.duration_since(at).map_or(0, |d| d.as_secs())
    }

    /// Seconds from now to `at`, 0 for a time in the past.
    pub fn until(&self, at: SystemTime) -> u64 {
        at.duration_since(self.now).map_or(0, |d| d.as_secs())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    // 2026-10-19 11:28:55 UTC.
    const NOW: u64 = 1_792_409_335;

    fn time() -> LocalTime {
        LocalTime::new(UNIX_EPOCH + Duration::from_secs(NOW), 0)
    }

    fn ago(secs: u64) -> SystemTime {
        UNIX_EPOCH + Duration::from_secs(NOW - secs)
    }

    #[test]
    fn clocks_and_dates() {
        let time = time();
        assert_eq!(time.clock(ago(0)), "11:28");
        assert_eq!(time.clock_seconds(ago(74)), "11:27:41");
        assert_eq!(time.date(ago(0)), "19.10.2026");
        assert_eq!(time.iso_date(ago(0)), "2026-10-19");
        assert_eq!(
            time.clock_millis(ago(0) + Duration::from_millis(118)),
            "11:28:55.118"
        );
    }

    #[test]
    fn days_are_named_relative_to_today() {
        let time = time();
        assert_eq!(time.day_clock(ago(3 * 60 + 55)), "today 11:25");
        assert_eq!(
            time.day_clock(ago(13 * 3600 + 18 * 60 + 55)),
            "yesterday 22:10"
        );
        assert_eq!(time.day_title(ago(0)), "Today 19.10.2026");
        assert_eq!(time.day_title(ago(86_400)), "Sunday 18.10.2026");
        assert_eq!(time.day_title(ago(2 * 86_400)), "Saturday 17.10.2026");
    }

    #[test]
    fn the_offset_shifts_local_time() {
        let time = LocalTime::new(UNIX_EPOCH + Duration::from_secs(NOW), 3 * 3600);
        assert_eq!(time.clock(time.now), "14:28");
        assert_eq!(time.age(ago(42)), 42);
        assert_eq!(time.until(ago(0) + Duration::from_secs(60)), 60);
    }
}
