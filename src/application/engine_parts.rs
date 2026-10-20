use std::net::SocketAddrV4;
use std::sync::{Arc, Mutex};

use super::{
    ActiveProfiles, AlertService, DataFolder, DnsProxy, DnsService, GuardSource, ProfileSet,
    ProfileStates, SettingsFiles,
};
use crate::alerts::{AlertKind, AlertTexts};
use crate::clock::Clock;
use crate::config::{SettingProblem, Settings, StateFile};
use crate::dns::{
    DnsCacheFlusher, DnsExchangeFactory, NrptPolicyProbe, NrptSync, ResolverSet, SharedPolicy,
};
use crate::list::{ListSet, ProfileMatcher, SharedProfileMatcher};
use crate::logs::Log;
use crate::routes::{HostClaims, RouteTable};
use crate::stats::QueryStats;
use crate::tunnel::{EndpointResolver, Tunnel};

/// What the engine acts on, shared by the coordinator and the periodic jobs. Built once by
/// `Engine`; the parts that change are behind their own small locks.
pub struct EngineParts {
    pub folder: DataFolder,
    pub settings_files: SettingsFiles,
    pub tunnel: Arc<dyn Tunnel>,
    pub route_table: Arc<dyn RouteTable>,
    pub flusher: Arc<dyn DnsCacheFlusher>,
    pub bound: Arc<dyn DnsExchangeFactory>,
    pub nrpt: NrptSync,
    pub nrpt_policy: Arc<dyn NrptPolicyProbe>,
    pub guards: GuardSource,
    pub endpoints: EndpointResolver,
    pub active: Arc<ActiveProfiles>,
    pub resolvers: Arc<ResolverSet>,
    pub matcher: Arc<SharedProfileMatcher>,
    pub claims: Arc<Mutex<HostClaims>>,
    pub policy: Arc<SharedPolicy>,
    pub alerts: Arc<AlertService>,
    /// The proxy's query counters.
    pub stats: Arc<QueryStats>,
    pub states: Arc<ProfileStates>,
    pub dns: Arc<DnsService>,
    pub listen: SocketAddrV4,
    pub clock: Arc<dyn Clock>,
    pub log: Log,
}

/// What changes only under the coordinator's lock: the profiles, settings and lists as read from
/// disk, `connected=`, and the proxy while it runs.
pub struct Inventory {
    pub profiles: ProfileSet,
    pub settings: Result<Settings, Vec<SettingProblem>>,
    pub lists: ListSet,
    pub state: StateFile,
    pub proxy: Option<DnsProxy>,
}

impl EngineParts {
    /// Makes `connected` the matcher's profiles, then drops the learned routes whose names now
    /// go to another profile (re-homing); the Windows cache is flushed when any moved.
    pub fn rehome(&self, lists: &ListSet, connected: &[String]) {
        let matcher = ProfileMatcher::build(lists, connected);
        let now = self.clock.now();
        let mut moved = 0;
        let staying = self
            .active
            .all()
            .into_iter()
            .filter(|p| connected.contains(&p.name));
        for profile in staying {
            let mut routes = profile.routes.lock().unwrap();
            moved += routes.retain_names(
                |name| matcher.routes_to(name, &profile.name),
                &format!("no longer goes to {}", profile.name),
                now,
            );
            self.claims
                .lock()
                .unwrap()
                .retain(&profile.name, &routes.learned_ips());
        }
        self.matcher.replace(matcher);
        if moved > 0 {
            self.flush();
        }
    }

    /// Makes the NRPT rules the union of `connected`'s domains and flushes the Windows cache. A
    /// failed write raises `nrpt_write_failed`; the next good sync clears it.
    pub fn sync_nrpt(&self, lists: &ListSet, connected: &[String]) -> std::io::Result<()> {
        let domains = ProfileMatcher::build(lists, connected).domains();
        let result = self.nrpt.sync(&domains);
        match &result {
            Ok(()) => self.alerts.clear(AlertKind::NrptWriteFailed, None),
            Err(e) => self
                .alerts
                .raise(AlertTexts::nrpt_write_failed(&e.to_string())),
        }
        self.flush();
        result
    }

    /// Gives every connected profile its current never-routes and reconciles its routes.
    pub fn apply_guards(&self) {
        let now = self.clock.now();
        for profile in self.active.all() {
            let mut routes = profile.routes.lock().unwrap();
            routes.set_guard(self.guards.guard_for(&profile.name));
            routes.reconcile(now);
        }
    }

    pub fn flush(&self) {
        if let Err(e) = self.flusher.flush() {
            self.log
                .error(format!("flushing the Windows DNS cache failed: {e}"));
        }
    }

    /// Writes `connected=`.
    pub fn write_state(&self, state: &StateFile) -> std::io::Result<()> {
        let path = self.folder.state();
        self.folder
            .write(&path, state.render().as_bytes())
            .map_err(|e| std::io::Error::new(e.kind(), format!("writing {}: {e}", path.display())))
    }
}
