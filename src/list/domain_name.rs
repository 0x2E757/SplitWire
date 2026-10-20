use std::fmt;

/// A validated, lower-case DNS name without the trailing dot; it stands for itself and every subdomain.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct DomainName(String);

impl DomainName {
    pub fn parse(text: &str) -> Result<Self, String> {
        let name = text.strip_suffix('.').unwrap_or(text).to_ascii_lowercase();
        if name.starts_with("*.") {
            return Err("write the domain without '*.'; subdomains are always included".into());
        }
        if name.starts_with('.') {
            return Err("remove the leading dot".into());
        }
        if !name.is_ascii() {
            return Err("only ASCII names are supported (write IDNs as xn--)".into());
        }
        if name.len() > 253 {
            return Err("name is longer than 253 characters".into());
        }
        let labels: Vec<&str> = name.split('.').collect();
        if labels.len() < 2 {
            return Err("a domain needs at least two labels, e.g. example.com".into());
        }
        for label in &labels {
            Self::check_label(label)?;
        }
        if labels
            .last()
            .is_some_and(|top| top.bytes().all(|b| b.is_ascii_digit()))
        {
            return Err("the top-level label cannot be numeric".into());
        }
        Ok(Self(name))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    fn check_label(label: &str) -> Result<(), String> {
        if label.is_empty() {
            return Err("empty label (two dots in a row)".into());
        }
        if label.len() > 63 {
            return Err(format!("label '{label}' is longer than 63 characters"));
        }
        if let Some(bad) = label
            .chars()
            .find(|c| !(c.is_ascii_alphanumeric() || *c == '-' || *c == '_'))
        {
            return Err(format!("character '{bad}' is not allowed in a domain"));
        }
        if label.starts_with('-') || label.ends_with('-') {
            return Err(format!("label '{label}' starts or ends with '-'"));
        }
        Ok(())
    }
}

impl fmt::Display for DomainName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_case_and_trailing_dot() {
        assert_eq!(
            DomainName::parse("Contoso.EXAMPLE.").unwrap().as_str(),
            "contoso.example"
        );
    }

    #[test]
    fn rejects_invalid_names() {
        let long_label = format!("{}.com", "a".repeat(64));
        for bad in [
            "localhost",
            "*.example.com",
            ".example.com",
            "exa mple.com",
            "a..com",
            "-a.com",
            "a-.com",
            "1.2.3",
            "b\u{fc}cher.de",
            long_label.as_str(),
        ] {
            assert!(DomainName::parse(bad).is_err(), "{bad} should be rejected");
        }
    }

    #[test]
    fn accepts_hyphens_underscores_and_digits() {
        assert!(DomainName::parse("my-host_1.region-1.example.com").is_ok());
    }
}
