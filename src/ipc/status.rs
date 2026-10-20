use std::net::Ipv4Addr;
use std::time::SystemTime;

use super::domain_wire::DomainWire;
use super::settings_wire::SettingsWire;
use super::{Alert, ProfileSummary, Reader, WireError, Writer};
use crate::config::SettingProblem;
use crate::stats::QueryCounters;
use crate::tunnel::DriverVersion;

/// The tray icon's state, computed by the service so the window, the tray and the CLI agree.
/// `Service` (no pipe) is computed by the client and never sent.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TrayState {
    Service,
    Error,
    Warning,
    Connected,
    Off,
}

impl TrayState {
    pub const ALL: [Self; 5] = [
        Self::Service,
        Self::Error,
        Self::Warning,
        Self::Connected,
        Self::Off,
    ];
}

/// Revisions a client compares to know when to ask again (`GetAllLists` when `lists` changes).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Revisions {
    pub profiles: u64,
    pub lists: u64,
    pub settings: u64,
    pub alerts: u64,
}

impl Revisions {
    fn write(&self, w: &mut Writer) {
        for value in [self.profiles, self.lists, self.settings, self.alerts] {
            w.u64(value);
        }
    }

    fn read(r: &mut Reader) -> Result<Self, WireError> {
        Ok(Self {
            profiles: r.u64()?,
            lists: r.u64()?,
            settings: r.u64()?,
            alerts: r.u64()?,
        })
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Counters {
    pub queries: QueryCounters,
    pub cache_entries: u64,
    pub learned_routes: u64,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct NrptStatus {
    pub rules: u64,
    pub note: Option<String>,
}

/// The answer to `Status`: everything the window polls every second.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StatusReport {
    pub version: String,
    /// `None` until the driver is loaded.
    pub wgnt_version: Option<DriverVersion>,
    /// When the service started.
    pub since: SystemTime,
    pub tray_state: TrayState,
    pub revs: Revisions,
    pub counters: Counters,
    pub nrpt: NrptStatus,
    /// The fallback DNS servers in use.
    pub fallback: Vec<Ipv4Addr>,
    /// The problems of `splitwire.conf`.
    pub problems: Vec<SettingProblem>,
    /// Active and dismissed.
    pub alerts: Vec<Alert>,
    /// In the user's order.
    pub profiles: Vec<ProfileSummary>,
}

impl StatusReport {
    pub fn write(&self, w: &mut Writer) {
        w.str(&self.version);
        w.option(self.wgnt_version.as_ref(), DomainWire::driver_version);
        w.time(self.since);
        w.choice(&self.tray_state, &TrayState::ALL);
        self.revs.write(w);
        DomainWire::counters(&self.counters.queries, w);
        w.u64(self.counters.cache_entries);
        w.u64(self.counters.learned_routes);
        w.u64(self.nrpt.rules);
        w.opt_str(self.nrpt.note.as_ref());
        w.list(&self.fallback, |address, w| w.ipv4(*address));
        w.list(&self.problems, SettingsWire::problem);
        w.list(&self.alerts, Alert::write);
        w.list(&self.profiles, ProfileSummary::write);
    }

    pub fn read(r: &mut Reader) -> Result<Self, WireError> {
        Ok(Self {
            version: r.string()?,
            wgnt_version: r.option(DomainWire::read_driver_version)?,
            since: r.time()?,
            tray_state: r.choice(&TrayState::ALL)?,
            revs: Revisions::read(r)?,
            counters: Counters {
                queries: DomainWire::read_counters(r)?,
                cache_entries: r.u64()?,
                learned_routes: r.u64()?,
            },
            nrpt: NrptStatus {
                rules: r.u64()?,
                note: r.opt_string()?,
            },
            fallback: r.list(Reader::ipv4)?,
            problems: r.list(SettingsWire::read_problem)?,
            alerts: r.list(Alert::read)?,
            profiles: r.list(ProfileSummary::read)?,
        })
    }
}
