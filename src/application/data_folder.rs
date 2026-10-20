use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::config::{ProfileIndex, ProfileSettings, Settings, StateFile};
use crate::platform::atomic_file::AtomicFile;
use crate::profile::ProfileName;

/// Who may open the data folder: SYSTEM and Administrators only, `logs\` readable by interactive
/// users (an ACL, set at every start). A dry run leaves it as it is.
pub trait FolderAccess: Send + Sync {
    fn restrict(&self, root: &Path) -> io::Result<()>;
}

/// `%ProgramData%\SplitWire` (or `--dir`): every path SplitWire keeps there, and atomic writes.
/// A profile's paths are built only from a validated `ProfileName`.
#[derive(Clone, Debug)]
pub struct DataFolder {
    root: PathBuf,
}

impl DataFolder {
    pub const RUNNING_MARKER: &'static str = "splitwire.running";
    pub const BLOB: &'static str = "profile.dat";
    pub const LIST: &'static str = "routes.txt";

    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }

    pub fn default_root() -> PathBuf {
        let data = std::env::var_os("ProgramData").unwrap_or_else(|| r"C:\ProgramData".into());
        Path::new(&data).join("SplitWire")
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn settings(&self) -> PathBuf {
        self.root.join(Settings::FILE)
    }

    pub fn state(&self) -> PathBuf {
        self.root.join(StateFile::FILE)
    }

    pub fn running_marker(&self) -> PathBuf {
        self.root.join(Self::RUNNING_MARKER)
    }

    pub fn logs(&self) -> PathBuf {
        self.root.join("logs")
    }

    pub fn profiles(&self) -> PathBuf {
        self.root.join("profiles")
    }

    pub fn index(&self) -> PathBuf {
        self.profiles().join(ProfileIndex::FILE)
    }

    pub fn profile(&self, name: &ProfileName) -> PathBuf {
        self.profiles().join(name.as_str())
    }

    pub fn blob(&self, name: &ProfileName) -> PathBuf {
        self.profile(name).join(Self::BLOB)
    }

    pub fn profile_settings(&self, name: &ProfileName) -> PathBuf {
        self.profile(name).join(ProfileSettings::FILE)
    }

    pub fn list(&self, name: &ProfileName) -> PathBuf {
        self.profile(name).join(Self::LIST)
    }

    /// The folders under `profiles\` with a valid name and a blob, sorted ignoring case.
    pub fn profile_folders(&self) -> io::Result<Vec<ProfileName>> {
        let entries = match fs::read_dir(self.profiles()) {
            Ok(entries) => entries,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(error) => return Err(error),
        };
        let mut names = Vec::new();
        for entry in entries {
            let entry = entry?;
            let name = entry.file_name();
            if let Some(name) = name.to_str().and_then(|n| ProfileName::parse(n).ok())
                && entry.file_type()?.is_dir()
                && self.blob(&name).is_file()
            {
                names.push(name);
            }
        }
        names.sort_by_key(|name| name.as_str().to_ascii_lowercase());
        Ok(names)
    }

    /// A text file, or an empty text when it does not exist.
    pub fn read_text(&self, path: &Path) -> io::Result<String> {
        match fs::read_to_string(path) {
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(String::new()),
            other => other,
        }
    }

    pub fn write(&self, path: &Path, bytes: &[u8]) -> io::Result<()> {
        AtomicFile::write(path, bytes)
    }

    /// Writes all files or none of them.
    pub fn write_all(&self, files: &[(PathBuf, Vec<u8>)]) -> io::Result<()> {
        AtomicFile::write_all(files)
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// A fresh scratch data folder under the temp folder.
    pub fn scratch(name: &str) -> DataFolder {
        let root = std::env::temp_dir().join(format!("splitwire-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        DataFolder::new(root)
    }

    #[test]
    fn names_every_file() {
        let folder = DataFolder::new(PathBuf::from(r"C:\ProgramData\SplitWire"));
        let acme = ProfileName::parse("AcmeCorp").unwrap();
        assert_eq!(
            folder.settings(),
            Path::new(r"C:\ProgramData\SplitWire\splitwire.conf")
        );
        assert_eq!(
            folder.state(),
            Path::new(r"C:\ProgramData\SplitWire\splitwire.state")
        );
        assert_eq!(
            folder.index(),
            Path::new(r"C:\ProgramData\SplitWire\profiles\index.conf")
        );
        assert_eq!(
            folder.blob(&acme),
            Path::new(r"C:\ProgramData\SplitWire\profiles\AcmeCorp\profile.dat")
        );
        assert_eq!(
            folder.profile_settings(&acme),
            Path::new(r"C:\ProgramData\SplitWire\profiles\AcmeCorp\profile.conf")
        );
        assert_eq!(
            folder.list(&acme),
            Path::new(r"C:\ProgramData\SplitWire\profiles\AcmeCorp\routes.txt")
        );
        assert_eq!(folder.logs(), Path::new(r"C:\ProgramData\SplitWire\logs"));
    }

    #[test]
    fn finds_profile_folders_with_a_blob_and_a_valid_name() {
        let folder = scratch("folders");
        assert!(folder.profile_folders().unwrap().is_empty());
        for name in ["beta", "Alpha", "bad name", "NoBlob"] {
            fs::create_dir_all(folder.profiles().join(name)).unwrap();
            if name != "NoBlob" {
                fs::write(folder.profiles().join(name).join("profile.dat"), b"x").unwrap();
            }
        }
        let names: Vec<String> = folder
            .profile_folders()
            .unwrap()
            .iter()
            .map(ToString::to_string)
            .collect();
        assert_eq!(names, ["Alpha", "beta"]);
        assert_eq!(folder.read_text(&folder.settings()).unwrap(), "");
        fs::remove_dir_all(folder.root()).unwrap();
    }
}
