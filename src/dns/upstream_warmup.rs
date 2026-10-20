use std::time::{Duration, SystemTime};

/// A profile's upstream gets patience while its tunnel is fresh or cold: until the tunnel's first
/// handshake and for `GRACE` after it (or after the Connect, when that came later), and again
/// whenever the last handshake is older than `COLD_AFTER`. Then a failed exchange is retried
/// within `BUDGET` before the client gets SERVFAIL, and only a failure that spent its budget
/// counts toward `upstream_unreachable`.
#[derive(Clone, Copy, Debug)]
pub struct UpstreamWarmup {
    since: SystemTime,
    first: Option<SystemTime>,
    last: Option<SystemTime>,
}

impl UpstreamWarmup {
    pub const GRACE: Duration = Duration::from_secs(15);
    /// WireGuard's reject-after time: an older handshake's keys carry nothing, so the next packet
    /// waits for a new handshake, whose first initiation may be lost.
    pub const COLD_AFTER: Duration = Duration::from_secs(180);
    /// The longest a query is retried for; an attempt that would end later is not started. It
    /// outlasts WireGuard's 5 s before it repeats a lost initiation.
    pub const BUDGET: Duration = Duration::from_secs(10);
    /// Between two attempts, so an error that comes at once does not spin.
    pub const PAUSE: Duration = Duration::from_millis(500);

    pub fn new(since: SystemTime) -> Self {
        Self {
            since,
            first: None,
            last: None,
        }
    }

    /// A handshake the driver reported: the first starts the grace, the newest tells when the
    /// tunnel is cold.
    pub fn handshake(&mut self, at: SystemTime) {
        self.first.get_or_insert(at);
        self.last = self.last.max(Some(at));
    }

    pub fn patient(&self, now: SystemTime) -> bool {
        let (Some(first), Some(last)) = (self.first, self.last) else {
            return true;
        };
        now < first.max(self.since) + Self::GRACE || now > last + Self::COLD_AFTER
    }

    /// Whether another attempt fits in the budget of a query that started at `started`, when
    /// the last attempt took `took`.
    pub fn retry(&self, started: SystemTime, now: SystemTime, took: Duration) -> bool {
        let spent = now.duration_since(started).unwrap_or_default();
        self.patient(now) && spent + Self::PAUSE + took <= Self::BUDGET
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::UNIX_EPOCH;

    fn at(secs: u64) -> SystemTime {
        UNIX_EPOCH + Duration::from_secs(secs)
    }

    #[test]
    fn patient_until_the_handshake_and_for_fifteen_seconds_after_it() {
        let mut warmup = UpstreamWarmup::new(at(100));
        assert!(warmup.patient(at(1_000)), "no handshake yet");
        warmup.handshake(at(140));
        warmup.handshake(at(900));
        assert!(warmup.patient(at(154)));
        assert!(!warmup.patient(at(155)), "the first handshake counts");
        // The handshake came during the Connect's wait: the grace counts from the Connect.
        let mut early = UpstreamWarmup::new(at(110));
        early.handshake(at(103));
        assert!(early.patient(at(124)));
        assert!(!early.patient(at(125)));
    }

    #[test]
    fn patient_again_while_the_last_handshake_is_older_than_wireguard_rejects() {
        let mut warmup = UpstreamWarmup::new(at(100));
        warmup.handshake(at(100));
        assert!(!warmup.patient(at(280)));
        assert!(warmup.patient(at(281)), "cold");
        // A late report of an older handshake does not warm the tunnel up.
        warmup.handshake(at(90));
        assert!(warmup.patient(at(281)));
        warmup.handshake(at(285));
        assert!(!warmup.patient(at(286)), "warm again, without a new grace");
        assert!(warmup.patient(at(466)));
    }

    #[test]
    fn retries_only_while_the_next_attempt_fits_the_budget() {
        let warmup = UpstreamWarmup::new(at(100));
        // A 2 s timeout: the fourth attempt ends at 9.5 s, a fifth would end at 12 s.
        let started = at(100);
        let two = Duration::from_secs(2);
        assert!(warmup.retry(started, at(107), two));
        assert!(!warmup.retry(started, at(107) + Duration::from_millis(2_500), two));
        // An error at once: retried until 9.5 s are spent.
        assert!(warmup.retry(
            started,
            at(109) + Duration::from_millis(500),
            Duration::ZERO
        ));
        assert!(!warmup.retry(started, at(110), Duration::ZERO));
        let mut warm = UpstreamWarmup::new(at(100));
        warm.handshake(at(100));
        assert!(!warm.retry(at(120), at(120), Duration::ZERO), "warmed up");
    }
}
