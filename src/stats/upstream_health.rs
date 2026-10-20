use std::sync::Mutex;
use std::time::{Duration, SystemTime};

/// The last time a profile's upstream DNS answered and the last time it failed.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct UpstreamHealthState {
    pub last_success: Option<SystemTime>,
    pub last_failure: Option<SystemTime>,
    pub last_error: Option<String>,
    /// Failures since the last answer.
    pub consecutive_failures: u32,
    /// How long the last answer took ("DNS 18 ms").
    pub rtt: Option<Duration>,
}

impl UpstreamHealthState {
    pub const FAILURES: u32 = 3;
    pub const SILENCE: Duration = Duration::from_secs(30);

    /// Three failures in a row, or failing with no answer for 30 s since the last
    /// answer (or since `since`, when it never answered).
    pub fn unreachable(&self, since: SystemTime) -> bool {
        if self.consecutive_failures == 0 {
            return false;
        }
        let silent = self.last_failure.is_some_and(|failure| {
            let reference = self.last_success.unwrap_or(since);
            failure.duration_since(reference).unwrap_or_default() >= Self::SILENCE
        });
        self.consecutive_failures >= Self::FAILURES || silent
    }
}

#[derive(Default)]
pub struct UpstreamHealth {
    state: Mutex<UpstreamHealthState>,
}

impl UpstreamHealth {
    /// `true` for the first answer ever, or the first since failures: the clients may remember
    /// those failures.
    pub fn record_success(&self, at: SystemTime, rtt: Duration) -> bool {
        let mut state = self.state.lock().unwrap();
        let recovered = state.last_success.is_none() || state.consecutive_failures > 0;
        state.last_success = Some(at);
        state.consecutive_failures = 0;
        state.rtt = Some(rtt);
        recovered
    }

    pub fn record_failure(&self, at: SystemTime, error: String) {
        let mut state = self.state.lock().unwrap();
        state.last_failure = Some(at);
        state.last_error = Some(error);
        state.consecutive_failures += 1;
    }

    pub fn snapshot(&self) -> UpstreamHealthState {
        self.state.lock().unwrap().clone()
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
    fn keeps_the_latest_success_and_failure() {
        let health = UpstreamHealth::default();
        assert!(health.record_success(at(0), Duration::from_millis(18)));
        assert!(!health.record_success(at(0), Duration::from_millis(18)));
        health.record_failure(at(1), "timeout".into());
        let state = health.snapshot();
        assert_eq!(state.last_success, Some(at(0)));
        assert_eq!(state.last_error.as_deref(), Some("timeout"));
        assert_eq!(state.rtt, Some(Duration::from_millis(18)));
        assert_eq!(state.consecutive_failures, 1);
        assert!(health.record_success(at(2), Duration::from_millis(18)));
    }

    #[test]
    fn unreachable_after_three_failures_or_thirty_silent_seconds() {
        let health = UpstreamHealth::default();
        health.record_success(at(100), Duration::from_millis(5));
        health.record_failure(at(101), "timeout".into());
        health.record_failure(at(102), "timeout".into());
        assert!(!health.snapshot().unreachable(at(0)));
        health.record_failure(at(103), "timeout".into());
        assert!(health.snapshot().unreachable(at(0)));
        health.record_success(at(104), Duration::from_millis(5));
        assert!(!health.snapshot().unreachable(at(0)));
        health.record_failure(at(134), "timeout".into());
        assert!(health.snapshot().unreachable(at(0)));

        let never = UpstreamHealth::default();
        never.record_failure(at(10), "timeout".into());
        assert!(!never.snapshot().unreachable(at(0)));
        never.record_failure(at(31), "timeout".into());
        assert!(never.snapshot().unreachable(at(0)));
    }
}
