use std::io;
use std::ptr::{null, null_mut};

use super::abi::{
    CRYPTPROTECT_LOCAL_MACHINE, CRYPTPROTECT_UI_FORBIDDEN, CryptProtectData, CryptUnprotectData,
    DataBlob, LocalFree, check_bool,
};
use crate::profile::{SecretBytes, SecretStore, Zeroize};

/// Machine-scope DPAPI: any process on this machine may decrypt, so the protection of a blob is the
/// data folder's ACL; a copied blob is useless on another machine.
pub struct DpapiSecretStore {
    entropy: &'static [u8],
}

impl DpapiSecretStore {
    /// The entropy of profile blobs, version 1.
    pub const PROFILE_ENTROPY: &'static [u8] = b"SplitWire profile v1";

    pub fn new(entropy: &'static [u8]) -> Self {
        Self { entropy }
    }

    pub fn profiles() -> Self {
        Self::new(Self::PROFILE_ENTROPY)
    }

    fn blob(bytes: &[u8]) -> DataBlob {
        DataBlob {
            size: bytes.len() as u32,
            data: bytes.as_ptr().cast_mut(),
        }
    }

    /// Takes the output buffer of a DPAPI call, zeroing and freeing it.
    fn take(output: DataBlob, zero: bool) -> Vec<u8> {
        if output.data.is_null() {
            return Vec::new();
        }
        // DPAPI allocated `size` bytes at `data` with LocalAlloc; they are copied, then released.
        let bytes = unsafe { std::slice::from_raw_parts_mut(output.data, output.size as usize) };
        let copy = bytes.to_vec();
        if zero {
            Zeroize::bytes(bytes);
        }
        unsafe { LocalFree(output.data.cast()) };
        copy
    }
}

impl SecretStore for DpapiSecretStore {
    fn protect(&self, plaintext: &SecretBytes) -> io::Result<Vec<u8>> {
        let input = Self::blob(plaintext.expose());
        let entropy = Self::blob(self.entropy);
        let mut output = DataBlob {
            size: 0,
            data: null_mut(),
        };
        check_bool(unsafe {
            CryptProtectData(
                &input,
                null(),
                &entropy,
                null_mut(),
                null_mut(),
                CRYPTPROTECT_LOCAL_MACHINE | CRYPTPROTECT_UI_FORBIDDEN,
                &mut output,
            )
        })?;
        Ok(Self::take(output, false))
    }

    fn unprotect(&self, blob: &[u8]) -> io::Result<SecretBytes> {
        let input = Self::blob(blob);
        let entropy = Self::blob(self.entropy);
        let mut output = DataBlob {
            size: 0,
            data: null_mut(),
        };
        check_bool(unsafe {
            CryptUnprotectData(
                &input,
                null_mut(),
                &entropy,
                null_mut(),
                null_mut(),
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut output,
            )
        })?;
        Ok(SecretBytes::from_vec(Self::take(output, true)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A real round trip on made-up data: harmless, and it proves the ABI.
    #[test]
    fn round_trips_through_machine_dpapi() {
        let store = DpapiSecretStore::new(b"SplitWire test entropy");
        let mut plaintext = SecretBytes::with_capacity(64);
        plaintext.extend(b"version=1\nprivate_key=made-up-test-value\n");
        let blob = store.protect(&plaintext).unwrap();
        assert!(blob.len() > plaintext.len());
        assert!(!blob.windows(12).any(|w| w == b"made-up-test"));
        assert_eq!(store.unprotect(&blob).unwrap().expose(), plaintext.expose());
    }

    #[test]
    fn other_entropy_and_damaged_blobs_fail() {
        let store = DpapiSecretStore::profiles();
        let mut plaintext = SecretBytes::with_capacity(8);
        plaintext.extend(b"made-up");
        let mut blob = store.protect(&plaintext).unwrap();
        assert!(DpapiSecretStore::new(b"other").unprotect(&blob).is_err());
        let last = blob.len() - 1;
        blob[last] ^= 0xff;
        assert!(store.unprotect(&blob).is_err());
    }
}
