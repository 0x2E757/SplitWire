use super::{EngineParts, Inventory};

/// Takes a profile's pieces away, each on its own and best effort: Disconnect runs all of them,
/// and a failed Connect undoes the steps it had done with the same pieces.
pub struct Teardown<'a> {
    parts: &'a EngineParts,
    inventory: &'a mut Inventory,
}

impl<'a> Teardown<'a> {
    pub fn new(parts: &'a EngineParts, inventory: &'a mut Inventory) -> Self {
        Self { parts, inventory }
    }

    /// Disconnect: every piece runs even when one fails; errors are logged. A
    /// service stop keeps `connected=`.
    pub fn disconnect(&mut self, name: &str, keep_state: bool) {
        self.unmatch(name);
        self.unnrpt(name);
        self.unresolve(name);
        self.unroute(name);
        self.untunnel(name);
        self.unguard(name);
        if !keep_state {
            self.unstate(name);
        }
        self.stop_proxy_if_idle();
    }

    /// Its names fall through to the next connected profile.
    pub fn unmatch(&mut self, name: &str) {
        let others = self.parts.active.names_without(name);
        self.parts.rehome(&self.inventory.lists, &others);
    }

    /// The NRPT rules become the other profiles' union; the cache is flushed.
    pub fn unnrpt(&mut self, name: &str) {
        let others = self.parts.active.names_without(name);
        let _ = self.parts.sync_nrpt(&self.inventory.lists, &others);
    }

    pub fn unresolve(&self, name: &str) {
        self.parts.resolvers.remove(name);
    }

    /// Removes its routes from its adapter and releases its claims. The profile leaves the
    /// connected set; its keys are zeroed when the last reference goes.
    pub fn unroute(&self, name: &str) {
        if let Some(profile) = self.parts.active.remove(name) {
            profile.routes.lock().unwrap().remove_all();
        }
        self.parts.claims.lock().unwrap().release_profile(name);
        self.parts.alerts.clear_connected_conditions(name);
    }

    /// Sets the adapter down and closes it, which removes it.
    pub fn untunnel(&self, name: &str) {
        if let Err(e) = self.parts.tunnel.down(name) {
            self.parts
                .log
                .with_profile(name)
                .error(format!("taking the adapter down failed: {e}"));
        }
    }

    /// The other profiles may route its tunnel subnet again.
    pub fn unguard(&self, name: &str) {
        self.parts.guards.remove_tunnel(name);
        self.parts.apply_guards();
    }

    pub fn unstate(&mut self, name: &str) {
        if self.inventory.state.remove(name)
            && let Err(e) = self.parts.write_state(&self.inventory.state)
        {
            self.parts.log.error(e.to_string());
        }
    }

    /// The proxy stops after the last profile.
    pub fn stop_proxy_if_idle(&mut self) {
        if self.parts.active.is_empty()
            && let Some(proxy) = self.inventory.proxy.take()
        {
            proxy.stop();
        }
    }
}
