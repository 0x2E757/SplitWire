use std::sync::Arc;

use super::{ActiveProfile, EngineParts};
use crate::alerts::{AlertKind, AlertTexts};
use crate::profile::ProfileState;
use crate::tunnel::{AttemptKind, ConnectAttempt, ConnectStep, HandshakeHealth, StepResult};

/// Every 10 s, and at once on resume: each connected profile's adapter is made to
/// match its spec again (recreated, restored, its endpoint followed), its routes and bound DNS
/// follow a new link, and its health, handshake and upstream alerts are updated.
pub struct TunnelWatch<'a> {
    parts: &'a EngineParts,
}

impl<'a> TunnelWatch<'a> {
    pub fn new(parts: &'a EngineParts) -> Self {
        Self { parts }
    }

    pub fn tick(&self) {
        for profile in self.parts.active.all() {
            self.recover(&profile);
            self.judge_upstream(&profile);
        }
    }

    fn recover(&self, profile: &Arc<ActiveProfile>) {
        let name = profile.name.as_str();
        let link = match self.parts.tunnel.refresh(&profile.spec) {
            Ok(link) => link,
            Err(e) => {
                let error = e.to_string();
                self.parts
                    .alerts
                    .raise(AlertTexts::recover_failed(name, &error));
                let mut attempt = ConnectAttempt::new(self.parts.clock.now(), AttemptKind::Recover);
                attempt.set(ConnectStep::Adapter, StepResult::Failed, error);
                self.parts.states.attempt(name, attempt);
                self.parts.states.set(name, ProfileState::Failed);
                return;
            }
        };
        let old = *profile.link.lock().unwrap();
        if link != old {
            *profile.link.lock().unwrap() = link;
            let now = self.parts.clock.now();
            if link.luid != old.luid {
                profile.routes.lock().unwrap().rebind(link.luid, now);
            }
            if let Some(resolver) = self.parts.resolvers.get(name) {
                resolver.rebind(self.parts.bound.bound(profile.spec.address, link.if_index));
            }
            self.parts.log.with_profile(name).info(format!(
                "the adapter is now LUID {:#x}, index {}: routes and DNS follow",
                link.luid, link.if_index
            ));
        }
        self.parts
            .alerts
            .clear(AlertKind::RecoverFailed, Some(name));
        self.judge_handshake(profile);
    }

    fn judge_handshake(&self, profile: &ActiveProfile) {
        let name = profile.name.as_str();
        let Ok(health) = self.parts.tunnel.health(name) else {
            return;
        };
        // The upstream's warm-up ends after the first handshake and comes back while the last
        // is too old. Every 10 s is soon enough: a late report only lengthens the patience.
        if let (Some(at), Some(resolver)) = (health.last_handshake, self.parts.resolvers.get(name))
        {
            resolver.handshake(at);
        }
        let judged = profile
            .handshakes
            .lock()
            .unwrap()
            .judge(self.parts.clock.now(), &health);
        *profile.health.lock().unwrap() = judged;
        self.parts
            .states
            .set(name, ProfileState::Connected { health: judged });
        if judged == HandshakeHealth::Stale {
            self.parts.alerts.raise(AlertTexts::handshake_stale(name));
        } else if health.last_handshake.is_some() {
            self.parts
                .alerts
                .clear(AlertKind::HandshakeStale, Some(name));
        }
    }

    fn judge_upstream(&self, profile: &ActiveProfile) {
        let name = profile.name.as_str();
        let Some(resolver) = self.parts.resolvers.get(name) else {
            return;
        };
        if resolver.unreachable() {
            let servers: Vec<String> = resolver.servers().iter().map(ToString::to_string).collect();
            let error = resolver.health().last_error.unwrap_or_default();
            self.parts.alerts.raise(AlertTexts::upstream_unreachable(
                name,
                &servers.join(", "),
                &error,
            ));
        } else {
            self.parts
                .alerts
                .clear(AlertKind::UpstreamUnreachable, Some(name));
        }
    }
}
