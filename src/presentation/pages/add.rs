//! Add profile and the first run: a drop zone filling the page, three fact cards. Dragging
//! files over it names the profiles to be made; files that cannot be read are listed with why.

use crate::ipc::StatusReport;
use crate::presentation::UnreadFile;
use crate::profile::ImportPreview;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DropState {
    Idle,
    /// Files are over the zone; their names when known.
    Over(Vec<String>),
    /// Some dropped files could not be read: `(file, outcome, readable)`.
    Bad(Vec<(String, String, bool)>),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Fact {
    pub icon: &'static str,
    pub title: String,
    pub text: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AddView {
    pub first_run: bool,
    pub title: String,
    pub subtitle: String,
    pub state: DropState,
    pub zone_title: String,
    pub zone_text: String,
    /// The limit of 16 profiles is reached.
    pub full: bool,
    /// Some files of a bad drop can still be reviewed.
    pub review_others: bool,
    pub facts: Vec<Fact>,
}

impl AddView {
    pub fn of(
        status: &StatusReport,
        dragging: Option<&[String]>,
        unread: &[UnreadFile],
        readable: &[String],
    ) -> Self {
        let count = status.profiles.len();
        let first_run = count == 0;
        let names = status
            .profiles
            .iter()
            .map(|p| p.name.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        let fact = |icon, title: &str, text: &str| Fact {
            icon,
            title: title.into(),
            text: text.into(),
        };
        let facts = if first_run {
            vec![
                fact(
                    "lock",
                    "The keys stay here",
                    "Encrypted for this machine; never shown, logged or sent.",
                ),
                fact(
                    "tunnel",
                    "One .conf, one profile",
                    "Each gets its own tunnel, DNS and list. Up to 16, connected side by side.",
                ),
                fact(
                    "power",
                    "You connect",
                    "Nothing is routed until you switch a profile on.",
                ),
            ]
        } else {
            vec![
                fact(
                    "tunnel",
                    "Its own tunnel",
                    "Adapter SplitWire-<name> with its own keys, endpoint and DNS.",
                ),
                fact(
                    "list",
                    "Its own list",
                    "Starts empty. A name goes to the connected profile with the most specific match.",
                ),
                fact(
                    "shield",
                    "Checked before saving",
                    &format!("Name, tunnel address and endpoint are compared with {names}."),
                ),
            ]
        };
        let (state, zone_title, zone_text) = if let Some(names) = dragging {
            (
                DropState::Over(names.to_vec()),
                match names.len() {
                    0 => "Release to add the files".to_owned(),
                    1 => "Release to add 1 profile".to_owned(),
                    n => format!("Release to add {n} profiles"),
                },
                "You will see what each file contains, and what is ignored, before anything is saved.".to_owned(),
            )
        } else if !unread.is_empty() {
            let total = unread.len() + readable.len();
            let mut rows: Vec<(String, String, bool)> = unread
                .iter()
                .map(|u| (u.file_name.clone(), u.reason.clone(), false))
                .collect();
            rows.extend(
                readable
                    .iter()
                    .map(|f| (f.clone(), "Ready to review".to_owned(), true)),
            );
            (
                DropState::Bad(rows),
                format!("{} of {total} files cannot be imported", unread.len()),
                if readable.is_empty() {
                    "Fix the files and drop them again.".to_owned()
                } else {
                    "Fix them, or review the others on their own.".to_owned()
                },
            )
        } else {
            (
                DropState::Idle,
                "Drop WireGuard .conf files here".to_owned(),
                if first_run {
                    "Each file becomes a profile named after it. SplitWire sends only the names on a profile’s list through its tunnel; everything else stays on your normal connection.".to_owned()
                } else {
                    "Each file becomes a new profile named after it. A file named like an existing profile replaces it, and you see the differences first.".to_owned()
                },
            )
        };
        Self {
            first_run,
            title: if first_run {
                "Welcome to SplitWire".into()
            } else {
                "Add profile".into()
            },
            subtitle: if first_run {
                "No profile yet".into()
            } else {
                format!("{count} of {} profiles in use", ImportPreview::LIMIT)
            },
            review_others: !readable.is_empty() && !unread.is_empty(),
            state,
            zone_title,
            zone_text,
            full: count >= ImportPreview::LIMIT,
            facts,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::presentation::samples::Samples;

    #[test]
    fn the_first_run_welcomes_and_a_drag_names_the_profiles() {
        let empty = Samples::status("firstrun");
        let view = AddView::of(&empty, None, &[], &[]);
        assert!(view.first_run);
        assert_eq!(view.title, "Welcome to SplitWire");
        assert_eq!(view.facts[0].title, "The keys stay here");
        let status = Samples::status("normal");
        let names = ["Partner".to_owned(), "Contoso".to_owned()];
        let over = AddView::of(&status, Some(&names), &[], &[]);
        assert_eq!(over.zone_title, "Release to add 2 profiles");
        assert_eq!(over.subtitle, "3 of 16 profiles in use");
        assert!(over.facts[2].text.ends_with("Contoso, AcmeCorp, HomeLab."));
    }

    #[test]
    fn unreadable_files_are_listed_with_the_others() {
        let status = Samples::status("normal");
        let unread = [UnreadFile {
            file_name: "big.conf".into(),
            reason: "larger than 64 KB".into(),
        }];
        let view = AddView::of(&status, None, &unread, &["Partner.conf".into()]);
        assert_eq!(view.zone_title, "1 of 2 files cannot be imported");
        assert!(view.review_others);
        assert!(matches!(&view.state, DropState::Bad(rows) if rows.len() == 2 && !rows[0].2));
    }
}
