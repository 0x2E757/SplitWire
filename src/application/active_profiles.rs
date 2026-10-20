use std::collections::BTreeMap;
use std::net::{Ipv4Addr, SocketAddrV4};
use std::sync::{Arc, Mutex, RwLock};
use std::time::SystemTime;

use crate::config::ProfileSettings;
use crate::net::Ipv4Net;
use crate::routes::RouteManager;
use crate::stats::TrafficSeries;
use crate::tunnel::{HandshakeHealth, HandshakePolicy, TunnelLink, TunnelSpec};

/// A connected profile: its adapter, its routes on that adapter, and what the periodic jobs
/// track. It holds the decrypted keys (`spec`); dropping it zeroes them.
pub struct ActiveProfile {
    pub name: String,
    pub spec: TunnelSpec,
    /// The `profile.conf` it connected with; a change to `upstream` or `mtu` reconnects.
    pub settings: ProfileSettings,
    pub subnet: Ipv4Net,
    pub upstream: Vec<SocketAddrV4>,
    pub link: Mutex<TunnelLink>,
    pub routes: Mutex<RouteManager>,
    pub handshakes: Mutex<HandshakePolicy>,
    pub health: Mutex<HandshakeHealth>,
    pub traffic: Mutex<TrafficSeries>,
    /// Answered addresses another profile holds: the address, the name it answered, the holder.
    pub held: Mutex<BTreeMap<Ipv4Addr, (String, String)>>,
    pub since: SystemTime,
}

/// The connected profiles by name. The DNS path reads it to route an answer on the right adapter.
#[derive(Default)]
pub struct ActiveProfiles {
    profiles: RwLock<BTreeMap<String, Arc<ActiveProfile>>>,
}

impl ActiveProfiles {
    pub fn insert(&self, profile: ActiveProfile) -> Arc<ActiveProfile> {
        let profile = Arc::new(profile);
        self.profiles
            .write()
            .unwrap()
            .insert(profile.name.clone(), profile.clone());
        profile
    }

    pub fn remove(&self, name: &str) -> Option<Arc<ActiveProfile>> {
        self.profiles.write().unwrap().remove(name)
    }

    pub fn get(&self, name: &str) -> Option<Arc<ActiveProfile>> {
        self.profiles.read().unwrap().get(name).cloned()
    }

    pub fn all(&self) -> Vec<Arc<ActiveProfile>> {
        self.profiles.read().unwrap().values().cloned().collect()
    }

    pub fn names(&self) -> Vec<String> {
        self.profiles.read().unwrap().keys().cloned().collect()
    }

    /// The connected names except `name`.
    pub fn names_without(&self, name: &str) -> Vec<String> {
        self.names().into_iter().filter(|n| n != name).collect()
    }

    pub fn contains(&self, name: &str) -> bool {
        self.profiles.read().unwrap().contains_key(name)
    }

    pub fn is_empty(&self) -> bool {
        self.profiles.read().unwrap().is_empty()
    }
}
