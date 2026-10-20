//! One window process per session: the mutex `Local\SplitWire.Window` marks the running
//! instance; a second start sets its `.Show` event and ends, and the first brings its window to the
//! front. `.Exit` lets the installer close it. A demo and another pipe get names of their own, so
//! they never reach the real window.

use std::io;
use std::ptr;

use crate::platform::abi::{
    CreateEventW, CreateMutexW, Handle, SetEvent, WAIT_OBJECT_0, WaitForSingleObject, wide,
};
use crate::platform::pipe::PipeName;

const ERROR_ALREADY_EXISTS: i32 = 183;

/// The events of the instance, signalled by others.
struct Signals {
    show: Handle,
    exit: Handle,
}

impl Signals {
    /// Opens the events, creating them when this is the first instance.
    fn of(name: &str) -> io::Result<Self> {
        let event = |suffix: &str| {
            let name = wide(&format!(r"Local\{name}.{suffix}"));
            // SAFETY: an auto-reset, unsignalled event with a NUL-terminated name.
            Handle::new(unsafe { CreateEventW(ptr::null_mut(), 0, 0, name.as_ptr()) })
        };
        Ok(Self {
            show: event("Show")?,
            exit: event("Exit")?,
        })
    }
}

/// The running instance: it holds the mutex and listens to the events.
pub struct Instance {
    _mutex: Handle,
    signals: Signals,
}

impl Instance {
    /// A second start asked for the window (one answer per request).
    pub fn show_requested(&self) -> bool {
        Self::signalled(&self.signals.show)
    }

    /// The installer asked the process to end.
    pub fn exit_requested(&self) -> bool {
        Self::signalled(&self.signals.exit)
    }

    fn signalled(event: &Handle) -> bool {
        // SAFETY: a zero wait on an event this process holds.
        unsafe { WaitForSingleObject(event.raw(), 0) == WAIT_OBJECT_0 }
    }
}

/// Another process is the instance.
pub struct Other {
    signals: Signals,
}

impl Other {
    /// Asks it to show its window; it may take the foreground, which this start was given.
    pub fn raise(&self) -> io::Result<()> {
        // SAFETY: ASFW_ANY, then setting an event this process holds.
        unsafe {
            AllowSetForegroundWindow(u32::MAX);
            Self::set(&self.signals.show)
        }
    }

    /// Asks it to end. The installer sets the same event itself before it replaces the files.
    #[cfg(test)]
    pub fn close(&self) -> io::Result<()> {
        Self::set(&self.signals.exit)
    }

    fn set(event: &Handle) -> io::Result<()> {
        // SAFETY: setting an event this process holds.
        match unsafe { SetEvent(event.raw()) } {
            0 => Err(io::Error::last_os_error()),
            _ => Ok(()),
        }
    }
}

pub enum Claim {
    First(Instance),
    Running(Other),
}

pub struct SingleInstance;

impl SingleInstance {
    pub const NAME: &'static str = "SplitWire.Window";

    /// The instance name of a start: the service's window, a demo's, or another pipe's.
    pub fn name(demo: bool, pipe: Option<&PipeName>) -> String {
        match (demo, pipe) {
            (true, _) => format!("{}.Demo", Self::NAME),
            (false, Some(pipe)) if !pipe.is_service() => format!("{}.{pipe}", Self::NAME),
            _ => Self::NAME.to_owned(),
        }
    }

    /// Becomes the instance named `name`, or finds the one that is. The events exist before the
    /// mutex, so whoever finds the mutex can signal them.
    pub fn claim(name: &str) -> io::Result<Claim> {
        let signals = Signals::of(name)?;
        let mutex_name = wide(&format!(r"Local\{name}"));
        // SAFETY: a named mutex, not owned; its last error says whether it existed.
        let mutex = Handle::new(unsafe { CreateMutexW(ptr::null_mut(), 0, mutex_name.as_ptr()) })?;
        if io::Error::last_os_error().raw_os_error() == Some(ERROR_ALREADY_EXISTS) {
            return Ok(Claim::Running(Other { signals }));
        }
        Ok(Claim::First(Instance {
            _mutex: mutex,
            signals,
        }))
    }
}

#[link(name = "user32")]
unsafe extern "system" {
    fn AllowSetForegroundWindow(process: u32) -> i32;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_second_start_signals_the_first() {
        let name = format!("SplitWire.Test.{}.Window", std::process::id());
        let Claim::First(first) = SingleInstance::claim(&name).unwrap() else {
            panic!("the first claim");
        };
        assert!(!first.show_requested());
        let Claim::Running(second) = SingleInstance::claim(&name).unwrap() else {
            panic!("the second claim finds the first");
        };
        second.raise().unwrap();
        assert!(first.show_requested());
        assert!(!first.show_requested(), "one answer per request");
        assert!(!first.exit_requested());
        second.close().unwrap();
        assert!(first.exit_requested());
        drop(second);
        drop(first);
        assert!(matches!(
            SingleInstance::claim(&name).unwrap(),
            Claim::First(_)
        ));
    }

    #[test]
    fn demos_and_other_pipes_have_names_of_their_own() {
        assert_eq!(SingleInstance::name(false, None), "SplitWire.Window");
        assert_eq!(
            SingleInstance::name(false, Some(&PipeName::service())),
            "SplitWire.Window"
        );
        assert_eq!(
            SingleInstance::name(false, Some(&PipeName::dry_run())),
            "SplitWire.Window.SplitWire.DryRun"
        );
        assert_eq!(SingleInstance::name(true, None), "SplitWire.Window.Demo");
    }
}
