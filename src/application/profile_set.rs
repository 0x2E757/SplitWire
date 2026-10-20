use std::fmt;
use std::fs;
use std::io;
use std::net::Ipv4Addr;
use std::sync::Arc;

use super::DataFolder;
use crate::clock::{Clock, unix_secs};
use crate::config::{ProfileIndex, ProfileSettings};
use crate::list::ListSet;
use crate::profile::{
    AdapterGuid, ImportContext, ImportPreview, ImportRequest, KnownProfile, Pin, Profile,
    ProfileCodec, ProfileFacts, ProfileName, SecretStore, WgConfParser,
};

/// A profile on disk: its name, colour and, when its blob could be read, its facts.
#[derive(Clone, Debug)]
pub struct StoredProfile {
    pub name: ProfileName,
    pub colour: Pin,
    pub facts: Result<ProfileFacts, String>,
}

/// Why a profile operation did nothing.
#[derive(Debug)]
pub enum ProfileError {
    UnknownProfile(String),
    /// The problems, one line each; nothing was written.
    Refused(Vec<String>),
    Io(io::Error),
}

impl fmt::Display for ProfileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ProfileError::UnknownProfile(name) => write!(f, "unknown profile {name}"),
            ProfileError::Refused(problems) => f.write_str(&problems.join("; ")),
            ProfileError::Io(error) => error.fmt(f),
        }
    }
}

impl From<io::Error> for ProfileError {
    fn from(error: io::Error) -> Self {
        ProfileError::Io(error)
    }
}

/// What a preview checks besides the stored profiles; the engine knows these.
#[derive(Clone, Copy)]
pub struct PreviewInputs<'a> {
    pub lists: &'a ListSet,
    pub connected: &'a [String],
    pub endpoint_addresses: &'a [Ipv4Addr],
    pub answers: &'a [(String, Ipv4Addr)],
}

/// The profiles on disk: import, replace, remove, colour and order. Keys are decrypted only to
/// read the public facts at load, to preview a replace and to connect; they are zeroed on drop.
pub struct ProfileSet {
    folder: DataFolder,
    store: Arc<dyn SecretStore>,
    clock: Arc<dyn Clock>,
    index: ProfileIndex,
    facts: Vec<(ProfileName, Result<ProfileFacts, String>)>,
}

impl ProfileSet {
    /// Reads `profiles\`, makes `index.conf` match the folders, and reads every blob's facts.
    pub fn load(
        folder: DataFolder,
        store: Arc<dyn SecretStore>,
        clock: Arc<dyn Clock>,
    ) -> io::Result<Self> {
        let names = folder.profile_folders()?;
        let mut index = ProfileIndex::parse(&folder.read_text(&folder.index())?);
        let changed = index.reconcile(&names);
        let mut set = Self {
            folder,
            store,
            clock,
            index,
            facts: Vec::new(),
        };
        if changed {
            set.write_index()?;
        }
        set.facts = names
            .into_iter()
            .map(|name| {
                let facts = set.open(&name).map(|profile| profile.facts.clone());
                (name, facts.map_err(|error| error.to_string()))
            })
            .collect();
        Ok(set)
    }

    /// The profiles in the user's order.
    pub fn profiles(&self) -> Vec<StoredProfile> {
        self.index
            .entries()
            .iter()
            .map(|(name, colour)| StoredProfile {
                name: name.clone(),
                colour: *colour,
                facts: self.facts_of(name).clone(),
            })
            .collect()
    }

    /// A profile by name, ignoring case.
    pub fn find(&self, name: &str) -> Option<StoredProfile> {
        self.profiles().into_iter().find(|p| p.name.same_as(name))
    }

    /// The profile with its keys, decrypted; for Connect and a replace's diff only.
    pub fn open(&self, name: &ProfileName) -> Result<Profile, ProfileError> {
        let blob = fs::read(self.folder.blob(name))?;
        let plaintext = self.store.unprotect(&blob)?;
        ProfileCodec::decode(&plaintext)
            .map_err(|reason| ProfileError::Refused(vec![format!("{name}: profile.dat: {reason}")]))
    }

    /// What importing `request` would do; stores nothing.
    pub fn preview(&self, request: &ImportRequest, inputs: PreviewInputs) -> ImportPreview {
        let known = self.known(inputs.connected);
        let replaced = self
            .exact(request.name)
            .and_then(|name| self.open(&name).ok());
        ImportPreview::build(
            request,
            &ImportContext {
                profiles: &known,
                lists: inputs.lists,
                replaced: replaced.as_ref(),
                endpoint_addresses: inputs.endpoint_addresses,
                answers: inputs.answers,
            },
        )
    }

    /// Imports a new profile (off, empty list, `connect_at_boot=false`, at the end of the order) or
    /// replaces the `.conf` of an existing one (its list, settings, colour, position and adapter
    /// GUID stay). It repeats the preview's validation; a failure changes nothing.
    pub fn import(
        &mut self,
        name: &str,
        conf_text: &str,
        colour: Pin,
        replace: bool,
    ) -> Result<ProfileFacts, ProfileError> {
        let exact = self.exact(name);
        match (replace, &exact) {
            (true, None) => return Err(ProfileError::UnknownProfile(name.to_owned())),
            (false, Some(_)) => {
                let problem = format!("{name} already exists; choose another name");
                return Err(ProfileError::Refused(vec![problem]));
            }
            _ => {}
        }
        let parsed = WgConfParser::parse(conf_text);
        let known = self.known(&[]);
        let replaced = exact.as_ref().and_then(|n| self.open(n).ok());
        let lists = ListSet::default();
        let request = ImportRequest {
            name,
            file_name: "",
            conf_text,
        };
        let preview = ImportPreview::of_parsed(
            &request,
            &parsed,
            &ImportContext {
                profiles: &known,
                lists: &lists,
                replaced: replaced.as_ref(),
                endpoint_addresses: &[],
                answers: &[],
            },
        );
        if preview.blocks() {
            return Err(ProfileError::Refused(preview.problems()));
        }
        let conf = parsed
            .into_conf()
            .expect("a preview without problems has a conf");
        let imported = unix_secs(self.clock.now());
        match exact {
            Some(name) => {
                let guid = self
                    .facts_of(&name)
                    .as_ref()
                    .map_or_else(|_| self.new_guid(), |f| f.guid);
                let profile = Profile::from_conf(conf, guid, imported);
                self.write_blob(&name, &profile)?;
                self.set_facts(&name, Ok(profile.facts.clone()));
                Ok(profile.facts)
            }
            None => {
                let name = ProfileName::parse(name).map_err(|p| ProfileError::Refused(vec![p]))?;
                let profile = Profile::from_conf(conf, self.new_guid(), imported);
                self.create(&name, &profile, colour)?;
                Ok(profile.facts)
            }
        }
    }

    /// Deletes the profile's folder and its `index.conf` line. Disconnecting first is the caller's.
    pub fn remove(&mut self, name: &str) -> Result<(), ProfileError> {
        let name = self.existing(name)?;
        fs::remove_dir_all(self.folder.profile(&name))?;
        self.index.remove(name.as_str());
        self.facts.retain(|(n, _)| n != &name);
        self.write_index()?;
        Ok(())
    }

    pub fn set_colour(&mut self, name: &str, colour: Pin) -> Result<(), ProfileError> {
        let name = self.existing(name)?;
        self.index.set_pin(name.as_str(), colour);
        Ok(self.write_index()?)
    }

    /// Moves the profile to `position` in the order (clamped to the end).
    pub fn move_profile(&mut self, name: &str, position: usize) -> Result<(), ProfileError> {
        let name = self.existing(name)?;
        self.index.move_to(name.as_str(), position);
        Ok(self.write_index()?)
    }

    fn create(
        &mut self,
        name: &ProfileName,
        profile: &Profile,
        colour: Pin,
    ) -> Result<(), ProfileError> {
        let directory = self.folder.profile(name);
        let created = !directory.exists();
        let result = (|| -> Result<(), ProfileError> {
            fs::create_dir_all(&directory)?;
            self.folder.write(&self.folder.list(name), b"")?;
            self.folder.write(
                &self.folder.profile_settings(name),
                ProfileSettings::NEW_FILE.as_bytes(),
            )?;
            self.write_blob(name, profile)?;
            self.index.push(name.clone(), colour);
            if let Err(error) = self.write_index() {
                self.index.remove(name.as_str());
                return Err(error.into());
            }
            Ok(())
        })();
        match result {
            Ok(()) => {
                self.facts.push((name.clone(), Ok(profile.facts.clone())));
                Ok(())
            }
            Err(error) => {
                if created {
                    let _ = fs::remove_dir_all(&directory);
                }
                Err(error)
            }
        }
    }

    fn write_blob(&self, name: &ProfileName, profile: &Profile) -> Result<(), ProfileError> {
        let blob = self.store.protect(&ProfileCodec::encode(profile))?;
        Ok(self.folder.write(&self.folder.blob(name), &blob)?)
    }

    fn write_index(&self) -> io::Result<()> {
        fs::create_dir_all(self.folder.profiles())?;
        self.folder
            .write(&self.folder.index(), self.index.render().as_bytes())
    }

    fn known(&self, connected: &[String]) -> Vec<KnownProfile> {
        self.profiles()
            .into_iter()
            .map(|p| KnownProfile {
                connected: connected.iter().any(|c| p.name.same_as(c)),
                facts: p.facts.ok(),
                name: p.name,
                colour: p.colour,
            })
            .collect()
    }

    /// The stored name spelled exactly as `name`.
    fn exact(&self, name: &str) -> Option<ProfileName> {
        self.index
            .entries()
            .iter()
            .map(|(n, _)| n)
            .find(|n| n.as_str() == name)
            .cloned()
    }

    fn existing(&self, name: &str) -> Result<ProfileName, ProfileError> {
        self.index
            .position(name)
            .map(|at| self.index.entries()[at].0.clone())
            .ok_or_else(|| ProfileError::UnknownProfile(name.to_owned()))
    }

    fn facts_of(&self, name: &ProfileName) -> &Result<ProfileFacts, String> {
        static MISSING: Result<ProfileFacts, String> = Err(String::new());
        self.facts
            .iter()
            .find(|(n, _)| n == name)
            .map_or(&MISSING, |(_, facts)| facts)
    }

    fn set_facts(&mut self, name: &ProfileName, facts: Result<ProfileFacts, String>) {
        match self.facts.iter_mut().find(|(n, _)| n == name) {
            Some(slot) => slot.1 = facts,
            None => self.facts.push((name.clone(), facts)),
        }
    }

    /// A GUID no stored profile has.
    fn new_guid(&self) -> u128 {
        loop {
            let guid = AdapterGuid::random();
            if !self
                .facts
                .iter()
                .any(|(_, f)| f.as_ref().is_ok_and(|f| f.guid == guid))
            {
                return guid;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::data_folder::tests::scratch;
    use crate::clock::ManualClock;
    use crate::profile::fixtures::{FakeStore, OTHER_PRIVATE, PRESHARED, PRIVATE, conf, full_conf};
    use crate::profile::{Base64, SecretBytes};
    use std::time::{Duration, UNIX_EPOCH};

    fn open_set(folder: &DataFolder) -> ProfileSet {
        let clock = Arc::new(ManualClock::new(
            UNIX_EPOCH + Duration::from_secs(1_792_423_315),
        ));
        ProfileSet::load(folder.clone(), Arc::new(FakeStore), clock).unwrap()
    }

    fn pin(number: u8) -> Pin {
        Pin::new(number).unwrap()
    }

    fn names(set: &ProfileSet) -> Vec<String> {
        set.profiles()
            .iter()
            .map(|p| format!("{}={}", p.name, p.colour))
            .collect()
    }

    fn partner() -> String {
        conf(
            OTHER_PRIVATE,
            "10.77.0.5/32",
            "10.77.0.53",
            "vpn.partner.example:51820",
        )
    }

    #[test]
    fn a_new_profile_gets_its_folder_files_and_index_line() {
        let folder = scratch("import");
        let mut set = open_set(&folder);
        let facts = set.import("AcmeCorp", &full_conf(), pin(2), false).unwrap();
        assert_eq!(facts.imported, 1_792_423_315);
        let acme = ProfileName::parse("AcmeCorp").unwrap();
        assert_eq!(fs::read_to_string(folder.list(&acme)).unwrap(), "");
        assert_eq!(
            fs::read_to_string(folder.profile_settings(&acme)).unwrap(),
            "connect_at_boot=false\n"
        );
        assert_eq!(
            fs::read_to_string(folder.index()).unwrap(),
            "AcmeCorp = 2\n"
        );
        let blob = fs::read(folder.blob(&acme)).unwrap();
        assert!(blob.starts_with(FakeStore::MARKER));
        let reloaded = open_set(&folder);
        assert_eq!(names(&reloaded), ["AcmeCorp=2"]);
        assert_eq!(reloaded.find("acmecorp").unwrap().facts.unwrap(), facts);
        let profile = reloaded.open(&acme).unwrap();
        assert_eq!(Base64::encode(profile.private_key.expose()), PRIVATE);
        fs::remove_dir_all(folder.root()).unwrap();
    }

    #[test]
    fn replace_swaps_the_blob_and_keeps_everything_else() {
        let folder = scratch("replace");
        let mut set = open_set(&folder);
        let first = set.import("AcmeCorp", &full_conf(), pin(4), false).unwrap();
        set.import("Partner", &partner(), pin(1), false).unwrap();
        let acme = ProfileName::parse("AcmeCorp").unwrap();
        fs::write(folder.list(&acme), "corp.com\n").unwrap();
        fs::write(folder.profile_settings(&acme), "connect_at_boot=true\n").unwrap();
        let text = conf(
            OTHER_PRIVATE,
            "10.88.4.12/24",
            "10.66.0.59",
            "vpn.example.com:51820",
        );
        let second = set.import("AcmeCorp", &text, pin(6), true).unwrap();
        assert_eq!(second.guid, first.guid);
        assert_eq!(second.dns, [Ipv4Addr::new(10, 66, 0, 59)]);
        assert_eq!(names(&set), ["AcmeCorp=4", "Partner=1"]);
        assert_eq!(
            fs::read_to_string(folder.list(&acme)).unwrap(),
            "corp.com\n"
        );
        assert_eq!(
            fs::read_to_string(folder.profile_settings(&acme)).unwrap(),
            "connect_at_boot=true\n"
        );
        let stored = open_set(&folder).open(&acme).unwrap();
        assert_eq!(Base64::encode(stored.private_key.expose()), OTHER_PRIVATE);
        fs::remove_dir_all(folder.root()).unwrap();
    }

    #[test]
    fn refusals_change_nothing() {
        let folder = scratch("refuse");
        let mut set = open_set(&folder);
        set.import("AcmeCorp", &full_conf(), pin(1), false).unwrap();
        let blob = fs::read(folder.blob(&ProfileName::parse("AcmeCorp").unwrap())).unwrap();
        let refused = |result: Result<ProfileFacts, ProfileError>| match result {
            Err(ProfileError::Refused(problems)) => problems,
            other => panic!("{other:?}"),
        };
        assert_eq!(
            refused(set.import("acmecorp", &partner(), pin(2), false)),
            ["AcmeCorp already exists; choose another name"]
        );
        assert_eq!(
            refused(set.import("AcmeCorp", &partner(), pin(2), false)),
            ["AcmeCorp already exists; choose another name"]
        );
        assert!(matches!(
            set.import("Ghost", &partner(), pin(2), true),
            Err(ProfileError::UnknownProfile(_))
        ));
        let overlap = conf(
            OTHER_PRIVATE,
            "10.88.4.99/32",
            "10.9.9.9",
            "198.51.100.9:51820",
        );
        assert!(
            refused(set.import("Overlap", &overlap, pin(2), false))[0]
                .contains("Overlaps AcmeCorp")
        );
        assert!(!refused(set.import("AcmeCorp", "[Interface]\n", pin(1), true)).is_empty());
        assert_eq!(
            fs::read(folder.blob(&ProfileName::parse("AcmeCorp").unwrap())).unwrap(),
            blob
        );
        assert_eq!(names(&set), ["AcmeCorp=1"]);
        assert!(!folder.profiles().join("Overlap").exists());
        fs::remove_dir_all(folder.root()).unwrap();
    }

    #[test]
    fn a_failing_store_leaves_no_folder_behind() {
        struct BrokenStore;
        impl SecretStore for BrokenStore {
            fn protect(&self, _: &SecretBytes) -> io::Result<Vec<u8>> {
                Err(io::Error::other("no DPAPI"))
            }
            fn unprotect(&self, _: &[u8]) -> io::Result<SecretBytes> {
                Err(io::Error::other("no DPAPI"))
            }
        }
        let folder = scratch("broken");
        let clock = Arc::new(ManualClock::new(UNIX_EPOCH));
        let mut set = ProfileSet::load(folder.clone(), Arc::new(BrokenStore), clock).unwrap();
        assert!(matches!(
            set.import("AcmeCorp", &full_conf(), pin(1), false),
            Err(ProfileError::Io(_))
        ));
        assert!(!folder.profiles().join("AcmeCorp").exists());
        assert!(set.profiles().is_empty());
        fs::remove_dir_all(folder.root()).unwrap();
    }

    #[test]
    fn removes_recolours_and_reorders_at_once() {
        let folder = scratch("order");
        let mut set = open_set(&folder);
        set.import("AcmeCorp", &full_conf(), pin(1), false).unwrap();
        set.import("Partner", &partner(), pin(2), false).unwrap();
        let home = conf(
            OTHER_PRIVATE,
            "10.3.0.2/32",
            "10.3.0.1",
            "198.51.100.30:51820",
        );
        set.import("HomeLab", &home, pin(3), false).unwrap();
        set.move_profile("homelab", 0).unwrap();
        set.set_colour("Partner", pin(5)).unwrap();
        assert_eq!(
            fs::read_to_string(folder.index()).unwrap(),
            "HomeLab = 3\nAcmeCorp = 1\nPartner = 5\n"
        );
        set.remove("acmecorp").unwrap();
        assert!(!folder.profiles().join("AcmeCorp").exists());
        assert_eq!(names(&open_set(&folder)), ["HomeLab=3", "Partner=5"]);
        assert!(matches!(
            set.remove("AcmeCorp"),
            Err(ProfileError::UnknownProfile(_))
        ));
        assert!(matches!(
            set.set_colour("Ghost", pin(1)),
            Err(ProfileError::UnknownProfile(_))
        ));
        fs::remove_dir_all(folder.root()).unwrap();
    }

    #[test]
    fn load_reconciles_the_index_and_keeps_unreadable_profiles() {
        let folder = scratch("load");
        let mut set = open_set(&folder);
        set.import("AcmeCorp", &full_conf(), pin(1), false).unwrap();
        set.import("Partner", &partner(), pin(2), false).unwrap();
        fs::write(folder.index(), "Gone = 4\nPartner = 2\n").unwrap();
        fs::write(
            folder.blob(&ProfileName::parse("Partner").unwrap()),
            b"damaged",
        )
        .unwrap();
        let reloaded = open_set(&folder);
        assert_eq!(names(&reloaded), ["Partner=2", "AcmeCorp=1"]);
        assert_eq!(
            fs::read_to_string(folder.index()).unwrap(),
            "Partner = 2\nAcmeCorp = 1\n"
        );
        assert!(reloaded.find("Partner").unwrap().facts.is_err());
        fs::remove_dir_all(folder.root()).unwrap();
    }

    #[test]
    fn previews_through_the_set_mark_a_connected_replace() {
        let folder = scratch("preview");
        let mut set = open_set(&folder);
        set.import("AcmeCorp", &full_conf(), pin(1), false).unwrap();
        let lists = ListSet::default();
        let connected = ["AcmeCorp".to_owned()];
        let inputs = PreviewInputs {
            lists: &lists,
            connected: &connected,
            endpoint_addresses: &[],
            answers: &[],
        };
        let text = full_conf();
        let request = ImportRequest {
            name: "AcmeCorp",
            file_name: "AcmeCorp.conf",
            conf_text: &text,
        };
        let preview = set.preview(&request, inputs);
        assert!(preview.replace);
        assert_eq!(preview.changes.last().unwrap().new, "changed: no");
        assert_eq!(preview.checks.last().unwrap().title, "Will reconnect");
        fs::remove_dir_all(folder.root()).unwrap();
    }

    /// The redaction scan after a fixture import: no file but the blob, and nothing the set
    /// reports, holds key material.
    #[test]
    fn no_key_reaches_any_file_but_the_blob() {
        let folder = scratch("redaction");
        let mut set = open_set(&folder);
        set.import("AcmeCorp", &full_conf(), pin(1), false).unwrap();
        let mut shown = format!("{:?}", set.profiles());
        for path in [
            folder.index(),
            folder.profiles().join("AcmeCorp").join("routes.txt"),
        ] {
            shown.push_str(&fs::read_to_string(path).unwrap());
        }
        shown.push_str(
            &fs::read_to_string(folder.profile_settings(&ProfileName::parse("AcmeCorp").unwrap()))
                .unwrap(),
        );
        let blob = fs::read(folder.blob(&ProfileName::parse("AcmeCorp").unwrap())).unwrap();
        shown.push_str(&String::from_utf8_lossy(&blob));
        for secret in [PRIVATE, PRESHARED] {
            assert!(!shown.contains(&secret[..12]), "a key appears");
        }
        fs::remove_dir_all(folder.root()).unwrap();
    }
}
