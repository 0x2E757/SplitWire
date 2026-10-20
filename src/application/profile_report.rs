use std::net::SocketAddrV4;
use std::time::{Duration, UNIX_EPOCH};

use super::wire_attempts::WireAttempts;
use super::{Coordinator, ListsViewBuilder, RouteGroups, StatusReporter, StoredProfile};
use crate::ipc::{ProfileDetail, ProfileRoutes};
use crate::profile::Base64;
use crate::tunnel::TunnelSpec;

/// The profile page (`ProfileStatus`) and the Routes page (`Routes`): one profile's facts, public
/// keys, upstream, list and grouped routes, or every profile's routes. Never the coordinator's
/// lock, never a key.
pub struct ProfileReport<'a> {
    coordinator: &'a Coordinator,
    reporter: &'a StatusReporter,
}

impl<'a> ProfileReport<'a> {
    pub fn new(coordinator: &'a Coordinator, reporter: &'a StatusReporter) -> Self {
        Self {
            coordinator,
            reporter,
        }
    }

    /// `None` for an unknown profile; `Err` when its blob cannot be read.
    pub fn detail(&self, name: &str) -> Option<Result<ProfileDetail, String>> {
        let catalog = self.coordinator.catalog();
        let stored = catalog
            .iter()
            .find(|p| p.name.as_str().eq_ignore_ascii_case(name))?;
        Some(self.detail_of(stored, &catalog))
    }

    /// Every profile's grouped routes, in the user's order.
    pub fn routes(&self) -> Vec<ProfileRoutes> {
        let texts = self.coordinator.list_texts().unwrap_or_default();
        let set = ListsViewBuilder::set_of(&texts);
        let groups = RouteGroups::new(&self.coordinator.parts().active);
        self.coordinator
            .catalog()
            .iter()
            .map(|stored| ProfileRoutes {
                name: stored.name.to_string(),
                groups: groups.of(stored.name.as_str(), set.get(stored.name.as_str())),
            })
            .collect()
    }

    fn detail_of(
        &self,
        stored: &StoredProfile,
        catalog: &[StoredProfile],
    ) -> Result<ProfileDetail, String> {
        let parts = self.coordinator.parts();
        let name = stored.name.as_str();
        let facts = stored.facts.as_ref().map_err(Clone::clone)?;
        let settings = parts.settings_files.profile(&stored.name);
        let (settings, settings_problems) = match settings {
            Ok(Ok(settings)) => (Some(settings), Vec::new()),
            Ok(Err(problems)) => (None, problems),
            Err(_) => (None, Vec::new()),
        };
        let mtu_override = settings.as_ref().and_then(|s| s.mtu);
        let active = parts.active.get(name);
        let texts = self.coordinator.list_texts().unwrap_or_default();
        let set = ListsViewBuilder::set_of(&texts);
        let text = texts
            .iter()
            .find(|(p, _)| p == name)
            .map_or("", |(_, t)| t.as_str());
        let upstream: Vec<SocketAddrV4> = match (&active, &settings) {
            (Some(active), _) => active.upstream.clone(),
            (None, Some(settings)) if !settings.upstream.is_empty() => settings.upstream.clone(),
            _ => facts
                .dns
                .iter()
                .map(|ip| SocketAddrV4::new(*ip, 53))
                .collect(),
        };
        let mut endpoint_addresses: Vec<_> = facts.endpoint.literal().into_iter().collect();
        if let Some(active) = &active {
            endpoint_addresses.push(*active.spec.endpoint.ip());
            endpoint_addresses.dedup();
        }
        let interface_public_key = active
            .as_ref()
            .and_then(|_| parts.tunnel.health(name).ok())
            .map(|h| h.interface_public_key)
            .filter(|key| key.iter().any(|b| *b != 0))
            .map(|key| Base64::encode(&key));
        Ok(ProfileDetail {
            summary: self.reporter.summary(stored),
            adapter: format!("{}{name}", TunnelSpec::ADAPTER_PREFIX),
            address: facts.address,
            mtu: mtu_override.unwrap_or_else(|| facts.mtu_or_default()),
            mtu_override,
            endpoint: facts.endpoint.clone(),
            endpoint_addresses,
            interface_public_key,
            peer_public_key: Base64::encode(&facts.peer_public_key),
            upstream,
            upstream_health: parts
                .resolvers
                .get(name)
                .map(|r| r.health())
                .unwrap_or_default(),
            keepalive: facts.keepalive,
            imported: UNIX_EPOCH + Duration::from_secs(facts.imported),
            list: ListsViewBuilder::new(&parts.active, catalog).lines(name, text, &set),
            routes: RouteGroups::new(&parts.active).of(name, set.get(name)),
            settings_problems,
            last_attempt: parts
                .states
                .get(name)
                .last_attempt
                .as_ref()
                .map(WireAttempts::attempt),
        })
    }
}
