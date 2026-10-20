use std::io;
use std::ptr::null_mut;

use super::abi::{
    GetCurrentProcess, GetTokenInformation, Handle, OpenProcessToken, Raw, TOKEN_ELEVATION_CLASS,
    TOKEN_QUERY, check_bool,
};

/// Whether this process runs elevated (an administrator console or LocalSystem): `install`,
/// `uninstall`, `cleanup` and a live `run` need it, and say so with exit code 4 otherwise.
pub struct Elevation;

impl Elevation {
    pub fn is_elevated() -> io::Result<bool> {
        let mut token: Raw = null_mut();
        check_bool(unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) })?;
        let token = Handle::new(token)?;
        let mut elevated = 0u32;
        let mut returned = 0u32;
        check_bool(unsafe {
            GetTokenInformation(
                token.raw(),
                TOKEN_ELEVATION_CLASS,
                (&mut elevated as *mut u32).cast(),
                size_of::<u32>() as u32,
                &mut returned,
            )
        })?;
        Ok(elevated != 0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_token_of_this_process() {
        // Either answer is fine; the query itself must work unelevated.
        assert!(Elevation::is_elevated().is_ok());
    }
}
