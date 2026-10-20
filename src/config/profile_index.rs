use super::KeyValueFile;
use crate::profile::{Pin, ProfileName};

/// `profiles\index.conf`: the user's order of the profiles and their colours, one
/// `<name> = <pin 1–6>` line each. The service owns it, so reading it is lenient: a bad line is
/// dropped, and `reconcile` makes it match the profile folders.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ProfileIndex {
    entries: Vec<(ProfileName, Pin)>,
}

impl ProfileIndex {
    pub const FILE: &'static str = "index.conf";

    pub fn parse(text: &str) -> Self {
        let mut index = Self::default();
        for entry in KeyValueFile::parse(Self::FILE, text).entries {
            if let (Ok(name), Some(pin)) =
                (ProfileName::parse(&entry.key), Pin::parse(&entry.value))
                && index.position(name.as_str()).is_none()
            {
                index.entries.push((name, pin));
            }
        }
        index
    }

    pub fn render(&self) -> String {
        self.entries
            .iter()
            .map(|(name, pin)| format!("{name} = {pin}\n"))
            .collect()
    }

    pub fn entries(&self) -> &[(ProfileName, Pin)] {
        &self.entries
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// The position of `name`, ignoring case.
    pub fn position(&self, name: &str) -> Option<usize> {
        self.entries.iter().position(|(n, _)| n.same_as(name))
    }

    /// Drops names without a folder and appends folders without a line, with the first unused pin.
    /// True when anything changed.
    pub fn reconcile(&mut self, existing: &[ProfileName]) -> bool {
        let before = self.entries.clone();
        self.entries
            .retain(|(name, _)| existing.iter().any(|e| e.same_as(name.as_str())));
        for name in existing {
            if self.position(name.as_str()).is_none() {
                self.push(name.clone(), self.first_unused());
            }
        }
        self.entries != before
    }

    pub fn first_unused(&self) -> Pin {
        Pin::first_unused(self.entries.iter().map(|(_, pin)| *pin))
    }

    pub fn push(&mut self, name: ProfileName, pin: Pin) {
        self.entries.push((name, pin));
    }

    pub fn remove(&mut self, name: &str) -> bool {
        let before = self.entries.len();
        self.entries.retain(|(n, _)| !n.same_as(name));
        self.entries.len() != before
    }

    pub fn set_pin(&mut self, name: &str, pin: Pin) -> bool {
        match self.position(name) {
            Some(at) => {
                self.entries[at].1 = pin;
                true
            }
            None => false,
        }
    }

    /// Moves `name` to `index`, clamped to the end.
    pub fn move_to(&mut self, name: &str, index: usize) -> bool {
        let Some(at) = self.position(name) else {
            return false;
        };
        let entry = self.entries.remove(at);
        let index = index.min(self.entries.len());
        self.entries.insert(index, entry);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(index: &ProfileIndex) -> Vec<String> {
        index
            .entries()
            .iter()
            .map(|(name, pin)| format!("{name}={pin}"))
            .collect()
    }

    fn profile(name: &str) -> ProfileName {
        ProfileName::parse(name).unwrap()
    }

    #[test]
    fn parses_leniently_and_renders() {
        let index =
            ProfileIndex::parse("Contoso = 3\nAcmeCorp=1\nbad name = 2\nX = 9\nacmecorp = 4\n");
        assert_eq!(names(&index), ["Contoso=3", "AcmeCorp=1"]);
        assert_eq!(index.render(), "Contoso = 3\nAcmeCorp = 1\n");
    }

    #[test]
    fn reconcile_drops_unknown_and_appends_missing_with_free_pins() {
        let mut index = ProfileIndex::parse("Gone = 1\nAcmeCorp = 2\n");
        assert!(index.reconcile(&[profile("AcmeCorp"), profile("HomeLab"), profile("Partner")]));
        assert_eq!(names(&index), ["AcmeCorp=2", "HomeLab=1", "Partner=3"]);
        assert!(!index.reconcile(&[profile("acmecorp"), profile("HomeLab"), profile("Partner")]));
    }

    #[test]
    fn moves_recolours_and_removes() {
        let mut index = ProfileIndex::parse("A = 1\nB = 2\nC = 3\n");
        assert!(index.move_to("c", 0));
        assert_eq!(names(&index), ["C=3", "A=1", "B=2"]);
        assert!(index.move_to("C", 99));
        assert_eq!(names(&index), ["A=1", "B=2", "C=3"]);
        assert!(index.set_pin("b", Pin::new(6).unwrap()));
        assert!(index.remove("A"));
        assert_eq!(names(&index), ["B=6", "C=3"]);
        assert!(!index.remove("A"));
        assert!(!index.move_to("A", 0));
        assert_eq!(index.first_unused().number(), 1);
    }
}
