use std::io;
use std::sync::OnceLock;

use super::abi::{ServiceTableEntry, StartServiceCtrlDispatcherW};
use crate::platform::abi::{check_bool, wide};

type Body = Box<dyn Fn(Vec<String>) + Send + Sync>;

static BODY: OnceLock<Body> = OnceLock::new();

/// The SCM entry (`SplitWire.exe service`): hands the main thread to the service control
/// manager, which calls the service's body on a thread of its own with the start arguments
/// (`install --restore` starts it with `--restore`). Returns when the service has stopped.
pub struct Dispatcher;

impl Dispatcher {
    pub const SERVICE: &'static str = "SplitWire";

    /// `body` runs the service: it registers the `ControlHandler`, reports its states and returns
    /// once stopped. Fails at once when not started by the SCM.
    pub fn run(body: impl Fn(Vec<String>) + Send + Sync + 'static) -> io::Result<()> {
        BODY.set(Box::new(body))
            .map_err(|_| io::Error::other("the service entry runs once per process"))?;
        // The SCM keeps no copy of the table: it must outlive the call, which it does.
        let name = wide(Self::SERVICE);
        let table = [
            ServiceTableEntry {
                name: name.as_ptr(),
                main: Some(Self::service_main),
            },
            ServiceTableEntry {
                name: std::ptr::null(),
                main: None,
            },
        ];
        check_bool(unsafe { StartServiceCtrlDispatcherW(table.as_ptr()) })
    }

    unsafe extern "system" fn service_main(argc: u32, argv: *mut *mut u16) {
        // The first argument is the service's name.
        let args = (1..argc as usize)
            .map(|index| unsafe { Self::text(*argv.add(index)) })
            .collect();
        if let Some(body) = BODY.get() {
            body(args);
        }
    }

    /// A NUL-terminated UTF-16 string the SCM passed.
    unsafe fn text(raw: *const u16) -> String {
        if raw.is_null() {
            return String::new();
        }
        let mut len = 0;
        while unsafe { *raw.add(len) } != 0 {
            len += 1;
        }
        String::from_utf16_lossy(unsafe { std::slice::from_raw_parts(raw, len) })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_arguments_the_scm_passes() {
        let text = wide("--restore");
        assert_eq!(unsafe { Dispatcher::text(text.as_ptr()) }, "--restore");
        assert_eq!(unsafe { Dispatcher::text(std::ptr::null()) }, "");
    }
}
