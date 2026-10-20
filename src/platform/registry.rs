//! Registry keys over the hand-written ABI: open, create, enumerate, read and write values, and
//! delete a subtree. Only what NRPT, the Group Policy probe, the interface DNS lookup and the
//! window's Run value need.

use std::io;
use std::ptr::{null, null_mut};

use super::abi::*;

/// A predefined root key.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RegistryRoot {
    CurrentUser,
    LocalMachine,
}

impl RegistryRoot {
    fn raw(self) -> Raw {
        match self {
            RegistryRoot::CurrentUser => HKEY_CURRENT_USER,
            RegistryRoot::LocalMachine => HKEY_LOCAL_MACHINE,
        }
    }
}

fn status(code: i32) -> io::Result<()> {
    match code {
        0 => Ok(()),
        code => Err(io::Error::from_raw_os_error(code)),
    }
}

/// An open key, closed on drop.
pub struct RegistryKey(Raw);

// A registry handle is a process-wide kernel handle; the key owns its lifetime.
unsafe impl Send for RegistryKey {}
unsafe impl Sync for RegistryKey {}

impl RegistryKey {
    /// `root\path`, `None` when it does not exist.
    pub fn open(root: RegistryRoot, path: &str, write: bool) -> io::Result<Option<Self>> {
        Self::open_under(root.raw(), path, write)
    }

    /// `root\path`, created with its parents when missing.
    pub fn create(root: RegistryRoot, path: &str) -> io::Result<Self> {
        Self::create_under(root.raw(), path)
    }

    pub fn open_child(&self, name: &str, write: bool) -> io::Result<Option<Self>> {
        Self::open_under(self.0, name, write)
    }

    pub fn create_child(&self, name: &str) -> io::Result<Self> {
        Self::create_under(self.0, name)
    }

    /// The names of the direct subkeys.
    pub fn subkeys(&self) -> io::Result<Vec<String>> {
        let mut names = Vec::new();
        for index in 0.. {
            // Key names are at most 255 characters.
            let mut buffer = [0u16; 256];
            let mut len = buffer.len() as u32;
            let code = unsafe {
                RegEnumKeyExW(
                    self.0,
                    index,
                    buffer.as_mut_ptr(),
                    &mut len,
                    null_mut(),
                    null_mut(),
                    null_mut(),
                    null_mut(),
                )
            };
            match code {
                ERROR_NO_MORE_ITEMS => break,
                code => status(code)?,
            }
            names.push(String::from_utf16_lossy(&buffer[..len as usize]));
        }
        Ok(names)
    }

    /// A `REG_DWORD` value, `None` when it is missing.
    pub fn dword(&self, name: &str) -> io::Result<Option<u32>> {
        let name = wide(name);
        let mut value = 0u32;
        let mut size = 4u32;
        let code = unsafe {
            RegGetValueW(
                self.0,
                null(),
                name.as_ptr(),
                RRF_RT_REG_DWORD,
                null_mut(),
                (&mut value as *mut u32).cast(),
                &mut size,
            )
        };
        match code {
            ERROR_FILE_NOT_FOUND => Ok(None),
            code => status(code).map(|()| Some(value)),
        }
    }

    /// A `REG_SZ` value, `None` when it is missing.
    pub fn string(&self, name: &str) -> io::Result<Option<String>> {
        let name = wide(name);
        let mut buffer = vec![0u16; 256];
        loop {
            let mut size = (buffer.len() * 2) as u32;
            let code = unsafe {
                RegGetValueW(
                    self.0,
                    null(),
                    name.as_ptr(),
                    RRF_RT_REG_SZ,
                    null_mut(),
                    buffer.as_mut_ptr().cast(),
                    &mut size,
                )
            };
            match code {
                ERROR_FILE_NOT_FOUND => return Ok(None),
                ERROR_MORE_DATA => buffer.resize(size as usize / 2 + 1, 0),
                code => {
                    status(code)?;
                    let len = buffer.iter().position(|c| *c == 0).unwrap_or(buffer.len());
                    return Ok(Some(String::from_utf16_lossy(&buffer[..len])));
                }
            }
        }
    }

    pub fn set_dword(&self, name: &str, value: u32) -> io::Result<()> {
        self.set(name, REG_DWORD, &value.to_le_bytes())
    }

    pub fn set_string(&self, name: &str, value: &str) -> io::Result<()> {
        self.set(name, REG_SZ, &Self::utf16_bytes(&wide(value)))
    }

    /// `REG_MULTI_SZ`: each string ends with a NUL, and the list with one more.
    pub fn set_strings(&self, name: &str, values: &[&str]) -> io::Result<()> {
        let mut units: Vec<u16> = values.iter().flat_map(|v| wide(v)).collect();
        units.push(0);
        self.set(name, REG_MULTI_SZ, &Self::utf16_bytes(&units))
    }

    /// Deletes the subkey `name` with everything under it; a missing one is already gone.
    pub fn delete_tree(&self, name: &str) -> io::Result<()> {
        let name = wide(name);
        match unsafe { RegDeleteTreeW(self.0, name.as_ptr()) } {
            ERROR_FILE_NOT_FOUND => Ok(()),
            code => status(code),
        }
    }

    /// Deletes the value `name`; a missing one is already gone.
    pub fn delete_value(&self, name: &str) -> io::Result<()> {
        let name = wide(name);
        match unsafe { RegDeleteValueW(self.0, name.as_ptr()) } {
            ERROR_FILE_NOT_FOUND => Ok(()),
            code => status(code),
        }
    }

    fn set(&self, name: &str, kind: u32, data: &[u8]) -> io::Result<()> {
        let name = wide(name);
        status(unsafe {
            RegSetValueExW(
                self.0,
                name.as_ptr(),
                0,
                kind,
                data.as_ptr(),
                data.len() as u32,
            )
        })
    }

    fn utf16_bytes(units: &[u16]) -> Vec<u8> {
        units.iter().flat_map(|u| u.to_le_bytes()).collect()
    }

    fn open_under(parent: Raw, path: &str, write: bool) -> io::Result<Option<Self>> {
        let path = wide(path);
        let access = if write {
            KEY_READ | KEY_WRITE
        } else {
            KEY_READ
        };
        let mut key = null_mut();
        match unsafe { RegOpenKeyExW(parent, path.as_ptr(), 0, access, &mut key) } {
            ERROR_FILE_NOT_FOUND => Ok(None),
            code => status(code).map(|()| Some(Self(key))),
        }
    }

    fn create_under(parent: Raw, path: &str) -> io::Result<Self> {
        let path = wide(path);
        let mut key = null_mut();
        status(unsafe {
            RegCreateKeyExW(
                parent,
                path.as_ptr(),
                0,
                null(),
                0,
                KEY_READ | KEY_WRITE,
                null_mut(),
                &mut key,
                null_mut(),
            )
        })?;
        Ok(Self(key))
    }
}

impl Drop for RegistryKey {
    fn drop(&mut self) {
        unsafe {
            RegCloseKey(self.0);
        }
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// Tests write only under `HKCU\Software\SplitWire-test`, each in a key of its own that is
    /// deleted afterwards.
    pub(crate) struct ScratchKey {
        pub(crate) path: String,
    }

    impl ScratchKey {
        pub(crate) const PARENT: &'static str = r"Software\SplitWire-test";

        pub(crate) fn new(name: &str) -> Self {
            let path = format!(r"{}\{name}-{}", Self::PARENT, std::process::id());
            let parent = RegistryKey::create(RegistryRoot::CurrentUser, Self::PARENT).unwrap();
            let leaf = path.rsplit('\\').next().unwrap();
            let _ = parent.delete_tree(leaf);
            RegistryKey::create(RegistryRoot::CurrentUser, &path).unwrap();
            Self { path }
        }
    }

    impl Drop for ScratchKey {
        fn drop(&mut self) {
            if let Ok(Some(parent)) =
                RegistryKey::open(RegistryRoot::CurrentUser, Self::PARENT, true)
            {
                let _ = parent.delete_tree(self.path.rsplit('\\').next().unwrap());
            }
        }
    }

    #[test]
    fn writes_reads_lists_and_deletes_under_a_scratch_key() {
        let scratch = ScratchKey::new("registry");
        let key = RegistryKey::open(RegistryRoot::CurrentUser, &scratch.path, true)
            .unwrap()
            .unwrap();
        let child = key.create_child("Child").unwrap();
        child.set_string("Comment", "SplitWire").unwrap();
        child.set_dword("Version", 2).unwrap();
        child
            .set_strings("Name", &[".a.example", "a.example"])
            .unwrap();
        assert_eq!(
            child.string("Comment").unwrap().as_deref(),
            Some("SplitWire")
        );
        assert_eq!(child.string("Missing").unwrap(), None);
        assert_eq!(child.dword("Version").unwrap(), Some(2));
        assert_eq!(child.dword("Missing").unwrap(), None);
        child.delete_value("Comment").unwrap();
        assert_eq!(child.dword("Version").unwrap(), Some(2));
        assert_eq!(child.dword("Missing").unwrap(), None);
        child.delete_value("Comment").unwrap();
        assert_eq!(child.string("Comment").unwrap(), None);
        assert_eq!(key.subkeys().unwrap(), ["Child"]);
        drop(child);
        key.delete_tree("Child").unwrap();
        assert!(key.subkeys().unwrap().is_empty());
        assert!(key.open_child("Child", false).unwrap().is_none());
        assert!(
            RegistryKey::open(
                RegistryRoot::CurrentUser,
                r"Software\SplitWire-test\nope",
                false
            )
            .unwrap()
            .is_none()
        );
    }
}
