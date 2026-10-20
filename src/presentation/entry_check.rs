//! Checks an entry as it is typed into a profile's Add field: the same `list` rules
//! and wording as the service, against every list the window last received.

use crate::ipc::ListsView;
use crate::list::{ListConflicts, ListSet, ListTarget, ProfileFootprint, RoutesList};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EntryVerdict {
    Empty,
    /// Can be added; the note says how it relates to the other entries.
    Ok(Option<String>),
    /// Not an entry at all.
    Bad(String),
    /// Listed already; `other` is the other profile's line it collides with.
    Duplicate {
        reason: String,
        other: Option<(String, usize)>,
    },
}

impl EntryVerdict {
    pub fn blocks(&self) -> bool {
        !matches!(self, Self::Ok(_))
    }

    /// The hint under the field, if any.
    pub fn message(&self) -> Option<&str> {
        match self {
            Self::Empty | Self::Ok(None) => None,
            Self::Ok(Some(note)) => Some(note),
            Self::Bad(reason) | Self::Duplicate { reason, .. } => Some(reason),
        }
    }

    pub fn bad(&self) -> bool {
        matches!(self, Self::Bad(_) | Self::Duplicate { .. })
    }
}

pub struct EntryCheck {
    set: ListSet,
}

impl EntryCheck {
    pub fn new(lists: Option<&ListsView>) -> Self {
        let texts: Vec<(String, String)> = lists
            .map(|view| {
                view.files
                    .iter()
                    .map(|f| {
                        let text: Vec<&str> = f.lines.iter().map(|l| l.text.as_str()).collect();
                        (f.name.clone(), text.join("\n"))
                    })
                    .collect()
            })
            .unwrap_or_default();
        Self {
            set: ListSet::load(texts.iter().map(|(n, t)| (n.as_str(), t.as_str()))),
        }
    }

    pub fn check(&self, profile: &str, text: &str) -> EntryVerdict {
        if text.trim().is_empty() {
            return EntryVerdict::Empty;
        }
        let target: ListTarget = match RoutesList::parse_target(text) {
            Ok(target) => target,
            Err(reason) => return EntryVerdict::Bad(reason),
        };
        if let Err(reason) = self.set.check_add(profile, text) {
            let other = self
                .set
                .owner(&target)
                .filter(|(owner, _)| !owner.eq_ignore_ascii_case(profile))
                .map(|(owner, line)| (owner.to_owned(), line));
            return EntryVerdict::Duplicate { reason, other };
        }
        let footprints: [ProfileFootprint; 0] = [];
        EntryVerdict::Ok(ListConflicts::new(&self.set, &footprints).note(profile, &target, None))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::presentation::samples::Samples;

    #[test]
    fn verdicts_use_the_list_rules_wording() {
        let lists = Samples::lists("normal");
        let check = EntryCheck::new(Some(&lists));
        assert_eq!(check.check("AcmeCorp", "  "), EntryVerdict::Empty);
        assert_eq!(
            check.check("AcmeCorp", "intranet"),
            EntryVerdict::Bad("a domain needs at least 2 labels".into())
        );
        assert_eq!(
            check.check("AcmeCorp", "fd00::1"),
            EntryVerdict::Bad("IPv6 is not supported".into())
        );
        assert_eq!(
            check.check("AcmeCorp", "contoso.example"),
            EntryVerdict::Duplicate {
                reason: "also in Contoso line 2: a name belongs to one profile".into(),
                other: Some(("Contoso".into(), 2)),
            }
        );
        assert_eq!(
            check.check("AcmeCorp", "acme.example").message(),
            Some("already line 1 of AcmeCorp")
        );
        assert_eq!(
            check.check("AcmeCorp", "wiki.acme.example"),
            EntryVerdict::Ok(Some("inside acme.example (line 1)".into()))
        );
        assert_eq!(
            check.check("AcmeCorp", "jira.contoso.example"),
            EntryVerdict::Ok(Some(
                "overrides Contoso line 2 while both are connected".into()
            ))
        );
        assert_eq!(
            check.check("AcmeCorp", "example.org"),
            EntryVerdict::Ok(None)
        );
        assert!(EntryVerdict::Empty.blocks());
    }
}
