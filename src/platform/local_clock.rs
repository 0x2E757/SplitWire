use std::time::{SystemTime, UNIX_EPOCH};

use super::abi::{
    FileTimeToSystemTime, SystemTimeParts, SystemTimeToFileTime, SystemTimeToTzSpecificLocalTime,
};
use crate::clock::Clock;

/// The system clock and the machine's time zone: alerts count "today" from local midnight, and
/// the command line prints local times.
pub struct LocalClock;

impl LocalClock {
    /// 100 ns intervals from 1601-01-01 to 1970-01-01.
    const UNIX_IN_FILETIME: u64 = 116_444_736_000_000_000;

    /// Seconds local time is ahead of UTC at `at` (daylight saving included); 0 when Windows
    /// cannot tell.
    pub fn offset_secs(at: SystemTime) -> i64 {
        let since = at.duration_since(UNIX_EPOCH).unwrap_or_default();
        let utc = Self::UNIX_IN_FILETIME + (since.as_nanos() / 100) as u64;
        let mut parts = SystemTimeParts::default();
        let mut local_parts = SystemTimeParts::default();
        let mut local = 0u64;
        let converted = unsafe {
            FileTimeToSystemTime(&utc, &mut parts) != 0
                && SystemTimeToTzSpecificLocalTime(std::ptr::null_mut(), &parts, &mut local_parts)
                    != 0
                && SystemTimeToFileTime(&local_parts, &mut local) != 0
        };
        if !converted {
            return 0;
        }
        // Both sides are whole milliseconds apart from `utc`'s sub-millisecond part.
        let utc_ms = (utc / 10_000) as i64;
        let local_ms = (local / 10_000) as i64;
        (local_ms - utc_ms).div_euclid(1000)
    }
}

impl Clock for LocalClock {
    fn now(&self) -> SystemTime {
        SystemTime::now()
    }

    fn utc_offset_secs(&self, at: SystemTime) -> i64 {
        Self::offset_secs(at)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_offset_is_whole_minutes_within_a_day() {
        let offset = LocalClock::offset_secs(SystemTime::now());
        assert_eq!(offset % 60, 0);
        assert!(offset.abs() <= 14 * 3600, "{offset}");
    }
}
