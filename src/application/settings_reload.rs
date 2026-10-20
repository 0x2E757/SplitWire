use std::time::Duration;

use super::{EngineParts, Inventory};
use crate::alerts::{AlertKind, AlertTexts};
use crate::config::Settings;
use crate::dns::DnsPolicy;
use crate::list::DomainName;
use crate::profile::{EndpointHost, ProfileName, ProfileState};

/// Reads `splitwire.conf` and every `profile.conf` again and applies them: the live
/// keys at once, `held_off` and `blocked` with their alerts, and the connected profiles whose
/// `upstream` or `mtu` changed, which the caller reconnects.
pub struct SettingsReload<'a> {
    parts: &'a EngineParts,
    inventory: &'a mut Inventory,
}

impl<'a> SettingsReload<'a> {
    pub fn new(parts: &'a EngineParts, inventory: &'a mut Inventory) -> Self {
        Self { parts, inventory }
    }

    /// Returns the connected profiles to reconnect.
    pub fn reload(&mut self) -> Vec<ProfileName> {
        self.reload_global();
        let mut reconnect = Vec::new();
        for stored in self.inventory.profiles.profiles() {
            let name = stored.name;
            match self.parts.settings_files.profile(&name) {
                Ok(Ok(settings)) => {
                    self.parts
                        .alerts
                        .clear(AlertKind::ProfileSettingsInvalid, Some(name.as_str()));
                    let changed = self.parts.active.get(name.as_str()).is_some_and(|active| {
                        active.settings.upstream != settings.upstream
                            || active.settings.mtu != settings.mtu
                    });
                    if changed {
                        reconnect.push(name.clone());
                    }
                    self.idle_state(&name, false);
                }
                Ok(Err(problems)) => {
                    self.parts
                        .alerts
                        .raise(AlertTexts::profile_settings_invalid(
                            name.as_str(),
                            &problems,
                        ));
                    self.idle_state(&name, true);
                }
                Err(e) => self
                    .parts
                    .log
                    .with_profile(name.as_str())
                    .error(format!("reading profile.conf failed: {e}")),
            }
        }
        reconnect
    }

    /// The state of a profile that is not connected: `held_off`, `blocked`, or `off` (a failed
    /// attempt stays shown until the next one).
    fn idle_state(&self, name: &ProfileName, blocked: bool) {
        let states = &self.parts.states;
        let current = states.state(name.as_str());
        if self.parts.active.contains(name.as_str())
            || matches!(current, ProfileState::Connecting { .. })
        {
            return;
        }
        let state = if self.inventory.settings.is_err() {
            ProfileState::HeldOff
        } else if blocked {
            ProfileState::Blocked
        } else if current == ProfileState::Failed {
            ProfileState::Failed
        } else {
            ProfileState::Off
        };
        states.set(name.as_str(), state);
    }

    /// An invalid file keeps the live values as they are and every profile off.
    fn reload_global(&mut self) {
        match self.parts.settings_files.global() {
            Ok(Ok(settings)) => {
                self.parts.alerts.clear(AlertKind::SettingsInvalid, None);
                if let Ok(previous) = &self.inventory.settings
                    && previous.listen != settings.listen
                {
                    self.parts.log.info(format!(
                        "listen {} applies at the next start",
                        settings.listen
                    ));
                }
                self.apply_live(&settings);
                self.inventory.settings = Ok(settings);
            }
            Ok(Err(problems)) => {
                self.parts
                    .alerts
                    .raise(AlertTexts::settings_invalid(&problems));
                self.inventory.settings = Err(problems);
            }
            Err(e) => self
                .parts
                .log
                .error(format!("reading splitwire.conf failed: {e}")),
        }
    }

    /// `fallback`, `bypass`, `max_ttl` and `idle_expiry` apply without a reconnect.
    fn apply_live(&self, settings: &Settings) {
        let endpoint_hosts: Vec<DomainName> = self
            .inventory
            .profiles
            .profiles()
            .into_iter()
            .filter_map(|p| match p.facts.ok()?.endpoint.host {
                EndpointHost::Name(name) => Some(name),
                EndpointHost::Address(_) => None,
            })
            .collect();
        let max_ttl = settings.max_ttl.as_secs() as u32;
        self.parts
            .policy
            .replace(DnsPolicy::new(&settings.bypass, &endpoint_hosts, max_ttl));
        self.parts
            .guards
            .set_configured_fallback(settings.fallback.clone());
        self.parts.guards.refresh();
        self.set_idle_expiry(settings.idle_expiry);
    }

    fn set_idle_expiry(&self, idle_expiry: Duration) {
        for profile in self.parts.active.all() {
            profile.routes.lock().unwrap().set_idle_expiry(idle_expiry);
        }
    }
}
