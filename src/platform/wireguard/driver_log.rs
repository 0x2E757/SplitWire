use std::sync::Mutex;

use super::layout::{LOG_ERR, LOG_WARN};
use crate::logs::Log;

/// The log the driver's lines go to. The logger callback carries no context pointer, so the
/// target is process-wide, like the callback itself.
static TARGET: Mutex<Option<Log>> = Mutex::new(None);

/// Lines longer than this are cut: the driver's are short, and a missing terminator must not run
/// the reader off into memory.
const MAX_MESSAGE: usize = 4096;

/// The wireguard-nt logger callback, writing to `Log` with the source `wireguard`.
pub struct DriverLog;

impl DriverLog {
    pub fn attach(log: &Log) {
        *TARGET.lock().unwrap_or_else(|e| e.into_inner()) = Some(log.with_source("wireguard"));
    }

    /// Called by the driver from any of its threads. The driver's timestamp is dropped: the log
    /// stamps lines with its own clock, so they sort with everything else.
    ///
    /// # Safety
    ///
    /// `message` is null or a NUL-terminated UTF-16 string, as the driver passes it.
    pub unsafe extern "system" fn callback(level: i32, _timestamp: u64, message: *const u16) {
        let text = unsafe { Self::read(message) };
        let Ok(target) = TARGET.lock() else { return };
        let Some(log) = target.as_ref() else { return };
        match level {
            LOG_ERR => log.error(text),
            LOG_WARN => log.warn(text),
            _ => log.info(text),
        }
    }

    unsafe fn read(message: *const u16) -> String {
        if message.is_null() {
            return String::new();
        }
        let mut len = 0;
        while len < MAX_MESSAGE && unsafe { *message.add(len) } != 0 {
            len += 1;
        }
        String::from_utf16_lossy(unsafe { std::slice::from_raw_parts(message, len) })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::clock::ManualClock;
    use crate::logs::Level;
    use crate::platform::abi::wide;
    use std::sync::Arc;
    use std::time::UNIX_EPOCH;

    #[test]
    fn driver_lines_reach_the_log_with_their_level() {
        let log = Log::new(10_000, Arc::new(ManualClock::new(UNIX_EPOCH)), false);
        DriverLog::attach(&log);
        unsafe {
            DriverLog::callback(LOG_WARN, 0, wide("Handshake did not complete").as_ptr());
            DriverLog::callback(LOG_ERR, 0, wide("Failed to bind").as_ptr());
            DriverLog::callback(0, 0, std::ptr::null());
        }
        let entries = log.since(0, 10);
        assert_eq!(entries.len(), 3);
        assert_eq!(entries[0].level, Level::Warn);
        assert_eq!(entries[0].source, "wireguard");
        assert_eq!(entries[0].message, "Handshake did not complete");
        assert_eq!(entries[1].level, Level::Error);
        assert_eq!(entries[2].message, "");
    }
}
