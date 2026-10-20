use super::KeyValueFile;

/// `splitwire.state`: `connected=<name>,<name>`, the profiles to restore at start. The service owns
/// it; names are kept as written, and restore checks them against the profiles.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct StateFile {
    connected: Vec<String>,
}

impl StateFile {
    pub const FILE: &'static str = "splitwire.state";

    pub fn parse(text: &str) -> Self {
        let file = KeyValueFile::parse(Self::FILE, text);
        let mut state = Self::default();
        if let Some(entry) = file.get("connected") {
            for name in entry
                .value
                .split(',')
                .map(str::trim)
                .filter(|n| !n.is_empty())
            {
                state.add(name);
            }
        }
        state
    }

    pub fn render(&self) -> String {
        format!("connected={}\n", self.connected.join(","))
    }

    pub fn connected(&self) -> &[String] {
        &self.connected
    }

    pub fn contains(&self, name: &str) -> bool {
        self.connected.iter().any(|n| n.eq_ignore_ascii_case(name))
    }

    /// True when it was not there yet.
    pub fn add(&mut self, name: &str) -> bool {
        if self.contains(name) {
            return false;
        }
        self.connected.push(name.to_owned());
        true
    }

    pub fn remove(&mut self, name: &str) -> bool {
        let before = self.connected.len();
        self.connected.retain(|n| !n.eq_ignore_ascii_case(name));
        self.connected.len() != before
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_the_connected_names() {
        let mut state = StateFile::parse("connected=AcmeCorp, Contoso,,acmecorp\n");
        assert_eq!(state.connected(), ["AcmeCorp", "Contoso"]);
        assert!(state.contains("contoso"));
        assert!(state.add("HomeLab"));
        assert!(!state.add("homelab"));
        assert!(state.remove("ACMECORP"));
        assert_eq!(state.render(), "connected=Contoso,HomeLab\n");
        assert_eq!(StateFile::parse(&state.render()), state);
    }

    #[test]
    fn a_missing_or_damaged_file_is_empty() {
        assert!(StateFile::parse("").connected().is_empty());
        assert!(StateFile::parse("garbage\n").connected().is_empty());
        assert_eq!(StateFile::default().render(), "connected=\n");
    }
}
