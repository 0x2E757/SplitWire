use std::fmt;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::io;
use std::path::PathBuf;

use super::DataFolder;
use crate::list::{CombinedListText, ListSet, TextProblem};
use crate::profile::ProfileName;

/// Why a list change wrote nothing.
#[derive(Debug)]
pub enum ListRefusal {
    /// The files changed since `base_rev` (a hand edit): the caller reloads.
    Stale,
    UnknownProfile(String),
    Problems(Vec<TextProblem>),
    Io(io::Error),
}

impl fmt::Display for ListRefusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ListRefusal::Stale => f.write_str("the lists changed on disk; reloaded"),
            ListRefusal::UnknownProfile(name) => write!(f, "unknown profile {name}"),
            ListRefusal::Problems(problems) => {
                let lines: Vec<String> = problems.iter().map(ToString::to_string).collect();
                f.write_str(&lines.join("; "))
            }
            ListRefusal::Io(error) => error.fmt(f),
        }
    }
}

impl From<io::Error> for ListRefusal {
    fn from(error: io::Error) -> Self {
        ListRefusal::Io(error)
    }
}

/// The list files on disk: their revision, and the edits: one entry added or
/// removed, one file or the combined text saved. Each is checked against every list, refused
/// whole when the files changed since `base_rev`, and only changed files are written.
pub struct ListService {
    folder: DataFolder,
}

impl ListService {
    pub fn new(folder: DataFolder) -> Self {
        Self { folder }
    }

    /// `(profile, routes.txt text)` in the user's order.
    pub fn texts(&self, profiles: &[ProfileName]) -> io::Result<Vec<(String, String)>> {
        profiles
            .iter()
            .map(|name| {
                Ok((
                    name.to_string(),
                    self.folder.read_text(&self.folder.list(name))?,
                ))
            })
            .collect()
    }

    /// The revision of the lists: it changes with any file's text.
    pub fn rev(texts: &[(String, String)]) -> u64 {
        let mut hasher = DefaultHasher::new();
        texts.hash(&mut hasher);
        hasher.finish()
    }

    /// Appends `entry` to `name`'s list.
    pub fn add(
        &self,
        profiles: &[ProfileName],
        base_rev: u64,
        name: &ProfileName,
        entry: &str,
    ) -> Result<(), ListRefusal> {
        let texts = self.current(profiles, base_rev)?;
        let set = ListSet::load(texts.iter().map(|(p, t)| (p.as_str(), t.as_str())));
        set.check_add(name.as_str(), entry).map_err(|reason| {
            ListRefusal::Problems(vec![TextProblem {
                text_line: None,
                profile: Some(name.to_string()),
                file_line: None,
                text: entry.trim().to_owned(),
                reason,
                other: None,
                blocking: true,
            }])
        })?;
        let mut text = Self::text_of(&texts, name).to_owned();
        if !text.is_empty() && !text.ends_with('\n') {
            text.push('\n');
        }
        text.push_str(entry.trim());
        text.push('\n');
        Ok(self
            .folder
            .write(&self.folder.list(name), text.as_bytes())?)
    }

    /// Removes line `line` of `name`'s list, which must still read `entry`.
    pub fn remove(
        &self,
        profiles: &[ProfileName],
        base_rev: u64,
        name: &ProfileName,
        line: usize,
        entry: &str,
    ) -> Result<(), ListRefusal> {
        let texts = self.current(profiles, base_rev)?;
        let old = Self::text_of(&texts, name);
        let mut lines: Vec<&str> = old.lines().collect();
        let matches = line
            .checked_sub(1)
            .and_then(|at| lines.get(at))
            .is_some_and(|text| text.trim() == entry.trim());
        if !matches {
            return Err(ListRefusal::Stale);
        }
        lines.remove(line - 1);
        let text: String = lines.iter().map(|l| format!("{l}\n")).collect();
        Ok(self
            .folder
            .write(&self.folder.list(name), text.as_bytes())?)
    }

    /// Replaces `name`'s list with `text`.
    pub fn save(
        &self,
        profiles: &[ProfileName],
        base_rev: u64,
        name: &ProfileName,
        text: &str,
    ) -> Result<(), ListRefusal> {
        let texts = self.current(profiles, base_rev)?;
        let combined = CombinedListText::render(texts.iter().map(|(p, t)| {
            let t = if p == name.as_str() { text } else { t.as_str() };
            (p.as_str(), t)
        }));
        self.write_split(profiles, &texts, &combined)
    }

    /// Splits the combined text back into the files and writes the changed ones, all or none.
    pub fn save_all(
        &self,
        profiles: &[ProfileName],
        base_rev: u64,
        combined: &str,
    ) -> Result<(), ListRefusal> {
        let texts = self.current(profiles, base_rev)?;
        self.write_split(profiles, &texts, combined)
    }

    fn write_split(
        &self,
        profiles: &[ProfileName],
        current: &[(String, String)],
        combined: &str,
    ) -> Result<(), ListRefusal> {
        let names: Vec<&str> = profiles.iter().map(ProfileName::as_str).collect();
        let split = CombinedListText::split(combined, &names);
        if split.refused() {
            let blocking = split.problems.into_iter().filter(|p| p.blocking).collect();
            return Err(ListRefusal::Problems(blocking));
        }
        let current: Vec<(&str, &str)> = current
            .iter()
            .map(|(p, t)| (p.as_str(), t.as_str()))
            .collect();
        let files: Vec<(PathBuf, Vec<u8>)> = split
            .changed(&current)
            .into_iter()
            .filter_map(|(profile, text)| {
                let name = profiles.iter().find(|p| p.as_str() == profile)?;
                Some((self.folder.list(name), text.clone().into_bytes()))
            })
            .collect();
        Ok(self.folder.write_all(&files)?)
    }

    fn current(
        &self,
        profiles: &[ProfileName],
        base_rev: u64,
    ) -> Result<Vec<(String, String)>, ListRefusal> {
        let texts = self.texts(profiles)?;
        if Self::rev(&texts) != base_rev {
            return Err(ListRefusal::Stale);
        }
        Ok(texts)
    }

    fn text_of<'t>(texts: &'t [(String, String)], name: &ProfileName) -> &'t str {
        texts
            .iter()
            .find(|(p, _)| p == name.as_str())
            .map_or("", |(_, t)| t.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::data_folder::tests::scratch;
    use std::fs;

    struct Rig {
        folder: DataFolder,
        service: ListService,
        names: Vec<ProfileName>,
    }

    fn rig(name: &str) -> Rig {
        let folder = scratch(name);
        let names: Vec<ProfileName> = ["AcmeCorp", "Contoso"]
            .iter()
            .map(|n| ProfileName::parse(n).unwrap())
            .collect();
        for (name, text) in names
            .iter()
            .zip(["acme.example\n", "# contoso\ncontoso.example\n"])
        {
            fs::create_dir_all(folder.profile(name)).unwrap();
            fs::write(folder.list(name), text).unwrap();
        }
        Rig {
            service: ListService::new(folder.clone()),
            folder,
            names,
        }
    }

    impl Rig {
        fn rev(&self) -> u64 {
            ListService::rev(&self.service.texts(&self.names).unwrap())
        }

        fn text(&self, nth: usize) -> String {
            fs::read_to_string(self.folder.list(&self.names[nth])).unwrap()
        }
    }

    impl Drop for Rig {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(self.folder.root());
        }
    }

    #[test]
    fn adds_and_removes_one_entry_checked_against_every_list() {
        let rig = rig("lists-edit");
        let acme = &rig.names[0];
        rig.service
            .add(&rig.names, rig.rev(), acme, " intranet.acme.example ")
            .unwrap();
        assert_eq!(rig.text(0), "acme.example\nintranet.acme.example\n");
        match rig
            .service
            .add(&rig.names, rig.rev(), acme, "contoso.example")
        {
            Err(ListRefusal::Problems(problems)) => assert_eq!(
                problems[0].reason,
                "also in Contoso line 2: a name belongs to one profile"
            ),
            other => panic!("{other:?}"),
        }
        assert!(matches!(
            rig.service
                .remove(&rig.names, rig.rev(), acme, 1, "other.example"),
            Err(ListRefusal::Stale)
        ));
        rig.service
            .remove(&rig.names, rig.rev(), acme, 1, "acme.example")
            .unwrap();
        assert_eq!(rig.text(0), "intranet.acme.example\n");
    }

    #[test]
    fn a_hand_edit_in_between_makes_the_change_stale() {
        let rig = rig("lists-stale");
        let rev = rig.rev();
        fs::write(rig.folder.list(&rig.names[1]), "contoso.example\n").unwrap();
        let refused = rig
            .service
            .add(&rig.names, rev, &rig.names[0], "new.example");
        assert_eq!(
            refused.unwrap_err().to_string(),
            "the lists changed on disk; reloaded"
        );
        assert_eq!(rig.text(0), "acme.example\n");
    }

    #[test]
    fn saves_one_file_or_the_combined_text_all_or_none() {
        let rig = rig("lists-save");
        rig.service
            .save(
                &rig.names,
                rig.rev(),
                &rig.names[0],
                "acme.example\n10.88.0.0/16\n",
            )
            .unwrap();
        assert_eq!(rig.text(0), "acme.example\n10.88.0.0/16\n");
        let duplicate = "[profile AcmeCorp]\nacme.example\n\n[profile Contoso]\nacme.example\n";
        match rig.service.save_all(&rig.names, rig.rev(), duplicate) {
            Err(ListRefusal::Problems(problems)) => {
                assert_eq!(
                    problems[0].to_string(),
                    "text line 5 (Contoso line 1): also in AcmeCorp line 1: a name belongs to one profile"
                );
            }
            other => panic!("{other:?}"),
        }
        assert_eq!(rig.text(1), "# contoso\ncontoso.example\n");
        let good = "[profile AcmeCorp]\nacme.example\n\n[profile Contoso]\ncontoso.example\n";
        rig.service.save_all(&rig.names, rig.rev(), good).unwrap();
        assert_eq!(rig.text(1), "contoso.example\n");
        assert_eq!(rig.text(0), "acme.example\n");
    }
}
