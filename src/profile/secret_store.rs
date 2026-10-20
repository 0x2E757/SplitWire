use std::io;

use super::SecretBytes;

/// Machine-bound encryption of a profile's plaintext (DPAPI in `platform`); a fake in tests.
pub trait SecretStore: Send + Sync {
    fn protect(&self, plaintext: &SecretBytes) -> io::Result<Vec<u8>>;
    fn unprotect(&self, blob: &[u8]) -> io::Result<SecretBytes>;
}
