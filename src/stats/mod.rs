//! Counters of the DNS proxy, the health of a profile's upstream DNS and a profile's traffic.

mod query_stats;
mod traffic_series;
mod upstream_health;

pub use query_stats::{QueryCounters, QueryStats};
pub use traffic_series::{TrafficSample, TrafficSeries};
pub use upstream_health::{UpstreamHealth, UpstreamHealthState};
