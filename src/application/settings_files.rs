use std::io;
use std::path::PathBuf;

use super::DataFolder;
use crate::config::{
    MergedFile, ProfileSettings, SettingChange, SettingProblem, Settings, SettingsMerge,
    SettingsScope,
};
use crate::profile::ProfileName;

/// The outcome of a save that wrote nothing.
#[derive(Debug)]
pub enum SaveRefused {
    /// The files changed since `base_rev` (a hand edit): the caller reloads.
    Stale,
    Problems(Vec<SettingProblem>),
    Io(io::Error),
}

/// `splitwire.conf` and the profiles' `profile.conf` on disk: read, and save changes to all
/// affected files or to none. Applying the new values is the caller's.
pub struct SettingsFiles {
    folder: DataFolder,
    dry_run: bool,
}

impl SettingsFiles {
    /// `dry_run` allows any loopback port for `listen`.
    pub fn new(folder: DataFolder, dry_run: bool) -> Self {
        Self { folder, dry_run }
    }

    pub fn global_text(&self) -> io::Result<String> {
        self.folder.read_text(&self.folder.settings())
    }

    pub fn profile_text(&self, name: &ProfileName) -> io::Result<String> {
        self.folder.read_text(&self.folder.profile_settings(name))
    }

    /// The global settings; a missing file gives the defaults.
    pub fn global(&self) -> io::Result<Result<Settings, Vec<SettingProblem>>> {
        Ok(Settings::parse(&self.global_text()?, self.dry_run))
    }

    pub fn profile(
        &self,
        name: &ProfileName,
    ) -> io::Result<Result<ProfileSettings, Vec<SettingProblem>>> {
        Ok(ProfileSettings::parse(
            name.as_str(),
            &self.profile_text(name)?,
        ))
    }

    /// Merges `changes` into their files, validates every affected file and writes them all, or
    /// none when any is invalid. `profiles` are the existing profiles.
    pub fn save(
        &self,
        changes: &[SettingChange],
        profiles: &[ProfileName],
    ) -> Result<Vec<MergedFile>, SaveRefused> {
        let mut unreadable = None;
        let planned = SettingsMerge::plan(
            changes,
            |scope| {
                let path = self.path(scope, profiles)?;
                self.folder
                    .read_text(&path)
                    .map_err(|error| unreadable = Some(error))
                    .ok()
            },
            self.dry_run,
        );
        if let Some(error) = unreadable {
            return Err(SaveRefused::Io(error));
        }
        let merged = planned.map_err(SaveRefused::Problems)?;
        let files: Vec<(PathBuf, Vec<u8>)> = merged
            .iter()
            .map(|file| {
                let path = self
                    .path(&file.scope, profiles)
                    .expect("planned scopes exist");
                (path, file.text.clone().into_bytes())
            })
            .collect();
        self.folder.write_all(&files).map_err(SaveRefused::Io)?;
        Ok(merged)
    }

    fn path(&self, scope: &SettingsScope, profiles: &[ProfileName]) -> Option<PathBuf> {
        match scope {
            SettingsScope::Global => Some(self.folder.settings()),
            SettingsScope::Profile(name) => profiles
                .iter()
                .find(|p| p.as_str() == name)
                .map(|p| self.folder.profile_settings(p)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::data_folder::tests::scratch;
    use std::fs;

    fn change(scope: SettingsScope, key: &str, value: Option<&str>) -> SettingChange {
        SettingChange {
            scope,
            key: key.into(),
            value: value.map(Into::into),
        }
    }

    #[test]
    fn writes_every_file_or_none() {
        let folder = scratch("settings");
        let acme = ProfileName::parse("AcmeCorp").unwrap();
        fs::create_dir_all(folder.profile(&acme)).unwrap();
        fs::write(folder.settings(), "# global\nmax_ttl = 300 # cap\n").unwrap();
        fs::write(folder.profile_settings(&acme), "connect_at_boot=false\n").unwrap();
        let files = SettingsFiles::new(folder.clone(), false);
        let profiles = [acme.clone()];
        let scope = SettingsScope::Profile("AcmeCorp".into());

        let bad = [
            change(SettingsScope::Global, "max_ttl", Some("60")),
            change(scope.clone(), "mtu", Some("9000")),
        ];
        match files.save(&bad, &profiles) {
            Err(SaveRefused::Problems(problems)) => {
                assert_eq!(problems[0].line, 2);
                assert_eq!(problems[0].suggest.as_deref(), Some("1420"));
            }
            other => panic!("{other:?}"),
        }
        assert_eq!(
            files.global_text().unwrap(),
            "# global\nmax_ttl = 300 # cap\n"
        );

        let good = [
            change(SettingsScope::Global, "max_ttl", Some("60")),
            change(scope, "mtu", Some("1380")),
        ];
        assert_eq!(files.save(&good, &profiles).unwrap().len(), 2);
        assert_eq!(
            files.global_text().unwrap(),
            "# global\nmax_ttl = 60 # cap\n"
        );
        assert_eq!(files.profile(&acme).unwrap().unwrap().mtu, Some(1380));
        assert_eq!(files.global().unwrap().unwrap().max_ttl.as_secs(), 60);

        let unknown = [change(SettingsScope::Profile("Ghost".into()), "mtu", None)];
        assert!(matches!(
            files.save(&unknown, &profiles),
            Err(SaveRefused::Problems(_))
        ));
        fs::remove_dir_all(folder.root()).unwrap();
    }

    #[test]
    fn a_missing_global_file_gives_the_defaults() {
        let folder = scratch("settings-default");
        let files = SettingsFiles::new(folder.clone(), false);
        assert_eq!(files.global().unwrap().unwrap(), Settings::default());
        fs::remove_dir_all(folder.root()).unwrap();
    }
}
