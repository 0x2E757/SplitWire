use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

/// What ends a run: Ctrl+C, the `--for` time, or (later) the service control manager.
#[derive(Clone, Default)]
pub struct StopSignal {
    inner: Arc<(Mutex<Option<String>>, Condvar)>,
}

impl StopSignal {
    /// Asks the run to stop; the first reason wins.
    pub fn stop(&self, reason: &str) {
        let mut slot = self.inner.0.lock().unwrap();
        slot.get_or_insert_with(|| reason.to_owned());
        self.inner.1.notify_all();
    }

    /// Waits for a stop, or until `limit` passes; returns why it ended.
    pub fn wait(&self, limit: Option<Duration>) -> String {
        let deadline = limit.map(|limit| Instant::now() + limit);
        let mut slot = self.inner.0.lock().unwrap();
        loop {
            if let Some(reason) = slot.as_ref() {
                return reason.clone();
            }
            match deadline {
                Some(deadline) => {
                    let left = deadline.saturating_duration_since(Instant::now());
                    if left.is_zero() {
                        return "the --for time is up".into();
                    }
                    slot = self.inner.1.wait_timeout(slot, left).unwrap().0;
                }
                None => slot = self.inner.1.wait(slot).unwrap(),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    #[test]
    fn ends_on_a_stop_or_at_the_limit() {
        let signal = StopSignal::default();
        assert_eq!(
            signal.wait(Some(Duration::from_millis(10))),
            "the --for time is up"
        );
        let other = signal.clone();
        let stopper = thread::spawn(move || other.stop("Ctrl+C"));
        assert_eq!(signal.wait(None), "Ctrl+C");
        stopper.join().unwrap();
        signal.stop("later");
        assert_eq!(signal.wait(None), "Ctrl+C");
    }
}
