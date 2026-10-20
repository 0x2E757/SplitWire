use std::io;
use std::path::Path;
use std::ptr::{null, null_mut};
use std::thread;
use std::time::{Duration, Instant};

use super::Dispatcher;
use super::abi::{
    ChangeServiceConfig2W, ChangeServiceConfigW, CloseServiceHandle, ControlService,
    CreateServiceW, DeleteService, ERROR_SERVICE_DOES_NOT_EXIST, ERROR_SERVICE_NOT_ACTIVE,
    OpenSCManagerW, OpenServiceW, QueryServiceStatus, SC_ACTION_RESTART, SC_MANAGER_CONNECT,
    SC_MANAGER_CREATE_SERVICE, SERVICE_ALL_ACCESS, SERVICE_AUTO_START, SERVICE_CONFIG_DESCRIPTION,
    SERVICE_CONFIG_FAILURE_ACTIONS, SERVICE_CONFIG_FAILURE_ACTIONS_FLAG,
    SERVICE_CONFIG_SERVICE_SID_INFO, SERVICE_CONTROL_STOP, SERVICE_ERROR_NORMAL,
    SERVICE_SID_TYPE_UNRESTRICTED, SERVICE_STOPPED, SERVICE_WIN32_OWN_PROCESS, ScAction,
    ServiceDescription, ServiceFailureActions, ServiceFailureActionsFlag, ServiceSidInfo,
    ServiceStatus, StartServiceW,
};
use crate::platform::abi::{Raw, check_bool, last_error, wide};

/// Creates, configures, starts, stops and deletes the `SplitWire` service: LocalSystem,
/// automatic start, an unrestricted service SID, restarted after 5 s, 5 s and 30 s with the count
/// reset after a day. Only `install` and `uninstall` call it, from an elevated console; nothing
/// else, and no test, ever does.
pub struct Installer;

impl Installer {
    const DESCRIPTION: &'static str =
        "Sends the listed domains of each WireGuard profile through its own tunnel.";
    const RESTART_DELAYS_MS: [u32; 3] = [5_000, 5_000, 30_000];
    const RESET_SECS: u32 = 86_400;
    const STOP_WAIT: Duration = Duration::from_secs(30);

    /// What the SCM runs: the executable, quoted, with the `service` command.
    pub fn command_line(exe: &Path) -> String {
        format!("\"{}\" service", exe.display())
    }

    /// Creates the service, or points an existing one at `exe` again, and configures it.
    pub fn register(exe: &Path) -> io::Result<()> {
        let manager = ScHandle::manager(SC_MANAGER_CONNECT | SC_MANAGER_CREATE_SERVICE)?;
        let name = wide(Dispatcher::SERVICE);
        let command = wide(&Self::command_line(exe));
        let service = match manager.open(SERVICE_ALL_ACCESS) {
            Ok(service) => {
                check_bool(unsafe {
                    ChangeServiceConfigW(
                        service.0,
                        SERVICE_WIN32_OWN_PROCESS,
                        SERVICE_AUTO_START,
                        SERVICE_ERROR_NORMAL,
                        command.as_ptr(),
                        null(),
                        null_mut(),
                        null(),
                        null(),
                        null(),
                        name.as_ptr(),
                    )
                })?;
                service
            }
            Err(e) if e.raw_os_error() == Some(ERROR_SERVICE_DOES_NOT_EXIST) => {
                ScHandle::new(unsafe {
                    CreateServiceW(
                        manager.0,
                        name.as_ptr(),
                        name.as_ptr(),
                        SERVICE_ALL_ACCESS,
                        SERVICE_WIN32_OWN_PROCESS,
                        SERVICE_AUTO_START,
                        SERVICE_ERROR_NORMAL,
                        command.as_ptr(),
                        null(),
                        null_mut(),
                        null(),
                        // LocalSystem.
                        null(),
                        null(),
                    )
                })?
            }
            Err(e) => return Err(e),
        };
        Self::configure(&service)
    }

    /// Starts the service with `args` (`--restore` after an update).
    pub fn start(args: &[&str]) -> io::Result<()> {
        let manager = ScHandle::manager(SC_MANAGER_CONNECT)?;
        let service = manager.open(SERVICE_ALL_ACCESS)?;
        let args: Vec<Vec<u16>> = args.iter().map(|a| wide(a)).collect();
        let pointers: Vec<*const u16> = args.iter().map(|a| a.as_ptr()).collect();
        check_bool(unsafe {
            StartServiceW(
                service.0,
                pointers.len() as u32,
                if pointers.is_empty() {
                    null()
                } else {
                    pointers.as_ptr()
                },
            )
        })
    }

    /// Stops the service (which disconnects every profile), waiting up to 30 s, then deletes it.
    /// Nothing to do when it is not installed.
    pub fn remove() -> io::Result<()> {
        let manager = ScHandle::manager(SC_MANAGER_CONNECT)?;
        let service = match manager.open(SERVICE_ALL_ACCESS) {
            Ok(service) => service,
            Err(e) if e.raw_os_error() == Some(ERROR_SERVICE_DOES_NOT_EXIST) => return Ok(()),
            Err(e) => return Err(e),
        };
        let mut status = ServiceStatus::default();
        if unsafe { ControlService(service.0, SERVICE_CONTROL_STOP, &mut status) } == 0 {
            let error = last_error();
            if error.raw_os_error() != Some(ERROR_SERVICE_NOT_ACTIVE) {
                return Err(error);
            }
        }
        let deadline = Instant::now() + Self::STOP_WAIT;
        loop {
            check_bool(unsafe { QueryServiceStatus(service.0, &mut status) })?;
            if status.current_state == SERVICE_STOPPED {
                break;
            }
            if Instant::now() >= deadline {
                return Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    "the service did not stop within 30 s",
                ));
            }
            thread::sleep(Duration::from_millis(250));
        }
        check_bool(unsafe { DeleteService(service.0) })
    }

    fn configure(service: &ScHandle) -> io::Result<()> {
        let description = wide(Self::DESCRIPTION);
        let mut info = ServiceDescription {
            description: description.as_ptr(),
        };
        service.config(
            SERVICE_CONFIG_DESCRIPTION,
            (&mut info as *mut ServiceDescription).cast(),
        )?;
        let actions = Self::RESTART_DELAYS_MS.map(|delay_ms| ScAction {
            kind: SC_ACTION_RESTART,
            delay_ms,
        });
        let mut failure = ServiceFailureActions {
            reset_period_secs: Self::RESET_SECS,
            reboot_message: null(),
            command: null(),
            actions_count: actions.len() as u32,
            actions: actions.as_ptr(),
        };
        service.config(
            SERVICE_CONFIG_FAILURE_ACTIONS,
            (&mut failure as *mut ServiceFailureActions).cast(),
        )?;
        let mut flag = ServiceFailureActionsFlag {
            on_non_crash_failures: 1,
        };
        service.config(
            SERVICE_CONFIG_FAILURE_ACTIONS_FLAG,
            (&mut flag as *mut ServiceFailureActionsFlag).cast(),
        )?;
        let mut sid = ServiceSidInfo {
            sid_type: SERVICE_SID_TYPE_UNRESTRICTED,
        };
        service.config(
            SERVICE_CONFIG_SERVICE_SID_INFO,
            (&mut sid as *mut ServiceSidInfo).cast(),
        )
    }
}

/// An SCM or service handle, closed on drop.
struct ScHandle(Raw);

impl ScHandle {
    fn new(raw: Raw) -> io::Result<Self> {
        if raw.is_null() {
            Err(last_error())
        } else {
            Ok(Self(raw))
        }
    }

    fn manager(access: u32) -> io::Result<Self> {
        Self::new(unsafe { OpenSCManagerW(null(), null(), access) })
    }

    /// The `SplitWire` service of this manager.
    fn open(&self, access: u32) -> io::Result<Self> {
        let name = wide(Dispatcher::SERVICE);
        Self::new(unsafe { OpenServiceW(self.0, name.as_ptr(), access) })
    }

    fn config(&self, level: u32, info: Raw) -> io::Result<()> {
        check_bool(unsafe { ChangeServiceConfig2W(self.0, level, info) })
    }
}

impl Drop for ScHandle {
    fn drop(&mut self) {
        unsafe { CloseServiceHandle(self.0) };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_scm_runs_the_quoted_exe_with_the_service_command() {
        let exe = Path::new(r"C:\Program Files\SplitWire\SplitWire.exe");
        assert_eq!(
            Installer::command_line(exe),
            r#""C:\Program Files\SplitWire\SplitWire.exe" service"#
        );
    }
}
