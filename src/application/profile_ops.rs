use std::net::Ipv4Addr;

use super::{EngineParts, Inventory, PreviewInputs, ProfileError};
use crate::profile::{Endpoint, ImportPreview, ImportRequest, Pin, ProfileFacts, WgConfParser};

/// The profiles on disk as the engine changes them: the import preview with what the
/// engine knows (the endpoint resolved through the fallback, the connected profiles and their
/// answers), import, remove, colour and order. Reconnecting is the coordinator's.
pub struct ProfileOps<'a> {
    parts: &'a EngineParts,
    inventory: &'a mut Inventory,
}

impl<'a> ProfileOps<'a> {
    pub fn new(parts: &'a EngineParts, inventory: &'a mut Inventory) -> Self {
        Self { parts, inventory }
    }

    /// Stores nothing; the endpoint is looked up without becoming a never-route.
    pub fn preview(&self, request: &ImportRequest) -> ImportPreview {
        let parsed = WgConfParser::parse(request.conf_text);
        let endpoint_addresses = parsed
            .conf()
            .map(|conf| self.lookup(&conf.endpoint))
            .unwrap_or_default();
        let connected = self.parts.active.names();
        let answers: Vec<(String, Ipv4Addr)> = self
            .parts
            .active
            .all()
            .iter()
            .flat_map(|profile| {
                let learned = profile.routes.lock().unwrap().learned_ips();
                learned
                    .into_iter()
                    .map(|ip| (profile.name.clone(), ip))
                    .collect::<Vec<_>>()
            })
            .collect();
        self.inventory.profiles.preview(
            request,
            PreviewInputs {
                lists: &self.inventory.lists,
                connected: &connected,
                endpoint_addresses: &endpoint_addresses,
                answers: &answers,
            },
        )
    }

    pub fn import(
        &mut self,
        name: &str,
        conf_text: &str,
        colour: Pin,
        replace: bool,
    ) -> Result<ProfileFacts, ProfileError> {
        self.inventory
            .profiles
            .import(name, conf_text, colour, replace)
    }

    /// Deletes the profile's folder; it must be disconnected first.
    pub fn remove(&mut self, name: &str) -> Result<(), ProfileError> {
        self.inventory.profiles.remove(name)?;
        let names: Vec<String> = self
            .inventory
            .profiles
            .profiles()
            .into_iter()
            .map(|p| p.name.to_string())
            .collect();
        self.parts.states.retain(&names);
        Ok(())
    }

    pub fn set_colour(&mut self, name: &str, colour: Pin) -> Result<(), ProfileError> {
        self.inventory.profiles.set_colour(name, colour)
    }

    pub fn move_profile(&mut self, name: &str, position: usize) -> Result<(), ProfileError> {
        self.inventory.profiles.move_profile(name, position)
    }

    fn lookup(&self, endpoint: &Endpoint) -> Vec<Ipv4Addr> {
        match endpoint.literal() {
            Some(address) => vec![address],
            None => self
                .parts
                .endpoints
                .lookup(&endpoint.host.to_string())
                .unwrap_or_default(),
        }
    }
}
