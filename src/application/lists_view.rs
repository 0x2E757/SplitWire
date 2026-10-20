use super::{ActiveProfiles, ListService, RouteGroups, StoredProfile};
use crate::ipc::{ListFile, ListLine, ListLineKind, ListsView, RouteGroupKey};
use crate::list::{CombinedListText, ListConflicts, ListSet, ListTarget, ProfileFootprint};

/// Every list as the Lists page and the profile page show it: each file line with its
/// kind, its note (a problem's reason, an override, a refused network) and the routes it holds
/// now, plus the combined text with its problems carrying both line numbers.
pub struct ListsViewBuilder<'a> {
    active: &'a ActiveProfiles,
    catalog: &'a [StoredProfile],
}

impl<'a> ListsViewBuilder<'a> {
    pub fn new(active: &'a ActiveProfiles, catalog: &'a [StoredProfile]) -> Self {
        Self { active, catalog }
    }

    /// `texts` are `(profile, routes.txt text)` in the user's order, as read now.
    pub fn view(&self, texts: &[(String, String)]) -> ListsView {
        let set = Self::set_of(texts);
        let names: Vec<&str> = texts.iter().map(|(p, _)| p.as_str()).collect();
        let text = CombinedListText::render(texts.iter().map(|(p, t)| (p.as_str(), t.as_str())));
        let problems = CombinedListText::split(&text, &names).problems;
        ListsView {
            rev: ListService::rev(texts),
            files: texts
                .iter()
                .map(|(profile, text)| ListFile {
                    name: profile.clone(),
                    lines: self.lines(profile, text, &set),
                })
                .collect(),
            text,
            problems,
        }
    }

    pub fn set_of(texts: &[(String, String)]) -> ListSet {
        ListSet::load(texts.iter().map(|(p, t)| (p.as_str(), t.as_str())))
    }

    /// One line per line of `profile`'s file, blank and comment lines included.
    pub fn lines(&self, profile: &str, text: &str, set: &ListSet) -> Vec<ListLine> {
        let footprints = self.footprints();
        let conflicts = ListConflicts::new(set, &footprints);
        let notes = conflicts.notes(profile);
        let list = set.get(profile);
        let groups = RouteGroups::new(self.active).of(profile, list);
        let routes_of = |line: usize| {
            groups
                .iter()
                .find(|g| matches!(g.key, RouteGroupKey::Line { line: l, .. } if l == line))
                .map_or(0, |g| g.rows.len() as u64)
        };
        text.lines()
            .enumerate()
            .map(|(index, raw)| {
                let line = index + 1;
                let entry = raw.split('#').next().unwrap_or_default().trim();
                let problem = list.and_then(|l| l.problems.iter().find(|p| p.line == line));
                let target = list.and_then(|l| l.entries.iter().find(|e| e.line == line));
                let (kind, note) = if raw.trim().is_empty() {
                    (ListLineKind::Blank, None)
                } else if entry.is_empty() {
                    (ListLineKind::Comment, None)
                } else if let Some(problem) = problem {
                    (ListLineKind::Problem, Some(problem.reason.clone()))
                } else if let Some(entry) = target {
                    let note = notes
                        .iter()
                        .find(|(l, _)| *l == line)
                        .map(|(_, n)| n.clone());
                    (Self::kind_of(&entry.target), note)
                } else {
                    (ListLineKind::Problem, None)
                };
                ListLine {
                    line,
                    text: raw.to_owned(),
                    kind,
                    note,
                    routes: routes_of(line),
                }
            })
            .collect()
    }

    fn kind_of(target: &ListTarget) -> ListLineKind {
        match target {
            ListTarget::Domain(_) => ListLineKind::Domain,
            ListTarget::Network(net) if net.prefix() == 32 => ListLineKind::Address,
            ListTarget::Network(_) => ListLineKind::Network,
        }
    }

    /// Each profile's endpoint addresses (known for a literal, or once connected) and tunnel.
    fn footprints(&self) -> Vec<ProfileFootprint> {
        self.catalog
            .iter()
            .filter_map(|stored| {
                let facts = stored.facts.as_ref().ok()?;
                let mut endpoints: Vec<_> = facts.endpoint.literal().into_iter().collect();
                if let Some(active) = self.active.get(stored.name.as_str()) {
                    endpoints.push(*active.spec.endpoint.ip());
                }
                endpoints.dedup();
                Some(ProfileFootprint {
                    profile: stored.name.to_string(),
                    endpoints,
                    tunnel: Some(facts.address.subnet()),
                })
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn texts() -> Vec<(String, String)> {
        vec![
            (
                "AcmeCorp".into(),
                "# acme\ncorp.example.com\n\n10.1.0.0/16 # office\n10.9.9.9\nintranet\n".into(),
            ),
            (
                "Partner".into(),
                "a.corp.example.com\ncorp.example.com\n".into(),
            ),
        ]
    }

    #[test]
    fn every_line_gets_its_kind_note_and_both_numbers() {
        let active = ActiveProfiles::default();
        let view = ListsViewBuilder::new(&active, &[]).view(&texts());
        assert!(view.text.starts_with("[profile AcmeCorp]\n# acme\n"));
        let kinds: Vec<ListLineKind> = view.files[0].lines.iter().map(|l| l.kind).collect();
        assert_eq!(
            kinds,
            [
                ListLineKind::Comment,
                ListLineKind::Domain,
                ListLineKind::Blank,
                ListLineKind::Network,
                ListLineKind::Address,
                ListLineKind::Problem,
            ]
        );
        let intranet = &view.files[0].lines[5];
        assert_eq!(
            intranet.note.as_deref(),
            Some("a domain needs at least 2 labels")
        );
        let overrides = &view.files[1].lines[0];
        assert_eq!(
            overrides.note.as_deref(),
            Some("overrides AcmeCorp line 2 while both are connected")
        );
        let duplicate = &view.files[1].lines[1];
        assert_eq!(duplicate.kind, ListLineKind::Problem);
        assert!(
            duplicate
                .note
                .as_deref()
                .unwrap()
                .starts_with("also in AcmeCorp line 2")
        );
        let problem = view
            .problems
            .iter()
            .find(|p| p.profile.as_deref() == Some("Partner"))
            .unwrap();
        assert_eq!((problem.text_line, problem.file_line), (Some(11), Some(2)));
        assert_eq!(view.rev, ListService::rev(&texts()));
        assert!(
            view.files
                .iter()
                .flat_map(|f| &f.lines)
                .all(|l| l.routes == 0)
        );
    }
}
