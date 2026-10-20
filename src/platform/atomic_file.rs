use std::ffi::OsString;
use std::fs::{self, File};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use super::abi::{
    MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH, MoveFileExW, check_bool, wide_os,
};

/// Files replaced whole: readers see the old or the new content, never a part.
pub struct AtomicFile;

impl AtomicFile {
    /// Writes `<path>.tmp`, flushes it to the disk, then moves it over `path`.
    pub fn write(path: &Path, bytes: &[u8]) -> io::Result<()> {
        let temporary = Self::temporary(path);
        Self::write_temporary(&temporary, bytes)?;
        Self::replace(&temporary, path)
    }

    /// Writes every `.tmp` first and replaces the files only when all were written, so a full disk
    /// or a denied folder changes none of them.
    pub fn write_all(files: &[(PathBuf, Vec<u8>)]) -> io::Result<()> {
        let mut written = Vec::new();
        for (path, bytes) in files {
            let temporary = Self::temporary(path);
            if let Err(error) = Self::write_temporary(&temporary, bytes) {
                for done in written.iter().chain([&temporary]) {
                    let _ = fs::remove_file(done);
                }
                return Err(error);
            }
            written.push(temporary);
        }
        for ((path, _), temporary) in files.iter().zip(&written) {
            Self::replace(temporary, path)?;
        }
        Ok(())
    }

    fn temporary(path: &Path) -> PathBuf {
        let mut name = OsString::from(path.as_os_str());
        name.push(".tmp");
        PathBuf::from(name)
    }

    fn write_temporary(temporary: &Path, bytes: &[u8]) -> io::Result<()> {
        let mut file = File::create(temporary)?;
        file.write_all(bytes)?;
        file.sync_all()
    }

    fn replace(temporary: &Path, path: &Path) -> io::Result<()> {
        let from = wide_os(temporary.as_os_str());
        let to = wide_os(path.as_os_str());
        check_bool(unsafe {
            MoveFileExW(
                from.as_ptr(),
                to.as_ptr(),
                MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
            )
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let folder =
            std::env::temp_dir().join(format!("splitwire-atomic-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&folder);
        fs::create_dir_all(&folder).unwrap();
        folder
    }

    #[test]
    fn replaces_a_file_and_leaves_no_temporary() {
        let folder = scratch("one");
        let path = folder.join("splitwire.conf");
        AtomicFile::write(&path, b"max_ttl=300\n").unwrap();
        AtomicFile::write(&path, b"max_ttl=60\n").unwrap();
        assert_eq!(fs::read(&path).unwrap(), b"max_ttl=60\n");
        assert!(!folder.join("splitwire.conf.tmp").exists());
        fs::remove_dir_all(folder).unwrap();
    }

    #[test]
    fn writes_all_or_none() {
        let folder = scratch("all");
        let first = folder.join("a.conf");
        fs::write(&first, b"old").unwrap();
        let missing = folder.join("no-such-folder").join("b.conf");
        let files = [(first.clone(), b"new".to_vec()), (missing, b"new".to_vec())];
        assert!(AtomicFile::write_all(&files).is_err());
        assert_eq!(fs::read(&first).unwrap(), b"old");
        assert!(!folder.join("a.conf.tmp").exists());
        let second = folder.join("b.conf");
        AtomicFile::write_all(&[
            (first.clone(), b"1".to_vec()),
            (second.clone(), b"2".to_vec()),
        ])
        .unwrap();
        assert_eq!(fs::read(&first).unwrap(), b"1");
        assert_eq!(fs::read(&second).unwrap(), b"2");
        fs::remove_dir_all(folder).unwrap();
    }
}
