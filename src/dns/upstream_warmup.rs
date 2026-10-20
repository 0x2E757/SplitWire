use std::time::{Duration, SystemTime};

/// A freshly connected profile's upstream gets patience: until its tunnel's first
/// handshake, and for `GRACE` after it (or after the Connect, when that came later), a failed
/// exchange is retried within `BUDGET` before the client gets SERVFAIL. Only a failure that spent
/// its budget counts toward `upstream_unreachable` then.
#[derive(Clone, Copy, Debug)]
pub struct UpstreamWarmup {
    since: SystemTime,
    handshake: Option<SystemTime>,
}

impl UpstreamWarmup {
    pub const GRACE: Duration = Duration::from_secs(15);
    /// The longest a query is retried for; an attempt that would end later is not started.
    pub const BUDGET: Duration = Duration::from_secs(5);
    /// Between two attempts, so an error that comes at once does not spin.
    pub const PAUSE: Duration = Duration::from_millis(500);

    pub fn new(since: SystemTime) -> Self {
        Self {
            since,
            handshake: None,
        }
    }

    /// The tunnel's first handshake; later ones change nothing.
    pub fn handshake(&mut self, at: SystemTime) {
        self.handshake.get_or_insert(at);
    }

    pub fn patient(&self, now: SystemTime) -> bool {
        match self.handshake {
            None => true,
            Some(at) => now < at.max(self.since) + Self::GRACE,
        }
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
    fn retries_only_while_the_next_attempt_fits_the_budget() {
        let warmup = UpstreamWarmup::new(at(100));
        // A 2 s timeout: the second attempt ends at 4.5 s, a third would end at 7 s.
        let started = at(100);
        let two = Duration::from_secs(2);
        assert!(warmup.retry(started, at(102), two));
        assert!(!warmup.retry(started, at(102) + Duration::from_millis(2_500), two));
        // An error at once: retried until 4.5 s are spent.
        assert!(warmup.retry(
            started,
            at(104) + Duration::from_millis(500),
            Duration::ZERO
        ));
        assert!(!warmup.retry(started, at(105), Duration::ZERO));
        let mut warm = UpstreamWarmup::new(at(100));
        warm.handshake(at(100));
        assert!(!warm.retry(at(120), at(120), Duration::ZERO), "warmed up");
    }
}
