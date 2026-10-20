use std::fmt;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, RwLock};

use super::{
    ConnectRun, EngineParts, Inventory, ListRefusal, ListService, ListsReload, Maintenance,
    ProfileError, ProfileOps, SaveRefused, SettingsReload, StoredProfile, Teardown, TunnelWatch,
};
use crate::alerts::{AlertKind, AlertTexts};
use crate::config::{MergedFile, SettingChange};
use crate::profile::{ImportPreview, ImportRequest, Pin, ProfileFacts, ProfileName, ProfileState};
use crate::tunnel::ConnectAttempt;

/// Why a profile operation did nothing.
#[derive(Debug)]
pub enum CoordinatorError {
    UnknownProfile(String),
    /// The Connect failed at a step; everything it did is undone.
    ConnectFailed(Box<ConnectAttempt>),
}

impl fmt::Display for CoordinatorError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CoordinatorError::UnknownProfile(name) => write!(f, "unknown profile {name}"),
            CoordinatorError::ConnectFailed(attempt) => match &attempt.error {
                Some((step, error)) => write!(f, "step {} ({step}) failed: {error}", step.number()),
                None => f.write_str("the connect failed"),
            },
        }
    }
}

/// How the reconnect of a profile whose `upstream` or `mtu` changed went.
pub type Reconnected = (String, Result<(), CoordinatorError>);

/// The one lock over every state change: connect, disconnect, recover, the reloads
/// and the list edits run one at a time. The DNS path never takes it, and neither does status:
/// the profiles on disk are also published (`catalog`) for readers that must not wait for a
/// Connect.
pub struct Coordinator {
    parts: Arc<EngineParts>,
    inventory: Mutex<Inventory>,
    lists: ListService,
    catalog: RwLock<Vec<StoredProfile>>,
    profiles_rev: AtomicU64,
}

impl Coordinator {
    pub fn new(parts: Arc<EngineParts>, inventory: Inventory) -> Self {
        let lists = ListService::new(parts.folder.clone());
        let catalog = RwLock::new(inventory.profiles.profiles());
        Self {
            parts,
            inventory: Mutex::new(inventory),
            lists,
            catalog,
            profiles_rev: AtomicU64::new(1),
        }
    }

    /// The profiles on disk in the user's order, as of the last change; never waits.
    pub fn catalog(&self) -> Vec<StoredProfile> {
        self.catalog.read().unwrap().clone()
    }

    /// Changes whenever a profile is imported, replaced, removed, recoloured or moved.
    pub fn profiles_rev(&self) -> u64 {
        self.profiles_rev.load(Ordering::SeqCst)
    }

    /// `(profile, routes.txt text)` in the user's order, read now; never waits.
    pub fn list_texts(&self) -> std::io::Result<Vec<(String, String)>> {
        self.lists.texts(&self.catalog_names())
    }

    /// Empties the DNS cache and the learned routes of `name`, or of every connected profile,
    /// and flushes the Windows cache: `(cache entries, routes)` removed.
    pub fn reset_cache(&self, name: Option<&str>) -> Result<(u64, u64), CoordinatorError> {
        let inventory = self.lock();
        let names = match name {
            Some(name) => vec![Self::known(&inventory, name)?.to_string()],
            None => self.parts.active.names(),
        };
        let now = self.parts.clock.now();
        let (mut entries, mut routes) = (0, 0);
        for name in names {
            if let Some(resolver) = self.parts.resolvers.get(&name) {
                entries += resolver.clear_cache() as u64;
            }
            if let Some(profile) = self.parts.active.get(&name) {
                routes += profile.routes.lock().unwrap().clear_learned(now) as u64;
                profile.held.lock().unwrap().clear();
                self.parts.claims.lock().unwrap().release_profile(&name);
            }
        }
        self.parts.flush();
        Ok((entries, routes))
    }

    pub fn parts(&self) -> &Arc<EngineParts> {
        &self.parts
    }

    /// Connects `name` until it is disconnected; a connected profile is left as it is.
    pub fn connect(&self, name: &str) -> Result<(), CoordinatorError> {
        let mut inventory = self.lock();
        let name = Self::known(&inventory, name)?;
        if self.parts.active.get(name.as_str()).is_some() {
            return Ok(());
        }
        self.connect_locked(&mut inventory, name)
    }

    pub fn disconnect(&self, name: &str) -> Result<(), CoordinatorError> {
        let mut inventory = self.lock();
        let name = Self::known(&inventory, name)?;
        self.disconnect_locked(&mut inventory, name.as_str(), false);
        Ok(())
    }

    /// Every connected profile; a service stop keeps `connected=` for the next start.
    pub fn disconnect_all(&self, keep_state: bool) {
        let mut inventory = self.lock();
        for name in self.parts.active.names() {
            self.disconnect_locked(&mut inventory, &name, keep_state);
        }
    }

    /// Connects what `connected=` restores at start, then the names asked for (`--connect`).
    pub fn restore(&self, restore_all: bool, extra: &[String]) {
        let names = {
            let inventory = self.lock();
            let profiles: Vec<_> = inventory
                .profiles
                .profiles()
                .into_iter()
                .map(|p| {
                    let settings = self
                        .parts
                        .settings_files
                        .profile(&p.name)
                        .ok()
                        .and_then(Result::ok);
                    (p.name, settings)
                })
                .collect();
            let mut names: Vec<String> = super::BootRestore::plan(
                &inventory.state,
                inventory.settings.is_ok(),
                restore_all,
                &profiles,
            )
            .iter()
            .map(ToString::to_string)
            .collect();
            names.extend(extra.iter().cloned());
            names
        };
        for name in names {
            match self.connect(&name) {
                Ok(()) => {}
                Err(e) => self
                    .parts
                    .log
                    .with_profile(&name)
                    .error(format!("restore: {e}")),
            }
        }
    }

    /// TunnelWatch (every 10 s, and on resume).
    pub fn watch_tunnels(&self) {
        let _inventory = self.lock();
        TunnelWatch::new(&self.parts).tick();
    }

    /// Maintenance (every 30 s).
    pub fn maintain(&self) {
        let mut inventory = self.lock();
        Maintenance::new(&self.parts, &mut inventory).tick();
    }

    /// Reads `splitwire.conf` and every `profile.conf` again: live keys apply at once, the
    /// connected profiles whose `upstream` or `mtu` changed reconnect.
    pub fn reload_settings(&self) {
        let mut inventory = self.lock();
        self.reload_settings_locked(&mut inventory);
    }

    /// The revision `save_settings` must carry: it changes with any settings file's text. Never
    /// waits.
    pub fn settings_rev(&self) -> u64 {
        self.settings_rev_of(&self.catalog_names())
    }

    /// SaveSettings: merged into the affected files, validated, written all or none,
    /// then applied; the connected profiles whose `upstream` or `mtu` changed reconnect, and how
    /// each went is returned.
    pub fn save_settings(
        &self,
        base_rev: u64,
        changes: &[SettingChange],
    ) -> Result<(Vec<MergedFile>, Vec<Reconnected>), SaveRefused> {
        let mut inventory = self.lock();
        let names = Self::names(&inventory);
        if self.settings_rev_of(&names) != base_rev {
            return Err(SaveRefused::Stale);
        }
        let merged = self.parts.settings_files.save(changes, &names)?;
        let reconnects = self.reload_settings_locked(&mut inventory);
        Ok((merged, reconnects))
    }

    /// Reads the lists again: problems become alerts, and the connected profiles pick up the
    /// change through their statics, the matcher (re-homing) and NRPT.
    pub fn reload_lists(&self) {
        let mut inventory = self.lock();
        self.reload_lists_locked(&mut inventory);
    }

    /// The revision the next list edit must carry; never waits.
    pub fn lists_rev(&self) -> std::io::Result<u64> {
        Ok(ListService::rev(&self.list_texts()?))
    }

    /// Adds `entry` to `name`'s list, or removes `remove = (line, text)` from it.
    pub fn edit_list(
        &self,
        base_rev: u64,
        name: &str,
        add: Option<&str>,
        remove: Option<(usize, &str)>,
    ) -> Result<(), ListRefusal> {
        self.change_lists(|lists, names, inventory| {
            let name = Self::known_list(inventory, name)?;
            if let Some(entry) = add {
                lists.add(names, base_rev, &name, entry)?;
            }
            if let Some((line, text)) = remove {
                let rev = ListService::rev(&lists.texts(names)?);
                lists.remove(names, rev, &name, line, text)?;
            }
            Ok(())
        })
    }

    pub fn save_list(&self, base_rev: u64, name: &str, text: &str) -> Result<(), ListRefusal> {
        self.change_lists(|lists, names, inventory| {
            let name = Self::known_list(inventory, name)?;
            lists.save(names, base_rev, &name, text)
        })
    }

    pub fn save_all_lists(&self, base_rev: u64, text: &str) -> Result<(), ListRefusal> {
        self.change_lists(|lists, names, _| lists.save_all(names, base_rev, text))
    }

    /// What importing `request` would do; stores nothing.
    pub fn preview_import(&self, request: &ImportRequest) -> ImportPreview {
        let mut inventory = self.lock();
        ProfileOps::new(&self.parts, &mut inventory).preview(request)
    }

    /// Imports a new profile, or replaces a stored one's `.conf`; a connected profile replaced
    /// reconnects.
    pub fn import(
        &self,
        name: &str,
        conf_text: &str,
        colour: Pin,
        replace: bool,
    ) -> Result<ProfileFacts, ProfileError> {
        let mut inventory = self.lock();
        let facts = ProfileOps::new(&self.parts, &mut inventory)
            .import(name, conf_text, colour, replace)?;
        let stored =
            Self::known(&inventory, name).map_err(|_| ProfileError::UnknownProfile(name.into()))?;
        self.after_profiles_changed(&mut inventory);
        self.publish(&inventory);
        if replace && self.parts.active.contains(stored.as_str()) {
            let name = stored.to_string();
            if let Err(e) = self.reconnect_locked(&mut inventory, stored) {
                self.parts
                    .log
                    .with_profile(&name)
                    .error(format!("reconnect: {e}"));
            }
        }
        Ok(facts)
    }

    /// Disconnects the profile, then deletes its folder and its `index.conf` line.
    pub fn remove(&self, name: &str) -> Result<(), ProfileError> {
        let mut inventory = self.lock();
        let stored =
            Self::known(&inventory, name).map_err(|_| ProfileError::UnknownProfile(name.into()))?;
        self.disconnect_locked(&mut inventory, stored.as_str(), false);
        ProfileOps::new(&self.parts, &mut inventory).remove(stored.as_str())?;
        self.after_profiles_changed(&mut inventory);
        self.publish(&inventory);
        Ok(())
    }

    pub fn set_colour(&self, name: &str, colour: Pin) -> Result<(), ProfileError> {
        let mut inventory = self.lock();
        ProfileOps::new(&self.parts, &mut inventory).set_colour(name, colour)?;
        self.publish(&inventory);
        Ok(())
    }

    pub fn move_profile(&self, name: &str, position: usize) -> Result<(), ProfileError> {
        let mut inventory = self.lock();
        ProfileOps::new(&self.parts, &mut inventory).move_profile(name, position)?;
        self.publish(&inventory);
        Ok(())
    }

    /// The profiles in the user's order with their states.
    pub fn profiles(&self) -> Vec<(String, ProfileState)> {
        let inventory = self.lock();
        Self::names(&inventory)
            .into_iter()
            .map(|name| {
                let state = self.parts.states.state(name.as_str());
                (name.to_string(), state)
            })
            .collect()
    }

    /// A hash of the settings files' and lists' sizes and times: the file watcher compares it.
    pub fn file_signature(&self) -> (u64, u64) {
        let (settings, lists) = {
            let inventory = self.lock();
            let names = Self::names(&inventory);
            let folder = &self.parts.folder;
            let mut settings: Vec<PathBuf> = vec![folder.settings()];
            settings.extend(names.iter().map(|n| folder.profile_settings(n)));
            let lists: Vec<PathBuf> = names.iter().map(|n| folder.list(n)).collect();
            (settings, lists)
        };
        (Self::signature(&settings), Self::signature(&lists))
    }

    fn signature(paths: &[PathBuf]) -> u64 {
        let mut hasher = DefaultHasher::new();
        for path in paths {
            path.hash(&mut hasher);
            if let Ok(meta) = std::fs::metadata(path) {
                meta.len().hash(&mut hasher);
                meta.modified().ok().hash(&mut hasher);
            }
        }
        hasher.finish()
    }

    fn change_lists(
        &self,
        change: impl FnOnce(&ListService, &[ProfileName], &Inventory) -> Result<(), ListRefusal>,
    ) -> Result<(), ListRefusal> {
        let mut inventory = self.lock();
        let names = Self::names(&inventory);
        let result = change(&self.lists, &names, &inventory);
        // A stale request means a hand edit: reload either way.
        self.reload_lists_locked(&mut inventory);
        result
    }

    fn reload_lists_locked(&self, inventory: &mut Inventory) {
        let names = Self::names(inventory);
        match self.lists.texts(&names) {
            Ok(texts) => ListsReload::new(&self.parts, inventory).apply(&texts),
            Err(e) => self
                .parts
                .log
                .error(format!("reading the lists failed: {e}")),
        }
    }

    fn reload_settings_locked(&self, inventory: &mut Inventory) -> Vec<Reconnected> {
        let reconnect = SettingsReload::new(&self.parts, inventory).reload();
        let mut outcomes = Vec::new();
        for name in reconnect {
            let log = self.parts.log.with_profile(name.as_str());
            log.info("upstream or mtu changed: reconnecting");
            let key = name.to_string();
            let outcome = self.reconnect_locked(inventory, name);
            if let Err(e) = &outcome {
                log.error(format!("reconnect: {e}"));
            }
            outcomes.push((key, outcome));
        }
        outcomes
    }

    fn settings_rev_of(&self, names: &[ProfileName]) -> u64 {
        let files = &self.parts.settings_files;
        let mut hasher = DefaultHasher::new();
        files.global_text().unwrap_or_default().hash(&mut hasher);
        for name in names {
            name.as_str().hash(&mut hasher);
            files
                .profile_text(name)
                .unwrap_or_default()
                .hash(&mut hasher);
        }
        hasher.finish()
    }

    /// A profile's files changed: the settings (endpoint hosts in `bypass`), the lists and the
    /// states follow.
    fn after_profiles_changed(&self, inventory: &mut Inventory) {
        SettingsReload::new(&self.parts, inventory).reload();
        self.reload_lists_locked(inventory);
    }

    /// Disconnects and connects again.
    fn reconnect_locked(
        &self,
        inventory: &mut Inventory,
        name: ProfileName,
    ) -> Result<(), CoordinatorError> {
        self.disconnect_locked(inventory, name.as_str(), false);
        self.connect_locked(inventory, name)
    }

    /// Makes the profiles on disk the published catalog.
    fn publish(&self, inventory: &Inventory) {
        *self.catalog.write().unwrap() = inventory.profiles.profiles();
        self.profiles_rev.fetch_add(1, Ordering::SeqCst);
    }

    fn catalog_names(&self) -> Vec<ProfileName> {
        self.catalog
            .read()
            .unwrap()
            .iter()
            .map(|p| p.name.clone())
            .collect()
    }

    fn connect_locked(
        &self,
        inventory: &mut Inventory,
        name: ProfileName,
    ) -> Result<(), CoordinatorError> {
        let key = name.to_string();
        // The alert of the last failure ends when the next attempt starts.
        self.parts
            .alerts
            .clear(AlertKind::ConnectFailed, Some(&key));
        let result = ConnectRun::new(&self.parts, inventory, name).run();
        match result {
            Ok(attempt) => {
                self.parts.states.attempt(&key, attempt);
                self.parts.states.set(
                    &key,
                    ProfileState::Connected {
                        health: crate::tunnel::HandshakeHealth::Ok,
                    },
                );
                Ok(())
            }
            Err(attempt) => {
                self.parts
                    .alerts
                    .raise(AlertTexts::connect_failed(&key, &attempt));
                self.parts.states.attempt(&key, attempt.clone());
                self.parts.states.set(&key, ProfileState::Failed);
                Err(CoordinatorError::ConnectFailed(Box::new(attempt)))
            }
        }
    }

    fn disconnect_locked(&self, inventory: &mut Inventory, name: &str, keep_state: bool) {
        if !self.parts.active.contains(name) {
            if !keep_state {
                Teardown::new(&self.parts, inventory).unstate(name);
            }
            return;
        }
        self.parts.states.set(name, ProfileState::Disconnecting);
        Teardown::new(&self.parts, inventory).disconnect(name, keep_state);
        self.parts.states.disconnected(name, self.parts.clock.now());
        self.parts.log.with_profile(name).info("disconnected");
    }

    fn known(inventory: &Inventory, name: &str) -> Result<ProfileName, CoordinatorError> {
        inventory
            .profiles
            .find(name)
            .map(|p| p.name)
            .ok_or_else(|| CoordinatorError::UnknownProfile(name.to_owned()))
    }

    fn known_list(inventory: &Inventory, name: &str) -> Result<ProfileName, ListRefusal> {
        Self::known(inventory, name).map_err(|_| ListRefusal::UnknownProfile(name.to_owned()))
    }

    fn names(inventory: &Inventory) -> Vec<ProfileName> {
        inventory
            .profiles
            .profiles()
            .into_iter()
            .map(|p| p.name)
            .collect()
    }

    fn lock(&self) -> MutexGuard<'_, Inventory> {
        self.inventory
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}
