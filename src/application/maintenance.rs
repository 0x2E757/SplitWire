use std::net::Ipv4Addr;

use super::{EngineParts, Inventory};
use crate::alerts::{AlertKind, AlertTexts};

/// Every 30 s: the guard refreshed (fallback DNS, on-link networks) and every profile's routes
/// reconciled with it; idle learned routes expired with their claims; the alerts that follow
/// the routes and claims; the Group Policy probe; the proxy watchdog.
pub struct Maintenance<'a> {
    parts: &'a EngineParts,
    inventory: &'a mut Inventory,
}

impl<'a> Maintenance<'a> {
    pub fn new(parts: &'a EngineParts, inventory: &'a mut Inventory) -> Self {
        Self { parts, inventory }
    }

    pub fn tick(&mut self) {
        self.parts.guards.refresh();
        self.parts.apply_guards();
        self.expire();
        self.probe_group_policy();
        self.watchdog();
    }

    fn expire(&self) {
        let now = self.parts.clock.now();
        for profile in self.parts.active.all() {
            let name = profile.name.as_str();
            let mut routes = profile.routes.lock().unwrap();
            routes.expire(now);
            let mut claims = self.parts.claims.lock().unwrap();
            claims.retain(name, &routes.learned_ips());
            let snapshot = routes.snapshot();
            for refused in &snapshot.refused {
                self.parts.alerts.raise(AlertTexts::route_refused(
                    name,
                    &refused.net.to_string(),
                    &refused.origin,
                    &refused.reason,
                ));
            }
            let still_refused = |subject: &str| {
                snapshot
                    .refused
                    .iter()
                    .any(|r| r.net.to_string() == subject)
            };
            self.parts
                .alerts
                .clear_kind(AlertKind::RouteRefused, Some(name), still_refused);
            // `address_held` ends when the holder released the address.
            let still_held = |subject: &str| {
                subject
                    .parse::<Ipv4Addr>()
                    .ok()
                    .and_then(|ip| claims.holder(ip))
                    .is_some_and(|holder| holder != name)
            };
            self.parts
                .alerts
                .clear_kind(AlertKind::AddressHeld, Some(name), still_held);
            profile
                .held
                .lock()
                .unwrap()
                .retain(|ip, _| claims.holder(*ip).is_some_and(|holder| holder != name));
        }
    }

    fn probe_group_policy(&self) {
        match self.parts.nrpt_policy.policy_rules_exist() {
            Ok(true) => self.parts.alerts.raise(AlertTexts::gp_nrpt()),
            Ok(false) => self.parts.alerts.clear(AlertKind::GpNrpt, None),
            Err(e) => self
                .parts
                .log
                .warn(format!("reading the Group Policy NRPT key failed: {e}")),
        }
    }

    /// NRPT rules exist only while the proxy serves: a proxy that ended on its own takes them
    /// with it (fail closed: listed names then resolve normally).
    fn watchdog(&mut self) {
        let dead = self.inventory.proxy.as_ref().is_some_and(|p| !p.alive());
        if !dead {
            return;
        }
        self.parts
            .log
            .error("the DNS proxy stopped unexpectedly: removing the NRPT rules");
        if let Some(proxy) = self.inventory.proxy.take() {
            proxy.stop();
        }
        if let Err(e) = self.parts.nrpt.remove_all() {
            self.parts
                .alerts
                .raise(AlertTexts::nrpt_write_failed(&e.to_string()));
        }
        self.parts.flush();
    }
}
