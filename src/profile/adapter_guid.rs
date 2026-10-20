use std::collections::hash_map::RandomState;
use std::hash::BuildHasher;
use std::time::{SystemTime, UNIX_EPOCH};

/// The adapter GUID made at import and kept in the blob, so Windows keeps one network per profile.
/// It needs to be unique, not secret: std's per-process random hash keys are enough.
pub struct AdapterGuid;

impl AdapterGuid {
    /// A random RFC 4122 version 4 GUID.
    pub fn random() -> u128 {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos());
        let high = RandomState::new().hash_one((nanos, std::process::id()));
        let low = RandomState::new().hash_one((nanos, high));
        let value = u128::from(high) << 64 | u128::from(low);
        value & !(0xf000 << 64 | 0xc000 << 48) | (0x4000 << 64 | 0x8000 << 48)
    }

    /// `6b29fc40-ca47-1067-b31d-00dd010662da`.
    pub fn format(value: u128) -> String {
        let hex = format!("{value:032x}");
        format!(
            "{}-{}-{}-{}-{}",
            &hex[..8],
            &hex[8..12],
            &hex[12..16],
            &hex[16..20],
            &hex[20..]
        )
    }

    pub fn parse(text: &str) -> Option<u128> {
        let parts: Vec<&str> = text.split('-').collect();
        let lengths: Vec<usize> = parts.iter().map(|part| part.len()).collect();
        if lengths != [8, 4, 4, 4, 12]
            || !parts
                .iter()
                .all(|p| p.bytes().all(|b| b.is_ascii_hexdigit()))
        {
            return None;
        }
        u128::from_str_radix(&parts.concat(), 16).ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn random_guids_are_version_4_and_differ() {
        let first = AdapterGuid::random();
        let second = AdapterGuid::random();
        assert_ne!(first, second);
        let text = AdapterGuid::format(first);
        assert_eq!(&text[14..15], "4");
        assert!(matches!(&text[19..20], "8" | "9" | "a" | "b"), "{text}");
    }

    #[test]
    fn formats_and_parses() {
        let value = 0x6b29fc40_ca47_1067_b31d_00dd010662da;
        let text = AdapterGuid::format(value);
        assert_eq!(text, "6b29fc40-ca47-1067-b31d-00dd010662da");
        assert_eq!(AdapterGuid::parse(&text), Some(value));
        assert_eq!(AdapterGuid::parse("6b29fc40ca471067b31d00dd010662da"), None);
        assert_eq!(
            AdapterGuid::parse("6b29fc40-ca47-1067-b31d-00dd010662dz"),
            None
        );
    }
}
