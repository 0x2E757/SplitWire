use std::io;
use std::ptr::NonNull;
use std::sync::{Arc, Mutex};

use super::api::{AdapterHandle, Api};
use super::config_buffer::ConfigBuffer;
use super::driver_log::DriverLog;
use super::layout::*;
use crate::logs::Log;
use crate::platform::abi::{Guid, check_bool, last_error, wide};
use crate::platform::library::AppLibrary;
use crate::tunnel::{
    AdapterState, DriverConfig, DriverVersion, TunnelHealth, WireGuardAdapter, WireGuardDriver,
};

const ERROR_FILE_NOT_FOUND: i32 = 2;
const ERROR_MORE_DATA: i32 = 234;
const ERROR_NOT_FOUND: i32 = 1168;
/// One interface, one peer and one allowed IP: what a SplitWire adapter holds.
const FIRST_READ: usize = 80 + 136 + 24;

/// `impl WireGuardDriver` over `wireguard.dll`. The DLL is loaded at the first use (the first
/// Connect), from the application folder only, and then stays loaded with the driver log set.
pub struct WireGuardNt {
    folder: AppLibrary,
    log: Log,
    api: Mutex<Option<Arc<Api>>>,
}

impl WireGuardNt {
    pub fn new(folder: AppLibrary, log: Log) -> Self {
        Self {
            folder,
            log,
            api: Mutex::new(None),
        }
    }

    fn api(&self) -> io::Result<Arc<Api>> {
        let mut slot = self.api.lock().unwrap();
        if let Some(api) = slot.as_ref() {
            return Ok(api.clone());
        }
        let api = Arc::new(Api::load(&self.folder)?);
        DriverLog::attach(&self.log);
        unsafe { (api.set_logger)(Some(DriverLog::callback)) };
        *slot = Some(api.clone());
        Ok(api)
    }

    fn adapter(api: Arc<Api>, handle: AdapterHandle) -> io::Result<Box<dyn WireGuardAdapter>> {
        match NonNull::new(handle) {
            Some(handle) => Ok(Box::new(NtAdapter { api, handle })),
            None => Err(last_error()),
        }
    }
}

impl WireGuardDriver for WireGuardNt {
    fn create(
        &self,
        name: &str,
        tunnel_type: &str,
        guid: u128,
    ) -> io::Result<Box<dyn WireGuardAdapter>> {
        let api = self.api()?;
        let guid = Guid::from_u128(guid);
        let handle =
            unsafe { (api.create_adapter)(wide(name).as_ptr(), wide(tunnel_type).as_ptr(), &guid) };
        Self::adapter(api, handle)
    }

    fn open(&self, name: &str) -> io::Result<Option<Box<dyn WireGuardAdapter>>> {
        let api = self.api()?;
        let handle = unsafe { (api.open_adapter)(wide(name).as_ptr()) };
        if !handle.is_null() {
            return Self::adapter(api, handle).map(Some);
        }
        // Only "not found" means no such adapter; any other failure must stop a Connect rather
        // than let it create a second adapter of that name.
        let error = last_error();
        match error.raw_os_error() {
            Some(ERROR_FILE_NOT_FOUND | ERROR_NOT_FOUND) => Ok(None),
            _ => Err(error),
        }
    }

    /// Asks only a DLL the first Connect loaded: a status poll never loads it.
    fn running_version(&self) -> io::Result<Option<DriverVersion>> {
        let Some(api) = self.api.lock().unwrap().clone() else {
            return Ok(None);
        };
        let version = unsafe { (api.get_running_driver_version)() };
        if version != 0 {
            return Ok(Some(DriverVersion::from_dword(version)));
        }
        let error = last_error();
        match error.raw_os_error() {
            Some(ERROR_FILE_NOT_FOUND) => Ok(None),
            _ => Err(error),
        }
    }

    fn delete_driver(&self) -> io::Result<()> {
        let api = self.api()?;
        check_bool(unsafe { (api.delete_driver)() })
    }
}

/// An open adapter handle; closing it removes an adapter this process created.
struct NtAdapter {
    api: Arc<Api>,
    handle: NonNull<std::ffi::c_void>,
}

// The DLL's adapter functions may be called from any thread; the handle is owned here.
unsafe impl Send for NtAdapter {}

impl WireGuardAdapter for NtAdapter {
    fn luid(&self) -> u64 {
        let mut luid = 0u64;
        unsafe { (self.api.get_adapter_luid)(self.handle.as_ptr(), &mut luid) };
        luid
    }

    fn set_config(&self, config: &DriverConfig) -> io::Result<()> {
        let buffer = ConfigBuffer::for_config(config);
        let bytes = u32::try_from(buffer.len()).map_err(io::Error::other)?;
        check_bool(unsafe {
            (self.api.set_configuration)(self.handle.as_ptr(), buffer.as_ptr(), bytes)
        })
    }

    fn config(&self) -> io::Result<TunnelHealth> {
        let mut size = FIRST_READ;
        loop {
            let mut buffer = ConfigBuffer::zeroed(size);
            let mut bytes = u32::try_from(size).map_err(io::Error::other)?;
            let ok = unsafe {
                (self.api.get_configuration)(self.handle.as_ptr(), buffer.as_mut_ptr(), &mut bytes)
            };
            if ok != 0 {
                return buffer.health(bytes as usize);
            }
            let error = last_error();
            if error.raw_os_error() != Some(ERROR_MORE_DATA) || bytes as usize <= size {
                return Err(error);
            }
            size = bytes as usize;
        }
    }

    fn set_state(&self, state: AdapterState) -> io::Result<()> {
        let state = match state {
            AdapterState::Down => ADAPTER_STATE_DOWN,
            AdapterState::Up => ADAPTER_STATE_UP,
        };
        check_bool(unsafe { (self.api.set_adapter_state)(self.handle.as_ptr(), state) })
    }

    fn state(&self) -> io::Result<AdapterState> {
        let mut state = ADAPTER_STATE_DOWN;
        check_bool(unsafe { (self.api.get_adapter_state)(self.handle.as_ptr(), &mut state) })?;
        Ok(if state == ADAPTER_STATE_UP {
            AdapterState::Up
        } else {
            AdapterState::Down
        })
    }

    fn set_logging(&self, on: bool) -> io::Result<()> {
        let state = if on { ADAPTER_LOG_ON } else { ADAPTER_LOG_OFF };
        check_bool(unsafe { (self.api.set_adapter_logging)(self.handle.as_ptr(), state) })
    }
}

impl Drop for NtAdapter {
    fn drop(&mut self) {
        unsafe { (self.api.close_adapter)(self.handle.as_ptr()) };
    }
}
