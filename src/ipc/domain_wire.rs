use std::time::Duration;

use super::{Reader, WireError, Writer};
use crate::list::TextProblem;
use crate::logs::Level;
use crate::stats::{QueryCounters, UpstreamHealthState};
use crate::tunnel::{DriverVersion, HandshakeHealth};

/// The wire form of the list, log, stats and tunnel types the messages reuse.
pub struct DomainWire;

impl DomainWire {
    pub const LEVELS: [Level; 3] = [Level::Info, Level::Warn, Level::Error];
    pub const HEALTH: [HandshakeHealth; 3] = [
        HandshakeHealth::Ok,
        HandshakeHealth::Handshaking,
        HandshakeHealth::Stale,
    ];

    pub fn text_problem(problem: &TextProblem, w: &mut Writer) {
        w.option(problem.text_line.as_ref(), |line, w| w.usize(*line));
        w.opt_str(problem.profile.as_ref());
        w.option(problem.file_line.as_ref(), |line, w| w.usize(*line));
        w.str(&problem.text);
        w.str(&problem.reason);
        w.option(problem.other.as_ref(), |(profile, line), w| {
            w.str(profile);
            w.usize(*line);
        });
        w.bool(problem.blocking);
    }

    pub fn read_text_problem(r: &mut Reader) -> Result<TextProblem, WireError> {
        Ok(TextProblem {
            text_line: r.option(Reader::usize)?,
            profile: r.opt_string()?,
            file_line: r.option(Reader::usize)?,
            text: r.string()?,
            reason: r.string()?,
            other: r.option(|r| Ok((r.string()?, r.usize()?)))?,
            blocking: r.bool()?,
        })
    }

    pub fn counters(counters: &QueryCounters, w: &mut Writer) {
        for value in [
            counters.total,
            counters.listed,
            counters.bypassed,
            counters.other,
            counters.failed,
            counters.cache_hits,
        ] {
            w.u64(value);
        }
    }

    pub fn read_counters(r: &mut Reader) -> Result<QueryCounters, WireError> {
        Ok(QueryCounters {
            total: r.u64()?,
            listed: r.u64()?,
            bypassed: r.u64()?,
            other: r.u64()?,
            failed: r.u64()?,
            cache_hits: r.u64()?,
        })
    }

    pub fn upstream_health(health: &UpstreamHealthState, w: &mut Writer) {
        w.option(health.last_success.as_ref(), |at, w| w.time(*at));
        w.option(health.last_failure.as_ref(), |at, w| w.time(*at));
        w.opt_str(health.last_error.as_ref());
        w.u64(u64::from(health.consecutive_failures));
        w.option(health.rtt.as_ref(), |rtt, w| {
            w.u64(u64::try_from(rtt.as_micros()).unwrap_or(u64::MAX))
        });
    }

    pub fn read_upstream_health(r: &mut Reader) -> Result<UpstreamHealthState, WireError> {
        Ok(UpstreamHealthState {
            last_success: r.option(Reader::time)?,
            last_failure: r.option(Reader::time)?,
            last_error: r.opt_string()?,
            consecutive_failures: u32::try_from(r.u64()?)
                .map_err(|_| WireError::Invalid("consecutive_failures"))?,
            rtt: r.option(|r| r.u64().map(Duration::from_micros))?,
        })
    }

    pub fn driver_version(version: &DriverVersion, w: &mut Writer) {
        w.u16(version.major);
        w.u16(version.minor);
    }

    pub fn read_driver_version(r: &mut Reader) -> Result<DriverVersion, WireError> {
        Ok(DriverVersion {
            major: r.u16()?,
            minor: r.u16()?,
        })
    }
}
