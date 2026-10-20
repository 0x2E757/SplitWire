//! The Windows ABI SplitWire calls, written by hand from the SDK headers: the shared types,
//! handles and strings, and the `extern "system"` declarations. Layouts the code relies on are
//! asserted by tests whose `// C name` comments `tools/verify-abi.ps1` compares with the SDK.

use std::ffi::{OsStr, c_void};
use std::io;
use std::os::windows::ffi::OsStrExt;

pub type Raw = *mut c_void;
pub type Bool = i32;

pub const INVALID_HANDLE_VALUE: Raw = -1isize as Raw;
pub const WAIT_OBJECT_0: u32 = 0;
pub const WAIT_TIMEOUT: u32 = 258;
pub const LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR: u32 = 0x100;
pub const LOAD_LIBRARY_SEARCH_SYSTEM32: u32 = 0x800;

/// C `GUID`.
#[repr(C)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Guid {
    pub data1: u32,
    pub data2: u16,
    pub data3: u16,
    pub data4: [u8; 8],
}

impl Guid {
    /// `0x6b29fc40_ca47_1067_b31d_00dd010662da` is `{6B29FC40-CA47-1067-B31D-00DD010662DA}`.
    pub const fn from_u128(value: u128) -> Self {
        Self {
            data1: (value >> 96) as u32,
            data2: (value >> 80) as u16,
            data3: (value >> 64) as u16,
            data4: (value as u64).to_be_bytes(),
        }
    }

    pub const fn to_u128(self) -> u128 {
        ((self.data1 as u128) << 96)
            | ((self.data2 as u128) << 80)
            | ((self.data3 as u128) << 64)
            | u64::from_be_bytes(self.data4) as u128
    }
}

/// A NUL-terminated UTF-16 string for a `PCWSTR` parameter.
pub fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(Some(0)).collect()
}

/// A NUL-terminated UTF-16 path, without a lossy round trip through UTF-8.
pub fn wide_os(value: &OsStr) -> Vec<u16> {
    value.encode_wide().chain(Some(0)).collect()
}

/// The calling thread's last Win32 error.
pub fn last_error() -> io::Error {
    io::Error::last_os_error()
}

/// A `BOOL` result: `Ok` when nonzero, else the last error.
pub fn check_bool(result: Bool) -> io::Result<()> {
    if result == 0 {
        Err(last_error())
    } else {
        Ok(())
    }
}

/// An owned kernel handle, closed on drop.
pub struct Handle(Raw);

impl Handle {
    /// Takes ownership of `raw`; null and `INVALID_HANDLE_VALUE` are the last error.
    pub fn new(raw: Raw) -> io::Result<Self> {
        if raw.is_null() || raw == INVALID_HANDLE_VALUE {
            Err(last_error())
        } else {
            Ok(Self(raw))
        }
    }

    pub fn raw(&self) -> Raw {
        self.0
    }
}

impl Drop for Handle {
    fn drop(&mut self) {
        unsafe {
            CloseHandle(self.0);
        }
    }
}

// Kernel handles are process-wide, not apartment objects; Handle owns the lifetime.
unsafe impl Send for Handle {}
unsafe impl Sync for Handle {}

#[link(name = "kernel32")]
unsafe extern "system" {
    pub fn CloseHandle(handle: Raw) -> Bool;
    pub fn CreateEventW(attributes: Raw, manual: Bool, initial: Bool, name: *const u16) -> Raw;
    pub fn CreateMutexW(attributes: Raw, owner: Bool, name: *const u16) -> Raw;
    pub fn SetEvent(handle: Raw) -> Bool;
    pub fn WaitForSingleObject(handle: Raw, milliseconds: u32) -> u32;
    pub fn SetDefaultDllDirectories(flags: u32) -> Bool;
    pub fn LoadLibraryExW(path: *const u16, file: Raw, flags: u32) -> Raw;
    pub fn GetProcAddress(module: Raw, name: *const u8) -> Raw;
    pub fn FreeLibrary(module: Raw) -> Bool;
    pub fn GetModuleFileNameW(module: Raw, path: *mut u16, size: u32) -> u32;
}

// The process token (is this process elevated?) and local time.

pub const TOKEN_QUERY: u32 = 0x8;
pub const TOKEN_ELEVATION_CLASS: u32 = 20;

/// C `SYSTEMTIME`.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct SystemTimeParts {
    pub year: u16,
    pub month: u16,
    pub day_of_week: u16,
    pub day: u16,
    pub hour: u16,
    pub minute: u16,
    pub second: u16,
    pub milliseconds: u16,
}

#[link(name = "kernel32")]
unsafe extern "system" {
    pub fn GetCurrentProcess() -> Raw;
    pub fn FileTimeToSystemTime(file_time: *const u64, system_time: *mut SystemTimeParts) -> Bool;
    pub fn SystemTimeToFileTime(system_time: *const SystemTimeParts, file_time: *mut u64) -> Bool;
    pub fn SystemTimeToTzSpecificLocalTime(
        zone: Raw,
        universal: *const SystemTimeParts,
        local: *mut SystemTimeParts,
    ) -> Bool;
}

#[link(name = "advapi32")]
unsafe extern "system" {
    pub fn OpenProcessToken(process: Raw, access: u32, token: *mut Raw) -> Bool;
    pub fn GetTokenInformation(
        token: Raw,
        class: u32,
        information: *mut c_void,
        length: u32,
        returned: *mut u32,
    ) -> Bool;
}

// Files, DPAPI and security descriptors: the profile blobs and the data folder's ACL.

pub const MOVEFILE_REPLACE_EXISTING: u32 = 0x1;
pub const MOVEFILE_WRITE_THROUGH: u32 = 0x8;
pub const CRYPTPROTECT_UI_FORBIDDEN: u32 = 0x1;
pub const CRYPTPROTECT_LOCAL_MACHINE: u32 = 0x4;
pub const SDDL_REVISION_1: u32 = 1;
pub const SE_FILE_OBJECT: u32 = 1;
pub const OWNER_SECURITY_INFORMATION: u32 = 0x1;
pub const DACL_SECURITY_INFORMATION: u32 = 0x4;
pub const PROTECTED_DACL_SECURITY_INFORMATION: u32 = 0x8000_0000;

/// C `DATA_BLOB` (`CRYPT_INTEGER_BLOB`).
#[repr(C)]
pub struct DataBlob {
    pub size: u32,
    pub data: *mut u8,
}

#[link(name = "kernel32")]
unsafe extern "system" {
    pub fn LocalFree(memory: Raw) -> Raw;
    pub fn MoveFileExW(existing: *const u16, new: *const u16, flags: u32) -> Bool;
}

#[link(name = "crypt32")]
unsafe extern "system" {
    pub fn CryptProtectData(
        input: *const DataBlob,
        description: *const u16,
        entropy: *const DataBlob,
        reserved: Raw,
        prompt: Raw,
        flags: u32,
        output: *mut DataBlob,
    ) -> Bool;
    pub fn CryptUnprotectData(
        input: *const DataBlob,
        description: *mut *mut u16,
        entropy: *const DataBlob,
        reserved: Raw,
        prompt: Raw,
        flags: u32,
        output: *mut DataBlob,
    ) -> Bool;
}

#[link(name = "advapi32")]
unsafe extern "system" {
    pub fn ConvertStringSecurityDescriptorToSecurityDescriptorW(
        sddl: *const u16,
        revision: u32,
        descriptor: *mut Raw,
        size: *mut u32,
    ) -> Bool;
    pub fn ConvertSecurityDescriptorToStringSecurityDescriptorW(
        descriptor: Raw,
        revision: u32,
        information: u32,
        sddl: *mut *mut u16,
        length: *mut u32,
    ) -> Bool;
    pub fn GetSecurityDescriptorOwner(
        descriptor: Raw,
        owner: *mut Raw,
        defaulted: *mut Bool,
    ) -> Bool;
    pub fn GetSecurityDescriptorDacl(
        descriptor: Raw,
        present: *mut Bool,
        dacl: *mut Raw,
        defaulted: *mut Bool,
    ) -> Bool;
    pub fn SetNamedSecurityInfoW(
        name: *const u16,
        object: u32,
        information: u32,
        owner: Raw,
        group: Raw,
        dacl: Raw,
        sacl: Raw,
    ) -> u32;
    pub fn GetNamedSecurityInfoW(
        name: *const u16,
        object: u32,
        information: u32,
        owner: *mut Raw,
        group: *mut Raw,
        dacl: *mut Raw,
        sacl: *mut Raw,
        descriptor: *mut Raw,
    ) -> u32;
}

// The registry: NRPT rules, the Group Policy probe and the interfaces' DNS servers.

/// The predefined keys are sign-extended 32-bit values.
pub const HKEY_CURRENT_USER: Raw = 0x8000_0001u32 as i32 as isize as Raw;
pub const HKEY_LOCAL_MACHINE: Raw = 0x8000_0002u32 as i32 as isize as Raw;
pub const KEY_READ: u32 = 0x2_0019;
pub const KEY_WRITE: u32 = 0x2_0006;
pub const REG_SZ: u32 = 1;
pub const REG_DWORD: u32 = 4;
pub const REG_MULTI_SZ: u32 = 7;
pub const RRF_RT_REG_SZ: u32 = 0x2;
pub const RRF_RT_REG_DWORD: u32 = 0x10;
pub const ERROR_FILE_NOT_FOUND: i32 = 2;
pub const ERROR_MORE_DATA: i32 = 234;
pub const ERROR_NO_MORE_ITEMS: i32 = 259;

#[link(name = "advapi32")]
unsafe extern "system" {
    pub fn RegOpenKeyExW(
        key: Raw,
        subkey: *const u16,
        options: u32,
        access: u32,
        result: *mut Raw,
    ) -> i32;
    pub fn RegCreateKeyExW(
        key: Raw,
        subkey: *const u16,
        reserved: u32,
        class: *const u16,
        options: u32,
        access: u32,
        security: Raw,
        result: *mut Raw,
        disposition: *mut u32,
    ) -> i32;
    pub fn RegCloseKey(key: Raw) -> i32;
    pub fn RegSetValueExW(
        key: Raw,
        name: *const u16,
        reserved: u32,
        kind: u32,
        data: *const u8,
        size: u32,
    ) -> i32;
    pub fn RegGetValueW(
        key: Raw,
        subkey: *const u16,
        name: *const u16,
        flags: u32,
        kind: *mut u32,
        data: *mut c_void,
        size: *mut u32,
    ) -> i32;
    pub fn RegEnumKeyExW(
        key: Raw,
        index: u32,
        name: *mut u16,
        name_len: *mut u32,
        reserved: *mut u32,
        class: *mut u16,
        class_len: *mut u32,
        last_write: *mut u64,
    ) -> i32;
    pub fn RegDeleteTreeW(key: Raw, subkey: *const u16) -> i32;
    pub fn RegDeleteValueW(key: Raw, name: *const u16) -> i32;
}

// The console: Ctrl+C ends a foreground run.

pub type ConsoleHandler = unsafe extern "system" fn(event: u32) -> Bool;

#[link(name = "kernel32")]
unsafe extern "system" {
    pub fn SetConsoleCtrlHandler(handler: Option<ConsoleHandler>, add: Bool) -> Bool;
}

// The Windows DNS client cache. Undocumented but long-standing (what `ipconfig /flushdns`
// calls), and not in dnsapi.lib, so it is bound by name at load.

#[link(name = "dnsapi", kind = "raw-dylib")]
unsafe extern "system" {
    pub fn DnsFlushResolverCache() -> Bool;
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::mem::{offset_of, size_of};

    #[test]
    fn layouts_match_the_sdk() {
        assert_eq!(size_of::<Guid>(), 16); // GUID
        assert_eq!(offset_of!(Guid, data2), 4); // GUID.Data2
        assert_eq!(offset_of!(Guid, data3), 6); // GUID.Data3
        assert_eq!(offset_of!(Guid, data4), 8); // GUID.Data4
    }

    #[test]
    fn time_layouts_match_the_sdk() {
        assert_eq!(size_of::<SystemTimeParts>(), 16); // SYSTEMTIME
        assert_eq!(offset_of!(SystemTimeParts, day), 6); // SYSTEMTIME.wDay
        assert_eq!(offset_of!(SystemTimeParts, milliseconds), 14); // SYSTEMTIME.wMilliseconds
    }

    #[test]
    fn dpapi_layouts_match_the_sdk() {
        assert_eq!(size_of::<DataBlob>(), 16); // DATA_BLOB
        assert_eq!(offset_of!(DataBlob, data), 8); // DATA_BLOB.pbData
    }

    #[test]
    fn guids_round_trip_through_u128() {
        let value = 0x6b29fc40_ca47_1067_b31d_00dd010662da;
        let guid = Guid::from_u128(value);
        assert_eq!(guid.data1, 0x6b29fc40);
        assert_eq!(guid.data4, [0xb3, 0x1d, 0x00, 0xdd, 0x01, 0x06, 0x62, 0xda]);
        assert_eq!(guid.to_u128(), value);
    }

    #[test]
    fn strings_end_with_one_nul() {
        assert_eq!(wide("ab"), [0x61, 0x62, 0]);
        assert_eq!(wide_os(OsStr::new("é")), [0xe9, 0]);
    }

    #[test]
    fn an_owned_event_is_signalled_and_closed() {
        let event =
            Handle::new(unsafe { CreateEventW(std::ptr::null_mut(), 1, 0, std::ptr::null()) })
                .unwrap();
        assert_eq!(unsafe { WaitForSingleObject(event.raw(), 0) }, WAIT_TIMEOUT);
        check_bool(unsafe { SetEvent(event.raw()) }).unwrap();
        assert_eq!(
            unsafe { WaitForSingleObject(event.raw(), 0) },
            WAIT_OBJECT_0
        );
    }

    #[test]
    fn null_and_invalid_handles_are_errors() {
        assert!(Handle::new(std::ptr::null_mut()).is_err());
        assert!(Handle::new(INVALID_HANDLE_VALUE).is_err());
        assert!(check_bool(0).is_err());
    }
}
