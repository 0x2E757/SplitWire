use std::fmt;
use std::sync::atomic::{Ordering, compiler_fence};

/// Bytes that hold key material, such as a profile's plaintext before encryption. They are zeroed
/// on drop and `Debug` is redacted.
#[derive(Default)]
pub struct SecretBytes(Vec<u8>);

impl SecretBytes {
    /// An empty buffer that holds `capacity` bytes without reallocating, so no copy is left behind
    /// in freed memory while it fills.
    pub fn with_capacity(capacity: usize) -> Self {
        Self(Vec::with_capacity(capacity))
    }

    pub fn from_vec(bytes: Vec<u8>) -> Self {
        Self(bytes)
    }

    pub fn expose(&self) -> &[u8] {
        &self.0
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn extend(&mut self, bytes: &[u8]) {
        assert!(
            self.0.len() + bytes.len() <= self.0.capacity(),
            "SecretBytes would reallocate and leave a copy behind"
        );
        self.0.extend_from_slice(bytes);
    }
}

impl fmt::Debug for SecretBytes {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "SecretBytes({} bytes, redacted)", self.0.len())
    }
}

impl Drop for SecretBytes {
    fn drop(&mut self) {
        // The spare capacity may hold bytes from an earlier, shorter fill: clear the whole allocation.
        let capacity = self.0.capacity();
        self.0.resize(capacity, 0);
        Zeroize::bytes(&mut self.0);
    }
}

/// Overwrites memory the optimizer would otherwise consider dead.
pub struct Zeroize;

impl Zeroize {
    pub fn bytes(bytes: &mut [u8]) {
        for byte in bytes {
            // Volatile, so the write to memory that is about to be freed is not removed.
            unsafe { std::ptr::write_volatile(byte, 0) };
        }
        compiler_fence(Ordering::SeqCst);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debug_shows_only_the_length() {
        let mut secret = SecretBytes::with_capacity(8);
        secret.extend(b"hunter2");
        assert_eq!(format!("{secret:?}"), "SecretBytes(7 bytes, redacted)");
        assert_eq!(secret.expose(), b"hunter2");
    }

    #[test]
    #[should_panic(expected = "reallocate")]
    fn refuses_to_grow_past_its_capacity() {
        let mut secret = SecretBytes::with_capacity(2);
        secret.extend(b"abc");
    }

    #[test]
    fn zeroize_clears_every_byte() {
        let mut bytes = [7u8; 5];
        Zeroize::bytes(&mut bytes);
        assert_eq!(bytes, [0; 5]);
    }
}
