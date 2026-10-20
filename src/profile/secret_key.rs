use std::fmt;

use super::Zeroize;

/// A WireGuard private or preshared key. Its bytes are zeroed on drop; `Debug` is redacted and
/// there is no `Display`, so a key cannot reach a log, a status or an error by accident.
#[derive(Clone)]
pub struct SecretKey([u8; SecretKey::LEN]);

impl SecretKey {
    pub const LEN: usize = 32;

    /// Takes the key; the caller zeroes its own copy of `bytes`.
    pub fn new(bytes: [u8; Self::LEN]) -> Self {
        Self(bytes)
    }

    /// The raw bytes, for the driver's configuration buffer only.
    pub fn expose(&self) -> &[u8; Self::LEN] {
        &self.0
    }
}

impl fmt::Debug for SecretKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("SecretKey(redacted)")
    }
}

impl Drop for SecretKey {
    fn drop(&mut self) {
        Zeroize::bytes(&mut self.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debug_never_shows_the_bytes() {
        let key = SecretKey::new([0xab; SecretKey::LEN]);
        let shown = format!("{key:?}");
        assert_eq!(shown, "SecretKey(redacted)");
        assert!(!shown.to_ascii_lowercase().contains("ab"));
        assert_eq!(key.expose()[31], 0xab);
    }
}
