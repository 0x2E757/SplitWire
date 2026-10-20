use std::io;
use std::sync::OnceLock;

use super::abi::{Bool, SetConsoleCtrlHandler, check_bool};

type Callback = Box<dyn Fn() + Send + Sync>;

static CALLBACK: OnceLock<Callback> = OnceLock::new();

/// Ctrl+C, Ctrl+Break or closing the console of a foreground run: the callback is told instead
/// of the process ending at once, so the run stops cleanly.
pub struct ConsoleInterrupt;

impl ConsoleInterrupt {
    /// Installs `callback`; one per process.
    pub fn on_interrupt(callback: impl Fn() + Send + Sync + 'static) -> io::Result<()> {
        CALLBACK
            .set(Box::new(callback))
            .map_err(|_| io::Error::other("the console handler is already installed"))?;
        check_bool(unsafe { SetConsoleCtrlHandler(Some(Self::handle), 1) })
    }

    unsafe extern "system" fn handle(_event: u32) -> Bool {
        match CALLBACK.get() {
            Some(callback) => {
                callback();
                1
            }
            None => 0,
        }
    }
}
