use super::{SecretBytes, SecretKey, Zeroize};

/// Standard base64 with padding, as WireGuard writes its keys. Decoding is only for 32-byte keys,
/// and its errors never echo the text, which may be a private key.
pub struct Base64;

impl Base64 {
    const ALPHABET: &'static [u8; 64] =
        b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    /// 32 bytes take 43 characters and one `=`.
    pub const KEY_TEXT_LEN: usize = 44;
    pub const NOT_BASE64: &'static str = "not a valid base64 key";
    pub const WRONG_LENGTH: &'static str = "a key is 32 bytes: 44 base64 characters ending in '='";

    pub fn encode(bytes: &[u8]) -> String {
        let mut text = Vec::with_capacity(bytes.len().div_ceil(3) * 4);
        Self::encode_each(bytes, |c| text.push(c));
        String::from_utf8(text).expect("the alphabet is ASCII")
    }

    /// Appends the text of `bytes` to a secret buffer without an intermediate copy.
    pub fn encode_secret(bytes: &[u8], into: &mut SecretBytes) {
        Self::encode_each(bytes, |c| into.extend(&[c]));
    }

    /// A 32-byte key's raw bytes; the caller wraps or zeroes them.
    pub fn decode_key(text: &str) -> Result<[u8; SecretKey::LEN], &'static str> {
        let text = text.as_bytes();
        if text.len() != Self::KEY_TEXT_LEN || text[43] != b'=' {
            return Err(
                if text.iter().all(|c| Self::value(*c).is_some() || *c == b'=') {
                    Self::WRONG_LENGTH
                } else {
                    Self::NOT_BASE64
                },
            );
        }
        let mut sextets = [0u8; 43];
        for (slot, c) in sextets.iter_mut().zip(&text[..43]) {
            match Self::value(*c) {
                Some(value) => *slot = value,
                None => {
                    Zeroize::bytes(&mut sextets);
                    return Err(Self::NOT_BASE64);
                }
            }
        }
        // 43 characters carry 258 bits: the last 2 must be zero in a canonical key.
        if sextets[42] & 0b11 != 0 {
            Zeroize::bytes(&mut sextets);
            return Err(Self::NOT_BASE64);
        }
        let mut key = [0u8; SecretKey::LEN];
        for (index, chunk) in sextets.chunks(4).enumerate() {
            let mut group = [0u8; 4];
            group[..chunk.len()].copy_from_slice(chunk);
            let bits = u32::from(group[0]) << 18
                | u32::from(group[1]) << 12
                | u32::from(group[2]) << 6
                | u32::from(group[3]);
            Zeroize::bytes(&mut group);
            for (offset, byte) in bits.to_be_bytes()[1..].iter().enumerate() {
                if let Some(slot) = key.get_mut(index * 3 + offset) {
                    *slot = *byte;
                }
            }
        }
        Zeroize::bytes(&mut sextets);
        Ok(key)
    }

    /// A secret key from its text; the intermediate bytes are zeroed.
    pub fn decode_secret(text: &str) -> Result<SecretKey, &'static str> {
        let mut bytes = Self::decode_key(text)?;
        let key = SecretKey::new(bytes);
        Zeroize::bytes(&mut bytes);
        Ok(key)
    }

    fn encode_each(bytes: &[u8], mut push: impl FnMut(u8)) {
        for chunk in bytes.chunks(3) {
            let mut group = [0u8; 3];
            group[..chunk.len()].copy_from_slice(chunk);
            let bits = u32::from(group[0]) << 16 | u32::from(group[1]) << 8 | u32::from(group[2]);
            Zeroize::bytes(&mut group);
            for index in 0..4 {
                if index <= chunk.len() {
                    push(Self::ALPHABET[(bits >> (18 - 6 * index) & 0x3f) as usize]);
                } else {
                    push(b'=');
                }
            }
        }
    }

    fn value(c: u8) -> Option<u8> {
        match c {
            b'A'..=b'Z' => Some(c - b'A'),
            b'a'..=b'z' => Some(c - b'a' + 26),
            b'0'..=b'9' => Some(c - b'0' + 52),
            b'+' => Some(62),
            b'/' => Some(63),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_with_padding() {
        assert_eq!(Base64::encode(b""), "");
        assert_eq!(Base64::encode(b"f"), "Zg==");
        assert_eq!(Base64::encode(b"fo"), "Zm8=");
        assert_eq!(Base64::encode(b"foo"), "Zm9v");
        assert_eq!(Base64::encode(&[0xfb, 0xff]), "+/8=");
    }

    #[test]
    fn keys_round_trip() {
        let key: [u8; 32] = std::array::from_fn(|i| (i * 37 + 11) as u8);
        let text = Base64::encode(&key);
        assert_eq!(text.len(), Base64::KEY_TEXT_LEN);
        assert_eq!(Base64::decode_key(&text), Ok(key));
        let mut secret = SecretBytes::with_capacity(44);
        Base64::encode_secret(&key, &mut secret);
        assert_eq!(secret.expose(), text.as_bytes());
        assert_eq!(Base64::decode_secret(&text).unwrap().expose(), &key);
    }

    #[test]
    fn rejects_bad_keys_without_echoing_them() {
        let good = Base64::encode(&[7u8; 32]);
        let short = &good[..40];
        let bad_char = format!("{}!=", &good[..42]);
        let not_canonical = format!("{}B=", &good[..42]);
        let sixteen = Base64::encode(&[7u8; 16]);
        for (text, reason) in [
            (short, Base64::WRONG_LENGTH),
            (sixteen.as_str(), Base64::WRONG_LENGTH),
            (bad_char.as_str(), Base64::NOT_BASE64),
            (not_canonical.as_str(), Base64::NOT_BASE64),
            ("not base64 at all", Base64::NOT_BASE64),
        ] {
            let error = Base64::decode_key(text).unwrap_err();
            assert_eq!(error, reason, "{text}");
            assert!(!error.contains(&good[..8]));
        }
    }
}
