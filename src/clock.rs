use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Wall-clock time; behind a trait so expiry and TTLs are tested without waiting.
pub trait Clock: Send + Sync {
    fn now(&self) -> SystemTime;

    /// Seconds local time is ahead of UTC at `at`; UTC unless the clock knows the time zone.
    fn utc_offset_secs(&self, _at: SystemTime) -> i64 {
        0
    }

    /// Waits `duration`; a manual clock moves on instead, so waits are tested without waiting.
    fn sleep(&self, duration: Duration) {
        std::thread::sleep(duration);
    }
}

/// A clock shared with its owner, so a test can move a clock it has handed over.
impl<C: Clock + ?Sized> Clock for Arc<C> {
    fn now(&self) -> SystemTime {
        (**self).now()
    }

    fn utc_offset_secs(&self, at: SystemTime) -> i64 {
        (**self).utc_offset_secs(at)
    }

    fn sleep(&self, duration: Duration) {
        (**self).sleep(duration);
    }
}

/// The local midnight that began the day of `at`, for a local time `offset_secs` ahead of UTC:
/// "today" of the alert history.
pub fn local_midnight(at: SystemTime, offset_secs: i64) -> SystemTime {
    let unix = i64::try_from(unix_secs(at)).unwrap_or(i64::MAX);
    let into_day = (unix + offset_secs).rem_euclid(86_400) as u64;
    at.checked_sub(Duration::from_secs(into_day))
        .unwrap_or(UNIX_EPOCH)
        .max(UNIX_EPOCH)
}

/// `YYYY-MM-DD HH:MM:SS` in the local time `offset_secs` ahead of UTC.
pub fn format_local(at: SystemTime, offset_secs: i64) -> String {
    let unix = i64::try_from(unix_secs(at)).unwrap_or(i64::MAX);
    format_civil(u64::try_from(unix + offset_secs).unwrap_or(0))
}

pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> SystemTime {
        SystemTime::now()
    }
}

/// A clock that only moves when told to.
pub struct ManualClock {
    now: Mutex<SystemTime>,
}

impl ManualClock {
    pub fn new(start: SystemTime) -> Self {
        Self {
            now: Mutex::new(start),
        }
    }

    pub fn advance(&self, by: Duration) {
        let mut now = self.now.lock().unwrap();
        *now += by;
    }
}

impl Clock for ManualClock {
    fn now(&self) -> SystemTime {
        *self.now.lock().unwrap()
    }

    fn sleep(&self, duration: Duration) {
        self.advance(duration);
    }
}

/// Seconds since the Unix epoch, 0 for times before it.
pub fn unix_secs(time: SystemTime) -> u64 {
    time.duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

/// `YYYY-MM-DD HH:MM:SSZ` (UTC) for Unix seconds; std has no calendar.
pub fn format_utc(unix: u64) -> String {
    format!("{}Z", format_civil(unix))
}

/// `YYYY-MM-DD HH:MM:SS` for seconds counted like Unix time (shift them for local time).
pub fn format_civil(seconds: u64) -> String {
    let (year, month, day) = civil_from_days(seconds / 86_400);
    let rem = seconds % 86_400;
    format!(
        "{year:04}-{month:02}-{day:02} {:02}:{:02}:{:02}",
        rem / 3600,
        rem % 3600 / 60,
        rem % 60
    )
}

/// Days since 1970-01-01 to a proleptic Gregorian date (H. Hinnant's algorithm).
fn civil_from_days(days: u64) -> (u64, u64, u64) {
    let z = days + 719_468;
    let era = z / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + u64::from(month <= 2);
    (year, month, day)
}

/// Days since 1970-01-01 of a proleptic Gregorian date, the inverse of the one above.
pub fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let year = if month <= 2 { year - 1 } else { year };
    let era = year.div_euclid(400);
    let yoe = year - era * 400;
    let mp = (month + 9) % 12;
    let doy = (153 * mp + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_midnight_follows_the_offset() {
        // 2026-10-19 23:30:00Z.
        let at = UNIX_EPOCH + Duration::from_secs(1_792_452_600);
        assert_eq!(unix_secs(local_midnight(at, 0)), 1_792_368_000);
        // At UTC+3 it is already 02:30 on the 20th.
        assert_eq!(
            unix_secs(local_midnight(at, 3 * 3600)),
            1_792_454_400 - 3 * 3600
        );
        // At UTC-5 it is 18:30 on the 19th.
        assert_eq!(
            unix_secs(local_midnight(at, -5 * 3600)),
            1_792_368_000 + 5 * 3600
        );
        assert_eq!(format_local(at, 3 * 3600), "2026-10-20 02:30:00");
        assert_eq!(local_midnight(UNIX_EPOCH, 3600), UNIX_EPOCH);
    }

    #[test]
    fn manual_clock_moves_only_when_advanced() {
        let start = UNIX_EPOCH + Duration::from_secs(1_000);
        let clock = ManualClock::new(start);
        assert_eq!(clock.now(), start);
        clock.advance(Duration::from_secs(5));
        assert_eq!(unix_secs(clock.now()), 1_005);
        clock.sleep(Duration::from_secs(2));
        assert_eq!(unix_secs(clock.now()), 1_007);
    }

    #[test]
    fn formats_utc_dates() {
        assert_eq!(format_utc(0), "1970-01-01 00:00:00Z");
        assert_eq!(format_utc(951_782_400), "2000-02-29 00:00:00Z");
        assert_eq!(format_utc(1_792_423_315), "2026-10-19 15:21:55Z");
        assert_eq!(format_utc(4_102_444_799), "2099-12-31 23:59:59Z");
        for days in [0, 11_016, 20_745, 47_481] {
            let (year, month, day) = civil_from_days(days);
            assert_eq!(
                days_from_civil(year as i64, month as i64, day as i64),
                days as i64
            );
        }
    }
}
