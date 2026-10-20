//! SHA-256 through BCrypt's pseudo-handle (Windows 10 and later).

use std::ffi::c_void;
use std::io;

/// `BCRYPT_SHA256_ALG_HANDLE`.
const SHA256_ALGORITHM: usize = 0x41;

pub struct Sha256;

impl Sha256 {
    pub fn digest(data: &[u8]) -> io::Result<[u8; 32]> {
        let mut out = [0u8; 32];
        let length = u32::try_from(data.len()).map_err(io::Error::other)?;
        let status = unsafe {
            BCryptHash(
                SHA256_ALGORITHM as *mut c_void,
                std::ptr::null(),
                0,
                data.as_ptr(),
                length,
                out.as_mut_ptr(),
                out.len() as u32,
            )
        };
        if status < 0 {
            return Err(io::Error::other(format!(
                "BCryptHash failed: NTSTATUS {status:#010x}"
            )));
        }
        Ok(out)
    }

    /// The digest in upper-case hex, as `vendor/wireguard-nt/README.md` writes it.
    pub fn hex(data: &[u8]) -> io::Result<String> {
        Ok(Self::digest(data)?
            .iter()
            .map(|byte| format!("{byte:02X}"))
            .collect())
    }
}

#[link(name = "bcrypt")]
unsafe extern "system" {
    fn BCryptHash(
        algorithm: *mut c_void,
        secret: *const u8,
        secret_len: u32,
        input: *const u8,
        input_len: u32,
        output: *mut u8,
        output_len: u32,
    ) -> i32;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hashes_the_standard_vectors() {
        assert_eq!(
            Sha256::hex(b"abc").unwrap(),
            "BA7816BF8F01CFEA414140DE5DAE2223B00361A396177A9CB410FF61F20015AD"
        );
        assert_eq!(
            Sha256::hex(b"").unwrap(),
            "E3B0C44298FC1C149AFBF4C8996FB92427AE41E4649B934CA495991B7852B855"
        );
    }
}
