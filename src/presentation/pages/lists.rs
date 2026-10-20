//! Lists: every list in one view. Rows: a group per profile with its lines, notes, route
//! counts, remove and an Add field checked against all lists as typed. Text: the combined text,
//! refused whole with both line numbers on every problem.

use super::InfoBar;
use crate::ipc::{ListLineKind, ListsView, ProfileState, StatusReport};
use crate::list::TextProblem;
use crate::presentation::{Dot, EntryCheck, EntryVerdict, Format, StateText};
use crate::profile::Pin;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ListRowView {
    pub line: usize,
    pub text: String,
    pub comment: bool,
    pub domain: bool,
    pub note: Option<String>,
    /// The entry typed for another profile collides with this line.
    pub clash: bool,
    pub routes: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ListGroupView {
    pub name: String,
    pub colour: Pin,
    pub dot: Dot,
    pub caption: String,
    pub open: bool,
    pub rows: Vec<ListRowView>,
    pub add_value: String,
    pub add_verdict: EntryVerdict,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ListsTextView {
    /// The text the editor starts with: the combined text, or the user's draft.
    pub text: String,
    pub dirty: bool,
    /// Text lines with a blocking problem of the last refused save.
    pub bad_lines: Vec<usize>,
    pub section_lines: Vec<usize>,
    pub line_count: usize,
    /// "Save refused: nothing was written" and its problems.
    pub refused: Option<InfoBar>,
    /// Skipped lines of the saved files.
    pub skipped: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ListsPageView {
    pub subtitle: String,
    pub message: Option<InfoBar>,
    pub groups: Vec<ListGroupView>,
    pub text: ListsTextView,
}

pub struct ListsSource<'a> {
    pub status: &'a StatusReport,
    pub lists: &'a ListsView,
    pub search: &'a str,
    pub adds: &'a dyn Fn(&str) -> String,
    pub collapsed: &'a dyn Fn(&str) -> bool,
    /// The user's edited combined text, if any.
    pub draft: Option<&'a str>,
    pub refused: &'a [TextProblem],
}

impl ListsSource<'_> {
    pub fn view(&self) -> ListsPageView {
        let check = EntryCheck::new(Some(self.lists));
        let global_invalid = !self.status.problems.is_empty();
        let verdicts: Vec<(String, String, EntryVerdict)> = self
            .lists
            .files
            .iter()
            .map(|f| {
                let value = (self.adds)(&f.name);
                let verdict = check.check(&f.name, &value);
                (f.name.clone(), value, verdict)
            })
            .collect();
        let clashes: Vec<(String, usize)> = verdicts
            .iter()
            .filter_map(|(_, _, v)| match v {
                EntryVerdict::Duplicate { other: Some(o), .. } => Some(o.clone()),
                _ => None,
            })
            .collect();
        let query = self.search.trim().to_lowercase();
        let total: usize = self
            .lists
            .files
            .iter()
            .map(|f| f.lines.iter().filter(|l| Self::entry(l.kind)).count())
            .sum();
        let groups = self
            .lists
            .files
            .iter()
            .zip(verdicts)
            .filter_map(|(file, (_, add_value, add_verdict))| {
                let profile = self.status.profiles.iter().find(|p| p.name == file.name)?;
                let entries = file.lines.iter().filter(|l| Self::entry(l.kind)).count();
                let connected = matches!(profile.state, ProfileState::Connected(_));
                let entry_text = Format::plural(entries, "entry", "entries");
                let caption = match &profile.state {
                    ProfileState::Connected(_) => format!("{entry_text} · connected"),
                    _ if global_invalid => {
                        format!("{entry_text} · off (splitwire.conf is invalid): not in use")
                    }
                    ProfileState::Blocked => format!("{entry_text} · can’t connect: not in use"),
                    ProfileState::Failed => format!("{entry_text} · connect failed: not in use"),
                    _ => format!("{entry_text} · off: not in use"),
                };
                let rows = file
                    .lines
                    .iter()
                    .filter(|l| l.kind != ListLineKind::Blank)
                    .filter(|l| query.is_empty() || l.text.to_lowercase().contains(&query))
                    .map(|l| ListRowView {
                        line: l.line,
                        text: l.text.clone(),
                        comment: l.kind == ListLineKind::Comment,
                        domain: l.kind == ListLineKind::Domain,
                        note: l.note.clone().or_else(|| {
                            matches!(l.kind, ListLineKind::Address | ListLineKind::Network)
                                .then(|| "static".to_owned())
                        }),
                        clash: clashes.contains(&(file.name.clone(), l.line)),
                        routes: if connected && Self::entry(l.kind) {
                            l.routes.to_string()
                        } else {
                            "—".into()
                        },
                    })
                    .collect();
                Some(ListGroupView {
                    name: file.name.clone(),
                    colour: profile.colour,
                    dot: StateText::dot(&profile.state),
                    caption,
                    open: !(self.collapsed)(&file.name),
                    rows,
                    add_value,
                    add_verdict,
                })
            })
            .collect();
        ListsPageView {
            subtitle: format!(
                "{} · {} · changes apply at once to connected profiles",
                Format::plural(self.lists.files.len(), "profile", "profiles"),
                Format::plural(total, "entry", "entries")
            ),
            message: global_invalid.then(|| {
                InfoBar::new(
                    "splitwire.conf is invalid",
                    "Every profile is disconnected, so no list is in use. The lists can still be edited.",
                )
            }),
            groups,
            text: self.text(),
        }
    }

    fn entry(kind: ListLineKind) -> bool {
        !matches!(kind, ListLineKind::Comment | ListLineKind::Blank)
    }

    fn text(&self) -> ListsTextView {
        let text = self.draft.unwrap_or(&self.lists.text).to_owned();
        let lines: Vec<&str> = text.lines().collect();
        let mut bad_lines: Vec<usize> = self.refused.iter().flat_map(|p| p.text_line).collect();
        bad_lines.sort_unstable();
        bad_lines.dedup();
        ListsTextView {
            dirty: self.draft.is_some_and(|d| d != self.lists.text),
            section_lines: lines
                .iter()
                .enumerate()
                .filter(|(_, l)| l.trim_start().starts_with("[profile "))
                .map(|(i, _)| i + 1)
                .collect(),
            line_count: lines.len().max(1),
            refused: (!self.refused.is_empty()).then(|| {
                InfoBar::new(
                    "Save refused: nothing was written",
                    &self
                        .refused
                        .iter()
                        .map(ToString::to_string)
                        .collect::<Vec<_>>()
                        .join(" · "),
                )
            }),
            skipped: self
                .lists
                .problems
                .iter()
                .filter(|p| !p.blocking)
                .map(|p| format!("{p}; the line is skipped"))
                .collect(),
            bad_lines,
            text,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::presentation::samples::Samples;

    #[test]
    fn rows_group_by_profile_and_typing_marks_a_clash() {
        let status = Samples::status("normal");
        let lists = Samples::lists("normal");
        let adds = |name: &str| {
            if name == "AcmeCorp" {
                "contoso.example".to_owned()
            } else {
                String::new()
            }
        };
        let open = |_: &str| false;
        let view = ListsSource {
            status: &status,
            lists: &lists,
            search: "",
            adds: &adds,
            collapsed: &open,
            draft: None,
            refused: &[],
        }
        .view();
        assert_eq!(
            view.subtitle,
            "3 profiles · 9 entries · changes apply at once to connected profiles"
        );
        let contoso = &view.groups[0];
        assert_eq!(contoso.caption, "4 entries · connected");
        assert!(contoso.rows[0].comment);
        assert_eq!(contoso.rows[1].routes, "8");
        assert!(contoso.rows[1].clash);
        assert_eq!(contoso.rows[3].note.as_deref(), Some("static"));
        assert_eq!(view.groups[1].caption, "4 entries · connected");
        assert!(view.groups[1].add_verdict.bad());
        assert_eq!(
            view.groups[2].caption,
            "1 entry · can’t connect: not in use"
        );
        assert_eq!(view.groups[2].rows[0].routes, "—");
        assert_eq!(view.text.section_lines, [1, 8, 14]);
        assert!(!view.text.dirty);
    }

    #[test]
    fn a_refused_save_marks_its_lines() {
        let status = Samples::status("normal");
        let lists = Samples::lists("normal");
        let none = |_: &str| String::new();
        let open = |_: &str| false;
        let refused = [TextProblem {
            text_line: Some(9),
            profile: Some("AcmeCorp".into()),
            file_line: Some(1),
            text: "acme.example".into(),
            reason: "also in Contoso line 2: a name belongs to one profile".into(),
            other: None,
            blocking: true,
        }];
        let view = ListsSource {
            status: &status,
            lists: &lists,
            search: "",
            adds: &none,
            collapsed: &open,
            draft: Some("[profile X]\n"),
            refused: &refused,
        }
        .view();
        assert!(view.text.dirty);
        assert_eq!(view.text.bad_lines, [9]);
        assert!(
            view.text
                .refused
                .unwrap()
                .message
                .starts_with("text line 9 (AcmeCorp line 1)")
        );
    }
}
