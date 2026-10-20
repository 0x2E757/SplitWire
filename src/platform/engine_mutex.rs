use std::io;

use super::abi::{CreateMutexW, Handle, wide};

const ERROR_ALREADY_EXISTS: i32 = 183;

/// `Global\SplitWire.Engine`: only one live engine runs on the machine. Held for the
/// life of the run; a dry run does not take it.
pub struct EngineMutex(#[allow(dead_code)] Handle);

impl EngineMutex {
    pub const NAME: &'static str = r"Global\SplitWire.Engine";

    pub fn acquire() -> io::Result<Self> {
        let name = wide(Self::NAME);
        let handle = Handle::new(unsafe { CreateMutexW(std::ptr::null_mut(), 0, name.as_ptr()) })?;
        if io::Error::last_os_error().raw_os_error() == Some(ERROR_ALREADY_EXISTS) {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                "another SplitWire engine is running",
            ));
        }
        Ok(Self(handle))
    }
}
