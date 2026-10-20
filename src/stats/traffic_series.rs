use std::collections::VecDeque;
use std::time::SystemTime;

/// One sample of a connected profile's traffic: bytes per second received and sent.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TrafficSample {
    pub seq: u64,
    pub rx_bps: u64,
    pub tx_bps: u64,
}

/// The last 300 samples of one profile (the 5-minute chart, one sample a second), made from the
/// tunnel's byte counters.
#[derive(Debug, Default)]
pub struct TrafficSeries {
    samples: VecDeque<TrafficSample>,
    last: Option<(SystemTime, u64, u64)>,
    rx_total: u64,
    tx_total: u64,
}

impl TrafficSeries {
    pub const CAPACITY: usize = 300;

    /// Adds the rates since the previous reading of the totals; the first reading only sets the
    /// reference. A counter that went back (a recreated adapter) restarts from it.
    pub fn record(&mut self, seq: u64, at: SystemTime, rx_total: u64, tx_total: u64) {
        self.rx_total = rx_total;
        self.tx_total = tx_total;
        let Some((before, rx_before, tx_before)) = self.last.replace((at, rx_total, tx_total))
        else {
            return;
        };
        let secs = at.duration_since(before).unwrap_or_default().as_secs_f64();
        if secs <= 0.0 {
            return;
        }
        let rate = |now: u64, then: u64| (now.saturating_sub(then) as f64 / secs).round() as u64;
        if self.samples.len() == Self::CAPACITY {
            self.samples.pop_front();
        }
        self.samples.push_back(TrafficSample {
            seq,
            rx_bps: rate(rx_total, rx_before),
            tx_bps: rate(tx_total, tx_before),
        });
    }

    /// The samples after `seq`, oldest first.
    pub fn after(&self, seq: u64) -> Vec<TrafficSample> {
        self.samples
            .iter()
            .filter(|s| s.seq > seq)
            .copied()
            .collect()
    }

    pub fn latest(&self) -> Option<TrafficSample> {
        self.samples.back().copied()
    }

    /// The byte counters of the last reading.
    pub fn totals(&self) -> (u64, u64) {
        (self.rx_total, self.tx_total)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, UNIX_EPOCH};

    fn at(secs: u64) -> SystemTime {
        UNIX_EPOCH + Duration::from_secs(secs)
    }

    #[test]
    fn turns_counters_into_rates() {
        let mut series = TrafficSeries::default();
        series.record(1, at(10), 1_000, 500);
        assert!(series.latest().is_none());
        series.record(2, at(11), 3_000, 700);
        series.record(3, at(13), 7_000, 700);
        assert_eq!(
            series.after(1),
            [
                TrafficSample {
                    seq: 2,
                    rx_bps: 2_000,
                    tx_bps: 200
                },
                TrafficSample {
                    seq: 3,
                    rx_bps: 2_000,
                    tx_bps: 0
                },
            ]
        );
        assert_eq!(series.after(2).len(), 1);
        assert_eq!(series.totals(), (7_000, 700));
        // A recreated adapter starts its counters again.
        series.record(4, at(14), 100, 50);
        assert_eq!(series.latest().unwrap().rx_bps, 0);
    }

    #[test]
    fn keeps_the_last_300_samples() {
        let mut series = TrafficSeries::default();
        for second in 0..400 {
            series.record(second, at(second), second * 10, 0);
        }
        let samples = series.after(0);
        assert_eq!(samples.len(), TrafficSeries::CAPACITY);
        assert_eq!(samples[0].seq, 100);
        assert_eq!(samples[0].rx_bps, 10);
    }
}
