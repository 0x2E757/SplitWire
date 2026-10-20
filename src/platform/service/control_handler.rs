use std::io;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicPtr, AtomicU32, Ordering};

use super::Dispatcher;
use super::abi::{
    ERROR_CALL_NOT_IMPLEMENTED, ERROR_SERVICE_SPECIFIC_ERROR, NO_ERROR, PBT_APMRESUMEAUTOMATIC,
    PBT_APMRESUMESUSPEND, RegisterServiceCtrlHandlerExW, SERVICE_ACCEPT_POWEREVENT,
    SERVICE_ACCEPT_SHUTDOWN, SERVICE_ACCEPT_STOP, SERVICE_CONTROL_INTERROGATE,
    SERVICE_CONTROL_POWEREVENT, SERVICE_CONTROL_SHUTDOWN, SERVICE_CONTROL_STOP, SERVICE_RUNNING,
    SERVICE_START_PENDING, SERVICE_STOP_PENDING, SERVICE_STOPPED, SERVICE_WIN32_OWN_PROCESS,
    ServiceStatus, SetServiceStatus,
};
use crate::platform::abi::{Raw, check_bool, last_error, wide};

/// What the service does on the SCM's events: stop and shutdown end the run (`StopSignal`),
/// a resume from sleep runs the periodic jobs at once (`SchedulerWaker::wake`).
pub struct ServiceEvents {
    pub on_stop: Box<dyn Fn() + Send + Sync>,
    pub on_resume: Box<dyn Fn() + Send + Sync>,
}

struct Registered {
    events: ServiceEvents,
    handle: AtomicPtr<std::ffi::c_void>,
    check_point: AtomicU32,
}

static REGISTERED: OnceLock<Registered> = OnceLock::new();

/// The service's control handler and its status reports to the SCM. It accepts stop, shutdown
/// and power events while running.
pub struct ControlHandler;

impl ControlHandler {
    /// How long a start or a stop may take before the SCM gives up on it.
    const WAIT_HINT_MS: u32 = 30_000;

    /// Registers the handler for the running service; once per process.
    pub fn register(events: ServiceEvents) -> io::Result<Self> {
        let registered = Registered {
            events,
            handle: AtomicPtr::new(std::ptr::null_mut()),
            check_point: AtomicU32::new(0),
        };
        REGISTERED
            .set(registered)
            .map_err(|_| io::Error::other("the control handler is already registered"))?;
        let name = wide(Dispatcher::SERVICE);
        let handle = unsafe {
            RegisterServiceCtrlHandlerExW(name.as_ptr(), Self::handle, std::ptr::null_mut())
        };
        if handle.is_null() {
            return Err(last_error());
        }
        Self::registered().handle.store(handle, Ordering::SeqCst);
        Ok(Self)
    }

    pub fn starting(&self) -> io::Result<()> {
        Self::report(SERVICE_START_PENDING, 0, None)
    }

    pub fn running(&self) -> io::Result<()> {
        let accepted = SERVICE_ACCEPT_STOP | SERVICE_ACCEPT_SHUTDOWN | SERVICE_ACCEPT_POWEREVENT;
        Self::report(SERVICE_RUNNING, accepted, None)
    }

    pub fn stopping(&self) -> io::Result<()> {
        Self::report(SERVICE_STOP_PENDING, 0, None)
    }

    /// `error` is the service's own exit code when the run failed.
    pub fn stopped(&self, error: Option<u32>) -> io::Result<()> {
        Self::report(SERVICE_STOPPED, 0, error)
    }

    fn report(state: u32, accepted: u32, error: Option<u32>) -> io::Result<()> {
        let registered = Self::registered();
        let pending = matches!(state, SERVICE_START_PENDING | SERVICE_STOP_PENDING);
        let check_point = if pending {
            registered.check_point.fetch_add(1, Ordering::SeqCst) + 1
        } else {
            0
        };
        let status = ServiceStatus {
            service_type: SERVICE_WIN32_OWN_PROCESS,
            current_state: state,
            controls_accepted: accepted,
            win32_exit_code: error.map_or(NO_ERROR, |_| ERROR_SERVICE_SPECIFIC_ERROR),
            service_exit_code: error.unwrap_or(0),
            check_point,
            wait_hint: if pending { Self::WAIT_HINT_MS } else { 0 },
        };
        let handle = registered.handle.load(Ordering::SeqCst);
        check_bool(unsafe { SetServiceStatus(handle, &status) })
    }

    fn registered() -> &'static Registered {
        REGISTERED.get().expect("registered before any report")
    }

    unsafe extern "system" fn handle(control: u32, event: u32, _data: Raw, _context: Raw) -> u32 {
        let Some(registered) = REGISTERED.get() else {
            return ERROR_CALL_NOT_IMPLEMENTED;
        };
        match control {
            SERVICE_CONTROL_STOP | SERVICE_CONTROL_SHUTDOWN => {
                let _ = Self::report(SERVICE_STOP_PENDING, 0, None);
                (registered.events.on_stop)();
                NO_ERROR
            }
            SERVICE_CONTROL_POWEREVENT => {
                if matches!(event, PBT_APMRESUMEAUTOMATIC | PBT_APMRESUMESUSPEND) {
                    (registered.events.on_resume)();
                }
                NO_ERROR
            }
            SERVICE_CONTROL_INTERROGATE => NO_ERROR,
            _ => ERROR_CALL_NOT_IMPLEMENTED,
        }
    }
}
