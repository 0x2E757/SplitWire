use std::net::{Ipv4Addr, SocketAddrV4};
use std::time::SystemTime;

use super::domain_wire::DomainWire;
use super::profile_wire::ProfileWire;
use super::settings_wire::SettingsWire;
use super::{Attempt, ListLine, ProfileSummary, Reader, RouteGroup, WireError, Writer};
use crate::config::SettingProblem;
use crate::profile::{Endpoint, TunnelAddress};
use crate::stats::UpstreamHealthState;

/// The answer to `ProfileStatus{name}`: the profile page. Public keys only, as base64.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProfileDetail {
    pub summary: ProfileSummary,
    pub adapter: String,
    pub address: TunnelAddress,
    /// The MTU in effect, and the `profile.conf` override if any.
    pub mtu: u16,
    pub mtu_override: Option<u16>,
    pub endpoint: Endpoint,
    /// The addresses the endpoint resolved to in this run.
    pub endpoint_addresses: Vec<Ipv4Addr>,
    /// Read from the adapter while connected.
    pub interface_public_key: Option<String>,
    pub peer_public_key: String,
    pub upstream: Vec<SocketAddrV4>,
    pub upstream_health: UpstreamHealthState,
    pub keepalive: u16,
    pub imported: SystemTime,
    /// Its list file, with per-line notes and route counts.
    pub list: Vec<ListLine>,
    /// Its routes grouped by list line.
    pub routes: Vec<RouteGroup>,
    pub settings_problems: Vec<SettingProblem>,
    pub last_attempt: Option<Attempt>,
}

impl ProfileDetail {
    pub fn write(&self, w: &mut Writer) {
        self.summary.write(w);
        w.str(&self.adapter);
        ProfileWire::address(&self.address, w);
        w.u16(self.mtu);
        w.option(self.mtu_override.as_ref(), |mtu, w| w.u16(*mtu));
        ProfileWire::endpoint(&self.endpoint, w);
        w.list(&self.endpoint_addresses, |address, w| w.ipv4(*address));
        w.opt_str(self.interface_public_key.as_ref());
        w.str(&self.peer_public_key);
        w.list(&self.upstream, |server, w| w.socket(*server));
        DomainWire::upstream_health(&self.upstream_health, w);
        w.u16(self.keepalive);
        w.time(self.imported);
        w.list(&self.list, ListLine::write);
        w.list(&self.routes, RouteGroup::write);
        w.list(&self.settings_problems, SettingsWire::problem);
        w.option(self.last_attempt.as_ref(), Attempt::write);
    }

    pub fn read(r: &mut Reader) -> Result<Self, WireError> {
        Ok(Self {
            summary: ProfileSummary::read(r)?,
            adapter: r.string()?,
            address: ProfileWire::read_address(r)?,
            mtu: r.u16()?,
            mtu_override: r.option(Reader::u16)?,
            endpoint: ProfileWire::read_endpoint(r)?,
            endpoint_addresses: r.list(Reader::ipv4)?,
            interface_public_key: r.opt_string()?,
            peer_public_key: r.string()?,
            upstream: r.list(Reader::socket)?,
            upstream_health: DomainWire::read_upstream_health(r)?,
            keepalive: r.u16()?,
            imported: r.time()?,
            list: r.list(ListLine::read)?,
            routes: r.list(RouteGroup::read)?,
            settings_problems: r.list(SettingsWire::read_problem)?,
            last_attempt: r.option(Attempt::read)?,
        })
    }
}
