//! A profile's 5-minute traffic chart (TaskbarMetrics' mirrored NET chart):
//! receive above the axis, send below, on the profile's own scale. Geometry is in a 1000 × 1000
//! frame (500 is the axis) that the renderer stretches over the plot, so the chart needs no pixel
//! width.

use super::{Format, LocalTime};
use crate::ipc::TrafficSample;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChartView {
    /// `100 KB/s`, `2 MB/s` at the top and bottom edges: the only labels with the unit.
    pub scale_label: String,
    /// `50`, `1` at the quarter lines, in the edges' unit.
    pub half_label: String,
    /// Clock times under the five vertical grid lines; `now` closes the axis.
    pub x_labels: Vec<String>,
    /// Path data, or `None` while there is nothing to draw.
    pub series: Option<ChartSeries>,
    /// The big and the quiet line of an empty chart: `Handshaking…`, `waiting for …`.
    pub empty: Option<(String, String)>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChartSeries {
    pub rx_line: String,
    pub rx_fill: String,
    pub tx_line: String,
    pub tx_fill: String,
}

pub struct TrafficChart;

impl TrafficChart {
    /// The width of the window in samples: 5 minutes of 1 s.
    pub const WINDOW: usize = 300;
    const FRAME: f64 = 1000.0;
    /// The units of the axis labels, largest first, in KB/s.
    const UNITS: [(u64, &'static str); 3] = [(1_000_000, "GB/s"), (1000, "MB/s"), (1, "KB/s")];

    /// The scale in KB/s for a peak in bytes per second: the first 1-2-5 step from 10 KB/s up
    /// that holds the peak (TaskbarMetrics' NET scale).
    pub fn scale(peak_bps: u64) -> u64 {
        let peak = peak_bps.div_ceil(1000);
        let mut decade: u64 = 10;
        loop {
            if let Some(step) = [1, 2, 5]
                .into_iter()
                .map(|step| step * decade)
                .find(|step| *step >= peak)
            {
                return step;
            }
            decade = decade.saturating_mul(10);
        }
    }

    /// `kb` in the unit of `scale`, whole or with the one decimal a half of a 1-2-5 step needs.
    fn amount(kb: u64, scale: u64) -> (String, &'static str) {
        let (size, unit) = Self::UNITS
            .into_iter()
            .find(|(size, _)| scale >= *size)
            .unwrap_or((1, "KB/s"));
        let whole = Format::count(kb / size);
        let tenths = kb % size * 10 / size;
        if tenths == 0 {
            (whole, unit)
        } else {
            (format!("{whole}.{tenths}"), unit)
        }
    }

    fn labels(time: &LocalTime) -> Vec<String> {
        [0.1, 0.3, 0.5, 0.7, 0.9]
            .into_iter()
            .map(|f: f64| {
                let back = ((1.0 - f) * Self::WINDOW as f64) as u64;
                time.clock(time.now - std::time::Duration::from_secs(back))
            })
            .collect()
    }

    fn frame(scale: u64, time: &LocalTime) -> ChartView {
        ChartView {
            scale_label: {
                let (amount, unit) = Self::amount(scale, scale);
                format!("{amount} {unit}")
            },
            half_label: Self::amount(scale / 2, scale).0,
            x_labels: Self::labels(time),
            series: None,
            empty: None,
        }
    }

    /// An empty chart with its message.
    pub fn empty(title: &str, detail: &str, time: &LocalTime) -> ChartView {
        ChartView {
            empty: Some((title.into(), detail.into())),
            ..Self::frame(20, time)
        }
    }

    /// The newest `WINDOW` samples, right-aligned: a profile connected a minute ago fills the last
    /// fifth.
    pub fn of(samples: &[TrafficSample], time: &LocalTime) -> ChartView {
        let samples = &samples[samples.len().saturating_sub(Self::WINDOW)..];
        let peak = samples
            .iter()
            .map(|s| s.rx_bps.max(s.tx_bps))
            .max()
            .unwrap_or(0);
        let scale = Self::scale(peak);
        let mut view = Self::frame(scale, time);
        if samples.len() < 2 {
            view.empty = Some((
                "Waiting for traffic".into(),
                "the chart fills every second".into(),
            ));
            return view;
        }
        let first = Self::WINDOW - samples.len();
        let x = |i: usize| (first + i) as f64 * Self::FRAME / (Self::WINDOW - 1) as f64;
        let height = |bps: u64| (bps as f64 / (scale as f64 * 1000.0)).min(1.0) * Self::FRAME / 2.0;
        let mid = Self::FRAME / 2.0;
        let rx: Vec<(f64, f64)> = samples
            .iter()
            .enumerate()
            .map(|(i, s)| (x(i), mid - height(s.rx_bps)))
            .collect();
        let tx: Vec<(f64, f64)> = samples
            .iter()
            .enumerate()
            .map(|(i, s)| (x(i), mid + height(s.tx_bps)))
            .collect();
        view.series = Some(ChartSeries {
            rx_line: Self::line(&rx),
            rx_fill: Self::fill(&rx),
            tx_line: Self::line(&tx),
            tx_fill: Self::fill(&tx),
        });
        view
    }

    /// The anchors pin the 1000 × 1000 frame when the path is stretched over the plot.
    fn anchors() -> String {
        format!("M0,0 L0,0 M{0},{0} L{0},{0} ", Self::FRAME)
    }

    fn line(points: &[(f64, f64)]) -> String {
        let mut data = Self::anchors();
        for (index, (x, y)) in points.iter().enumerate() {
            data.push_str(if index == 0 { "M" } else { " L" });
            data.push_str(&format!("{x:.1},{y:.1}"));
        }
        data
    }

    fn fill(points: &[(f64, f64)]) -> String {
        let (Some(first), Some(last)) = (points.first(), points.last()) else {
            return String::new();
        };
        let mid = Self::FRAME / 2.0;
        format!(
            "{} L{:.1},{mid:.1} L{:.1},{mid:.1} Z",
            Self::line(points),
            last.0,
            first.0
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::presentation::samples::Samples;

    fn sample(seq: u64, rx: u64, tx: u64) -> TrafficSample {
        TrafficSample {
            seq,
            rx_bps: rx,
            tx_bps: tx,
        }
    }

    #[test]
    fn the_scale_grows_with_the_peak() {
        assert_eq!(TrafficChart::scale(0), 10);
        assert_eq!(TrafficChart::scale(38_000), 50);
        assert_eq!(TrafficChart::scale(100_000), 100);
        assert_eq!(TrafficChart::scale(100_001), 200);
        assert_eq!(TrafficChart::scale(12_000_000), 20_000);
        assert_eq!(TrafficChart::scale(3_000_000_000), 5_000_000);
        assert_eq!(TrafficChart::scale(u64::MAX), 20_000_000_000_000_000);
    }

    #[test]
    fn the_labels_take_the_shortest_unit() {
        let time = Samples::time();
        let labels = |scale: u64| {
            let view = TrafficChart::frame(scale, &time);
            (view.scale_label, view.half_label)
        };
        assert_eq!(labels(500), ("500 KB/s".into(), "250".into()));
        assert_eq!(labels(1000), ("1 MB/s".into(), "0.5".into()));
        assert_eq!(labels(5000), ("5 MB/s".into(), "2.5".into()));
        assert_eq!(labels(200_000), ("200 MB/s".into(), "100".into()));
        assert_eq!(labels(2_000_000), ("2 GB/s".into(), "1".into()));
    }

    #[test]
    fn samples_map_into_the_frame_mirrored() {
        let samples: Vec<TrafficSample> = (1..=300).map(|i| sample(i, 50_000, 10_000)).collect();
        let view = TrafficChart::of(&samples, &Samples::time());
        assert_eq!(view.scale_label, "50 KB/s");
        assert_eq!(view.half_label, "25");
        assert_eq!(view.x_labels, ["11:24", "11:25", "11:26", "11:27", "11:28"]);
        let series = view.series.unwrap();
        assert!(
            series
                .rx_line
                .starts_with("M0,0 L0,0 M1000,1000 L1000,1000 M0.0,0.0 L")
        );
        assert!(series.tx_line.ends_with("L1000.0,600.0"));
        assert!(series.rx_fill.ends_with("L1000.0,500.0 L0.0,500.0 Z"));
    }

    #[test]
    fn a_new_tunnel_fills_from_the_right_and_one_sample_waits() {
        let samples: Vec<TrafficSample> = (1..=60).map(|i| sample(i, 1000, 0)).collect();
        let series = TrafficChart::of(&samples, &Samples::time()).series.unwrap();
        assert!(
            series
                .rx_line
                .contains(&format!("M{:.1},", 240.0 * 1000.0 / 299.0))
        );
        assert!(
            TrafficChart::of(&samples[..1], &Samples::time())
                .empty
                .is_some()
        );
        let empty = TrafficChart::empty(
            "Handshaking…",
            "waiting for the first handshake",
            &Samples::time(),
        );
        assert_eq!(empty.empty.unwrap().0, "Handshaking…");
    }
}
