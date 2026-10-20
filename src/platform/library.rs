//! Loading a DLL that ships beside the executable: by full path, from that folder only, after
//! its SHA-256 matched the pinned value. A planted DLL elsewhere on the search path
//! is never picked up, and a replaced one is refused.

use std::ffi::{CStr, c_void};
use std::fs::OpenOptions;
use std::io::{self, Read};
use std::os::windows::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};
use std::ptr::NonNull;

use super::abi::{
    FreeLibrary, GetProcAddress, LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR, LOAD_LIBRARY_SEARCH_SYSTEM32,
    LoadLibraryExW, Raw, SetDefaultDllDirectories, check_bool, last_error, wide_os,
};
use super::sha256::Sha256;

const FILE_SHARE_READ: u32 = 1;

/// The folder DLLs may be loaded from: the executable's own (admin-only once installed).
pub struct AppLibrary {
    folder: PathBuf,
}

impl AppLibrary {
    /// The folder of the running executable.
    pub fn beside_exe() -> io::Result<Self> {
        let exe = std::env::current_exe()?;
        let folder = exe
            .parent()
            .ok_or_else(|| io::Error::other("the executable has no folder"))?;
        Self::new(folder.to_path_buf())
    }

    /// `folder` must be absolute: `LoadLibraryExW` takes the full path.
    pub fn new(folder: PathBuf) -> io::Result<Self> {
        if !folder.is_absolute() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("{} is not an absolute folder", folder.display()),
            ));
        }
        Ok(Self { folder })
    }

    /// Restricts the process's default DLL search to System32, so no later implicit load looks
    /// in the current or the PATH folders. Called once at process start.
    pub fn restrict_default_search() -> io::Result<()> {
        check_bool(unsafe { SetDefaultDllDirectories(LOAD_LIBRARY_SEARCH_SYSTEM32) })
    }

    pub fn folder(&self) -> &Path {
        &self.folder
    }

    /// Loads `file_name` from the folder when its SHA-256 is `sha256_hex`. Anything but a plain
    /// file name is refused before any call. The file stays open without write sharing from the
    /// hash to the load, so it cannot be swapped in between.
    pub fn load(&self, file_name: &str, sha256_hex: &str) -> io::Result<Library> {
        let path = self.path_of(file_name)?;
        let mut file = OpenOptions::new()
            .read(true)
            .share_mode(FILE_SHARE_READ)
            .open(&path)?;
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes)?;
        let actual = Sha256::hex(&bytes)?;
        if !actual.eq_ignore_ascii_case(sha256_hex) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "{} has SHA-256 {actual}, expected {sha256_hex}",
                    path.display()
                ),
            ));
        }
        let module = unsafe {
            LoadLibraryExW(
                wide_os(path.as_os_str()).as_ptr(),
                std::ptr::null_mut(),
                LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR | LOAD_LIBRARY_SEARCH_SYSTEM32,
            )
        };
        drop(file);
        match NonNull::new(module) {
            Some(module) => Ok(Library { module }),
            None => Err(last_error()),
        }
    }

    fn path_of(&self, file_name: &str) -> io::Result<PathBuf> {
        let plain = !file_name.is_empty()
            && file_name != "."
            && file_name != ".."
            && !file_name.contains(['\\', '/', ':']);
        if !plain {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "'{file_name}' is not a file name in {}",
                    self.folder.display()
                ),
            ));
        }
        Ok(self.folder.join(file_name))
    }
}

/// A loaded module; freed on drop. The service keeps its one library for the process lifetime.
pub struct Library {
    module: NonNull<c_void>,
}

impl Library {
    /// The address of an exported function; resolving calls nothing in the DLL.
    pub fn symbol(&self, name: &CStr) -> io::Result<NonNull<c_void>> {
        let address: Raw = unsafe { GetProcAddress(self.module.as_ptr(), name.as_ptr().cast()) };
        NonNull::new(address).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                format!("the DLL does not export {}", name.to_string_lossy()),
            )
        })
    }
}

impl Drop for Library {
    fn drop(&mut self) {
        unsafe {
            FreeLibrary(self.module.as_ptr());
        }
    }
}

// A module handle is process-wide; Library owns its reference count.
unsafe impl Send for Library {}
unsafe impl Sync for Library {}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use std::fs;

    /// A scratch "executable folder" holding copies of the given files; removed on drop.
    pub(crate) struct TestFolder(pub PathBuf);

    impl TestFolder {
        pub(crate) fn new(name: &str, files: &[(&str, &[u8])]) -> Self {
            let path =
                std::env::temp_dir().join(format!("SplitWire.Test.{}.{name}", std::process::id()));
            fs::create_dir_all(&path).unwrap();
            for (file, bytes) in files {
                fs::write(path.join(file), bytes).unwrap();
            }
            Self(path)
        }
    }

    impl Drop for TestFolder {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn refuses_anything_but_a_file_name_before_any_call() {
        let library = AppLibrary::new(std::env::temp_dir()).unwrap();
        for name in [
            "",
            ".",
            "..",
            "..\\wireguard.dll",
            "sub/wireguard.dll",
            "C:\\Windows\\System32\\kernel32.dll",
            "C:kernel32.dll",
            "\\\\server\\share\\wireguard.dll",
        ] {
            let error = library.load(name, "00").err().unwrap();
            assert_eq!(error.kind(), io::ErrorKind::InvalidInput, "{name}");
        }
        assert!(AppLibrary::new(PathBuf::from("relative")).is_err());
    }

    #[test]
    fn refuses_a_file_whose_hash_differs_and_a_missing_one() {
        let folder = TestFolder::new("hash", &[("other.dll", b"not the pinned bytes")]);
        let library = AppLibrary::new(folder.0.clone()).unwrap();
        let error = library.load("other.dll", &"0".repeat(64)).err().unwrap();
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        assert!(error.to_string().contains("expected"));
        let missing = library.load("missing.dll", &"0".repeat(64)).err().unwrap();
        assert_eq!(missing.kind(), io::ErrorKind::NotFound);
    }

    #[test]
    fn the_default_search_can_be_restricted() {
        AppLibrary::restrict_default_search().unwrap();
        assert!(AppLibrary::beside_exe().unwrap().folder().is_absolute());
    }
}
