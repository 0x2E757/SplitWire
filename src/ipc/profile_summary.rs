use std::net::SocketAddrV4;
use std::time::SystemTime;

use super::domain_wire::DomainWire;
use super::profile_wire::ProfileWire;
use super::{AttemptSummary, ConnectStep, Reader, WireError, Writer};
use crate::profile::Pin;
use crate::tunnel::HandshakeHealth;

/// A profile's state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProfileState {
    Off,
    Connecting(ConnectStep),
    Connected(HandshakeHealth),
    Disconnecting,
    /// The last Connect or Recover failed.
    Failed,
    /// Its `profile.conf` is invalid.
    Blocked,
    /// `splitwire.conf` is invalid: every profile stays off.
    HeldOff,
}

impl ProfileState {
    fn write(&self, w: &mut Writer) {
        match self {
            Self::Off => w.u64(0),
            Self::Connecting(step) => {
                w.u64(1);
                step.write(w);
            }
            Self::Connected(health) => {
                w.u64(2);
                w.choice(health, &DomainWire::HEALTH);
            }
            Self::Disconnecting => w.u64(3),
            Self::Failed => w.u64(4),
            Self::Blocked => w.u64(5),
            Self::HeldOff => w.u64(6),
        }
    }

    fn read(r: &mut Reader) -> Result<Self, WireError> {
        Ok(match r.u64()? {
            0 => Self::Off,
            1 => Self::Connecting(ConnectStep::read(r)?),
            2 => Self::Connected(r.choice(&DomainWire::HEALTH)?),
            3 => Self::Disconnecting,
            4 => Self::Failed,
            5 => Self::Blocked,
            6 => Self::HeldOff,
            _ => return Err(WireError::Invalid("an unknown profile state")),
        })
    }
}

/// One profile in `Status`: what the nav, the cards, the tray and `list` show.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProfileSummary {
    pub name: String,
    pub colour: Pin,
    pub state: ProfileState,
    /// When the current state began.
    pub since: Option<SystemTime>,
    pub uptime_secs: Option<u64>,
    pub last_connected: Option<SystemTime>,
    pub handshake_age_secs: Option<u64>,
    /// Bytes per second over the last sample.
    pub rx_rate: u64,
    pub tx_rate: u64,
    /// Bytes since the tunnel came up.
    pub rx_total: u64,
    pub tx_total: u64,
    pub learned: u64,
    pub dns_rtt_ms: Option<u64>,
    /// The first DNS server of the tunnel while connected.
    pub upstream: Option<SocketAddrV4>,
    /// The peer's persistent keepalive in seconds; 0 is off.
    pub keepalive: u16,
    pub last_attempt: Option<AttemptSummary>,
    /// Why it cannot connect, such as the bad line of its `profile.conf`.
    pub problem: Option<String>,
}

impl ProfileSummary {
    pub fn write(&self, w: &mut Writer) {
        w.str(&self.name);
        ProfileWire::pin(&self.colour, w);
        self.state.write(w);
        w.option(self.since.as_ref(), |at, w| w.time(*at));
        w.option(self.uptime_secs.as_ref(), |secs, w| w.u64(*secs));
        w.option(self.last_connected.as_ref(), |at, w| w.time(*at));
        w.option(self.handshake_age_secs.as_ref(), |secs, w| w.u64(*secs));
        for value in [self.rx_rate, self.tx_rate, self.rx_total, self.tx_total] {
            w.u64(value);
        }
        w.u64(self.learned);
        w.option(self.dns_rtt_ms.as_ref(), |ms, w| w.u64(*ms));
        w.option(self.upstream.as_ref(), |server, w| w.socket(*server));
        w.u16(self.keepalive);
        w.option(self.last_attempt.as_ref(), AttemptSummary::write);
        w.opt_str(self.problem.as_ref());
    }

    pub fn read(r: &mut Reader) -> Result<Self, WireError> {
        Ok(Self {
            name: r.string()?,
            colour: ProfileWire::read_pin(r)?,
            state: ProfileState::read(r)?,
            since: r.option(Reader::time)?,
            uptime_secs: r.option(Reader::u64)?,
            last_connected: r.option(Reader::time)?,
            handshake_age_secs: r.option(Reader::u64)?,
            rx_rate: r.u64()?,
            tx_rate: r.u64()?,
            rx_total: r.u64()?,
            tx_total: r.u64()?,
            learned: r.u64()?,
            dns_rtt_ms: r.option(Reader::u64)?,
            upstream: r.option(Reader::socket)?,
            keepalive: r.u16()?,
            last_attempt: r.option(AttemptSummary::read)?,
            problem: r.opt_string()?,
        })
    }
}
