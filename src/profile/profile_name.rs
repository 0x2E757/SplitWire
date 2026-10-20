use std::fmt;

/// A validated profile name: `[A-Za-z0-9._-]`, 1–32 characters, not a reserved Windows device name
/// and not ending in a dot. Every folder and adapter name is built from one, so no name can reach
/// outside `profiles\` or collide with a device.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ProfileName(String);

impl ProfileName {
    pub const MAX_LEN: usize = 32;
    const RESERVED: [&'static str; 4] = ["con", "prn", "aux", "nul"];
    const NUMBERED: [&'static str; 2] = ["com", "lpt"];

    pub fn parse(text: &str) -> Result<Self, String> {
        if text.is_empty() {
            return Err("Type a name".into());
        }
        if text.chars().count() > Self::MAX_LEN {
            return Err(format!("At most {} characters", Self::MAX_LEN));
        }
        if !text
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'))
        {
            return Err("Letters, digits, . _ - only (no spaces or brackets)".into());
        }
        // Windows drops trailing dots from file names, so `a.` would be the folder `a`; this also
        // refuses `.` and `..`.
        if text.ends_with('.') {
            return Err("A name cannot end with a dot".into());
        }
        if Self::is_reserved(text) {
            return Err(format!("{text} is a reserved Windows name"));
        }
        Ok(Self(text.to_owned()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Names are unique ignoring case, as Windows folder names are.
    pub fn same_as(&self, other: &str) -> bool {
        self.0.eq_ignore_ascii_case(other)
    }

    /// `CON`, `com1.txt`: a device name with or without an extension.
    fn is_reserved(text: &str) -> bool {
        let stem = text
            .split('.')
            .next()
            .unwrap_or_default()
            .to_ascii_lowercase();
        if Self::RESERVED.contains(&stem.as_str()) {
            return true;
        }
        let bytes = stem.as_bytes();
        bytes.len() == 4 && Self::NUMBERED.contains(&&stem[..3]) && bytes[3].is_ascii_digit()
    }
}

impl fmt::Display for ProfileName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_the_allowed_characters() {
        for name in [
            "AcmeCorp",
            "home-lab_2",
            "a",
            "v1.2",
            ".hidden",
            &"x".repeat(32),
        ] {
            assert_eq!(ProfileName::parse(name).unwrap().as_str(), name);
        }
    }

    #[test]
    fn refuses_traversal_separators_and_odd_characters() {
        for name in [
            "..",
            ".",
            "a.",
            "..\\x",
            "../x",
            "a/b",
            "a\\b",
            "C:x",
            "a b",
            "Acme Corp (old)",
            "x\0",
            "naïve",
            "a*",
        ] {
            assert!(ProfileName::parse(name).is_err(), "{name:?}");
        }
        assert_eq!(ProfileName::parse("").unwrap_err(), "Type a name");
        assert_eq!(
            ProfileName::parse(&"x".repeat(33)).unwrap_err(),
            "At most 32 characters"
        );
    }

    #[test]
    fn refuses_reserved_device_names_with_or_without_an_extension() {
        for name in [
            "CON",
            "con",
            "Prn",
            "AUX.txt",
            "nul.tar.gz",
            "COM0",
            "com9",
            "LPT1.log",
        ] {
            assert_eq!(
                ProfileName::parse(name).unwrap_err(),
                format!("{name} is a reserved Windows name")
            );
        }
        for name in ["CONSOLE", "COM10", "LPT", "comx", "auxiliary.net"] {
            assert!(ProfileName::parse(name).is_ok(), "{name}");
        }
    }

    #[test]
    fn compares_ignoring_case() {
        let name = ProfileName::parse("AcmeCorp").unwrap();
        assert!(name.same_as("acmecorp"));
        assert!(!name.same_as("AcmeCorp2"));
    }
}
