use super::{Reader, WireError, Writer};

/// One second of a tunnel's traffic.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TrafficSample {
    pub seq: u64,
    pub rx_bps: u64,
    pub tx_bps: u64,
}

/// A connected profile's samples after the `after_seq` the client sent.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProfileTraffic {
    pub name: String,
    pub samples: Vec<TrafficSample>,
}

impl ProfileTraffic {
    /// At most 300 samples: the 5-minute chart.
    pub const MAX_SAMPLES: usize = 300;

    pub fn write(&self, w: &mut Writer) {
        w.str(&self.name);
        w.list(&self.samples, |sample, w| {
            w.u64(sample.seq);
            w.u64(sample.rx_bps);
            w.u64(sample.tx_bps);
        });
    }

    pub fn read(r: &mut Reader) -> Result<Self, WireError> {
        Ok(Self {
            name: r.string()?,
            samples: r.list(|r| {
                Ok(TrafficSample {
                    seq: r.u64()?,
                    rx_bps: r.u64()?,
                    tx_bps: r.u64()?,
                })
            })?,
        })
    }
}
