use std::ffi::{CStr, c_void};
use std::io;

use super::layout::WireGuardInterface;
use crate::platform::abi::{Bool, Guid};
use crate::platform::library::{AppLibrary, Library};

pub type AdapterHandle = *mut c_void;
/// `WIREGUARD_LOGGER_CALLBACK`: level, 100 ns intervals since 1601, the message.
pub type LoggerCallback =
    unsafe extern "system" fn(level: i32, timestamp: u64, message: *const u16);

/// The `wireguard.dll` function table, resolved once from the DLL beside the executable. Every
/// function `wireguard.h` declares is resolved, so a DLL missing one is refused at load.
pub struct Api {
    pub create_adapter: unsafe extern "system" fn(
        name: *const u16,
        tunnel_type: *const u16,
        requested_guid: *const Guid,
    ) -> AdapterHandle,
    pub open_adapter: unsafe extern "system" fn(name: *const u16) -> AdapterHandle,
    pub close_adapter: unsafe extern "system" fn(adapter: AdapterHandle),
    pub delete_driver: unsafe extern "system" fn() -> Bool,
    pub get_adapter_luid: unsafe extern "system" fn(adapter: AdapterHandle, luid: *mut u64),
    pub get_running_driver_version: unsafe extern "system" fn() -> u32,
    pub set_logger: unsafe extern "system" fn(logger: Option<LoggerCallback>),
    pub set_adapter_logging: unsafe extern "system" fn(adapter: AdapterHandle, state: i32) -> Bool,
    pub set_adapter_state: unsafe extern "system" fn(adapter: AdapterHandle, state: i32) -> Bool,
    pub get_adapter_state:
        unsafe extern "system" fn(adapter: AdapterHandle, state: *mut i32) -> Bool,
    pub set_configuration: unsafe extern "system" fn(
        adapter: AdapterHandle,
        config: *const WireGuardInterface,
        bytes: u32,
    ) -> Bool,
    pub get_configuration: unsafe extern "system" fn(
        adapter: AdapterHandle,
        config: *mut WireGuardInterface,
        bytes: *mut u32,
    ) -> Bool,
    // Declared last, so it is dropped after the pointers into it.
    _library: Library,
}

impl Api {
    pub const DLL: &'static str = "wireguard.dll";
    /// The vendored DLL's SHA-256, as listed in `vendor/wireguard-nt/README.md` (a test keeps
    /// them equal).
    pub const DLL_SHA256: &'static str =
        "B1B85E072C45D81358BE29D94C599DC76652F912BE8C0F0A41E2D5D89A6461D3";

    /// Loads the pinned DLL from the application folder and resolves its functions; nothing in
    /// the DLL is called.
    pub fn load(folder: &AppLibrary) -> io::Result<Self> {
        Self::resolve(folder.load(Self::DLL, Self::DLL_SHA256)?)
    }

    fn resolve(library: Library) -> io::Result<Self> {
        unsafe {
            Ok(Self {
                create_adapter: symbol(&library, c"WireGuardCreateAdapter")?,
                open_adapter: symbol(&library, c"WireGuardOpenAdapter")?,
                close_adapter: symbol(&library, c"WireGuardCloseAdapter")?,
                delete_driver: symbol(&library, c"WireGuardDeleteDriver")?,
                get_adapter_luid: symbol(&library, c"WireGuardGetAdapterLUID")?,
                get_running_driver_version: symbol(&library, c"WireGuardGetRunningDriverVersion")?,
                set_logger: symbol(&library, c"WireGuardSetLogger")?,
                set_adapter_logging: symbol(&library, c"WireGuardSetAdapterLogging")?,
                set_adapter_state: symbol(&library, c"WireGuardSetAdapterState")?,
                get_adapter_state: symbol(&library, c"WireGuardGetAdapterState")?,
                set_configuration: symbol(&library, c"WireGuardSetConfiguration")?,
                get_configuration: symbol(&library, c"WireGuardGetConfiguration")?,
                _library: library,
            })
        }
    }
}

/// The export `name` as the function pointer type `F`, which the caller matches to `wireguard.h`.
unsafe fn symbol<F: Copy>(library: &Library, name: &CStr) -> io::Result<F> {
    const { assert!(size_of::<F>() == size_of::<*mut c_void>()) };
    let address = library.symbol(name)?.as_ptr();
    Ok(unsafe { std::mem::transmute_copy::<*mut c_void, F>(&address) })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::platform::library::tests::TestFolder;
    use crate::platform::sha256::Sha256;

    const VENDORED: &[u8] = include_bytes!("../../../vendor/wireguard-nt/wireguard.dll");

    #[test]
    fn the_pinned_hash_is_the_vendored_one() {
        let readme = include_str!("../../../vendor/wireguard-nt/README.md");
        let row = readme
            .lines()
            .find(|line| line.starts_with("| wireguard.dll |"))
            .unwrap();
        assert!(
            row.trim_end()
                .ends_with(&format!("| {} |", Api::DLL_SHA256))
        );
        assert_eq!(Sha256::hex(VENDORED).unwrap(), Api::DLL_SHA256);
    }

    /// The real DLL, loaded from a scratch folder: every symbol resolves, and no function in it
    /// is ever called (the safety rules forbid touching the driver).
    #[test]
    fn loads_from_a_test_folder_and_resolves_every_symbol_without_calling_any() {
        let folder = TestFolder::new("wireguard", &[(Api::DLL, VENDORED)]);
        let api = Api::load(&AppLibrary::new(folder.0.clone()).unwrap()).unwrap();
        let addresses = [
            api.create_adapter as usize,
            api.open_adapter as usize,
            api.close_adapter as usize,
            api.delete_driver as usize,
            api.get_adapter_luid as usize,
            api.get_running_driver_version as usize,
            api.set_logger as usize,
            api.set_adapter_logging as usize,
            api.set_adapter_state as usize,
            api.get_adapter_state as usize,
            api.set_configuration as usize,
            api.get_configuration as usize,
        ];
        assert!(addresses.iter().all(|address| *address != 0));
        let mut distinct = addresses.to_vec();
        distinct.sort_unstable();
        distinct.dedup();
        assert_eq!(distinct.len(), 12);
        drop(api);
    }

    #[test]
    fn a_dll_with_another_hash_is_refused_before_loading() {
        let folder = TestFolder::new("wireguard-tampered", &[(Api::DLL, b"MZ tampered")]);
        let error = Api::load(&AppLibrary::new(folder.0.clone()).unwrap())
            .err()
            .unwrap();
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
    }
}
