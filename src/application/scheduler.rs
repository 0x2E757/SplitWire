use std::io;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::{Arc, Condvar, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use crate::logs::Log;

/// A periodic job of the scheduler thread.
pub struct Job {
    pub name: &'static str,
    pub period: Duration,
    pub run: Box<dyn Fn() + Send>,
}

#[derive(Default)]
struct Signals {
    stop: bool,
    wake: bool,
}

/// Wakes the scheduler so every job runs at once (resume from sleep).
#[derive(Clone)]
pub struct SchedulerWaker {
    signals: Arc<(Mutex<Signals>, Condvar)>,
}

impl SchedulerWaker {
    pub fn wake(&self) {
        self.signals.0.lock().unwrap().wake = true;
        self.signals.1.notify_all();
    }
}

/// One thread for the periodic jobs: each runs every `period`, the first time one
/// period after the start. A job that panics is logged and runs again next time.
pub struct Scheduler {
    signals: Arc<(Mutex<Signals>, Condvar)>,
    thread: JoinHandle<()>,
}

impl Scheduler {
    pub fn start(jobs: Vec<Job>, log: Log) -> io::Result<Self> {
        let signals = Arc::new((Mutex::new(Signals::default()), Condvar::new()));
        let shared = signals.clone();
        let thread = thread::Builder::new()
            .name("scheduler".into())
            .spawn(move || Self::run(&jobs, &shared, &log))?;
        Ok(Self { signals, thread })
    }

    pub fn waker(&self) -> SchedulerWaker {
        SchedulerWaker {
            signals: self.signals.clone(),
        }
    }

    /// Stops after the job running now, if any.
    pub fn stop(self) {
        self.signals.0.lock().unwrap().stop = true;
        self.signals.1.notify_all();
        let _ = self.thread.join();
    }

    fn run(jobs: &[Job], signals: &(Mutex<Signals>, Condvar), log: &Log) {
        let start = Instant::now();
        let mut due: Vec<Instant> = jobs.iter().map(|job| start + job.period).collect();
        loop {
            let next = due
                .iter()
                .min()
                .copied()
                .unwrap_or(start + Duration::from_secs(3600));
            let mut state = signals.0.lock().unwrap();
            while !state.stop && !state.wake && Instant::now() < next {
                let left = next.saturating_duration_since(Instant::now());
                state = signals.1.wait_timeout(state, left).unwrap().0;
            }
            if state.stop {
                return;
            }
            let woken = std::mem::take(&mut state.wake);
            drop(state);
            let now = Instant::now();
            for (job, due) in jobs.iter().zip(due.iter_mut()) {
                if woken || *due <= now {
                    if catch_unwind(AssertUnwindSafe(|| (job.run)())).is_err() {
                        log.error(format!("the {} job panicked", job.name));
                    }
                    *due = Instant::now() + job.period;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::clock::SystemClock;
    use std::sync::atomic::{AtomicU32, Ordering};

    #[test]
    fn runs_jobs_on_their_period_and_at_once_when_woken() {
        let fast = Arc::new(AtomicU32::new(0));
        let slow = Arc::new(AtomicU32::new(0));
        let (f, s) = (fast.clone(), slow.clone());
        let log = Log::new(1 << 16, Arc::new(SystemClock), false);
        let scheduler = Scheduler::start(
            vec![
                Job {
                    name: "fast",
                    period: Duration::from_millis(20),
                    run: Box::new(move || {
                        f.fetch_add(1, Ordering::SeqCst);
                    }),
                },
                Job {
                    name: "slow",
                    period: Duration::from_secs(3600),
                    run: Box::new(move || {
                        s.fetch_add(1, Ordering::SeqCst);
                        panic!("a failing job");
                    }),
                },
            ],
            log.clone(),
        )
        .unwrap();
        thread::sleep(Duration::from_millis(150));
        assert!(fast.load(Ordering::SeqCst) >= 2);
        assert_eq!(slow.load(Ordering::SeqCst), 0);
        scheduler.waker().wake();
        let deadline = Instant::now() + Duration::from_secs(5);
        while slow.load(Ordering::SeqCst) == 0 && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(10));
        }
        assert_eq!(slow.load(Ordering::SeqCst), 1);
        scheduler.stop();
        assert!(log.contains("the slow job panicked"));
    }
}
