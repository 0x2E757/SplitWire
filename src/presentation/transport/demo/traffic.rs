use super::model::DemoChart;
use crate::ipc::{ProfileTraffic, TrafficSample};

/// Five minutes of made-up traffic per connected profile: a random walk,
/// receive around the profile's level, send at a third of it.
pub struct DemoTraffic;

impl DemoTraffic {
    pub const SAMPLES: u64 = ProfileTraffic::MAX_SAMPLES as u64;

    /// The generator: a Lehmer sequence seeded by the profile.
    fn walk(seed: u64, level: f64, count: usize) -> Vec<f64> {
        let mut state = (seed * 9973) % 2_147_483_647;
        if state == 0 {
            state = 1;
        }
        let mut next = move || {
            state = state * 16_807 % 2_147_483_647;
            state as f64 / 2_147_483_647.0
        };
        let mut value = level;
        (0..count)
            .map(|_| {
                value += (next() - 0.5) * level * 0.3 + (level - value) * 0.12;
                if next() < 0.025 {
                    value += level * 0.9;
                }
                value.max(0.0)
            })
            .collect()
    }

    /// The samples after `after_seq` up to `last_seq`, of a tunnel up since `from_seq`. The walk is
    /// the same for every call, so the chart moves on by one sample a second.
    pub fn samples(
        chart: DemoChart,
        after_seq: u64,
        last_seq: u64,
        from_seq: u64,
    ) -> Vec<TrafficSample> {
        let count = last_seq as usize;
        let rx = Self::walk(chart.seed, chart.level, count);
        let tx = Self::walk(chart.seed + 41, chart.level * 0.3, count);
        let mut samples: Vec<TrafficSample> = rx
            .into_iter()
            .zip(tx)
            .enumerate()
            .map(|(index, (rx, tx))| TrafficSample {
                seq: index as u64 + 1,
                rx_bps: (rx * 1000.0) as u64,
                tx_bps: (tx * 1000.0) as u64,
            })
            .filter(|s| s.seq > after_seq.max(from_seq))
            .collect();
        // A response holds at most the five minutes of the chart.
        let excess = samples.len().saturating_sub(ProfileTraffic::MAX_SAMPLES);
        samples.drain(..excess);
        samples
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn samples_are_stable_and_numbered() {
        let chart = DemoChart {
            level: 34.0,
            seed: 7,
        };
        let all = DemoTraffic::samples(chart, 0, 300, 0);
        assert_eq!(all.len(), 300);
        assert_eq!(all, DemoTraffic::samples(chart, 0, 300, 0));
        assert_eq!(all.last().unwrap().seq, 300);
        assert!(DemoTraffic::samples(chart, 300, 300, 0).is_empty());
        let next = DemoTraffic::samples(chart, 300, 302, 0);
        assert_eq!(next.iter().map(|s| s.seq).collect::<Vec<_>>(), [301, 302]);
        assert_eq!(DemoTraffic::samples(chart, 0, 310, 305).len(), 5);
        assert_eq!(DemoTraffic::samples(chart, 0, 900, 0)[0].seq, 601);
        let mean = all.iter().map(|s| s.rx_bps).sum::<u64>() / 300;
        assert!((10_000..80_000).contains(&mean), "{mean}");
    }
}
