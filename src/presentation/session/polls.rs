//! How often each surface asks the engine: `Status` every second (every 10 s while
//! the window is hidden), the page's own data on top, never two of a kind in flight.

use std::time::{Duration, Instant};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Poll {
    Status,
    Traffic,
    Detail,
    Routes,
    Lists,
    Settings,
    Logs,
}

impl Poll {
    const ALL: [Self; 7] = [
        Self::Status,
        Self::Traffic,
        Self::Detail,
        Self::Routes,
        Self::Lists,
        Self::Settings,
        Self::Logs,
    ];

    fn index(self) -> usize {
        Self::ALL.iter().position(|p| *p == self).unwrap_or(0)
    }
}

#[derive(Clone, Copy, Debug, Default)]
struct PollState {
    last: Option<Instant>,
    in_flight: bool,
    /// Asked for once, out of schedule (a revision changed, a page opened).
    due: bool,
}

#[derive(Debug, Default)]
pub struct Polls {
    states: [PollState; 7],
}

impl Polls {
    pub fn interval(poll: Poll, visible: bool) -> Option<Duration> {
        match (poll, visible) {
            (Poll::Status, true) => Some(Duration::from_secs(1)),
            (Poll::Status, false) => Some(Duration::from_secs(10)),
            (_, false) => None,
            (Poll::Traffic | Poll::Detail | Poll::Logs, true) => Some(Duration::from_secs(1)),
            (Poll::Routes, true) => Some(Duration::from_secs(2)),
            // Lists and settings are asked for when their revision changes.
            (Poll::Lists | Poll::Settings, true) => None,
        }
    }

    pub fn request(&mut self, poll: Poll) {
        self.states[poll.index()].due = true;
    }

    /// Whether `poll` should be sent now; `wanted` says the page needs it at all.
    pub fn ready(&self, poll: Poll, wanted: bool, visible: bool, now: Instant) -> bool {
        let state = self.states[poll.index()];
        if state.in_flight || !wanted {
            return false;
        }
        if state.due {
            return true;
        }
        match (Self::interval(poll, visible), state.last) {
            (Some(_), None) => true,
            (Some(interval), Some(last)) => now.duration_since(last) >= interval,
            (None, _) => false,
        }
    }

    pub fn sent(&mut self, poll: Poll, now: Instant) {
        let state = &mut self.states[poll.index()];
        state.in_flight = true;
        state.due = false;
        state.last = Some(now);
    }

    pub fn answered(&mut self, poll: Poll) {
        self.states[poll.index()].in_flight = false;
    }

    /// The service went away: every poll starts over.
    pub fn reset(&mut self) {
        self.states = Default::default();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn polls_wait_for_their_interval_and_their_answer() {
        let mut polls = Polls::default();
        let start = Instant::now();
        assert!(polls.ready(Poll::Status, true, true, start));
        polls.sent(Poll::Status, start);
        assert!(!polls.ready(Poll::Status, true, true, start + Duration::from_secs(5)));
        polls.answered(Poll::Status);
        assert!(!polls.ready(Poll::Status, true, true, start + Duration::from_millis(500)));
        assert!(polls.ready(Poll::Status, true, true, start + Duration::from_secs(1)));
        assert!(!polls.ready(Poll::Status, true, false, start + Duration::from_secs(5)));
        assert!(polls.ready(Poll::Status, true, false, start + Duration::from_secs(10)));
        assert!(!polls.ready(Poll::Lists, true, true, start));
        polls.request(Poll::Lists);
        assert!(polls.ready(Poll::Lists, true, true, start));
        assert!(!polls.ready(Poll::Routes, false, true, start));
    }
}
