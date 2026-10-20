//! Every list as the service answers `GetAllLists`: each line with its kind, note and route count,
//! built with the domain's own list rules.

use super::model::{DemoModel, DemoProfile};
use crate::ipc::{ListFile, ListLine, ListLineKind, ListsView, RouteRow};
use crate::list::{
    CombinedListText, ListConflicts, ListSet, ListTarget, ProfileFootprint, RoutesList, TextProblem,
};
use crate::net::Ipv4Net;

pub struct DemoLists;

impl DemoLists {
    pub fn footprints(model: &DemoModel) -> Vec<ProfileFootprint> {
        model
            .profiles
            .iter()
            .map(|p| ProfileFootprint {
                profile: p.name.clone(),
                endpoints: p.endpoint_addresses.clone(),
                tunnel: Ipv4Net::new(p.address.address, p.address.prefix),
            })
            .collect()
    }

    pub fn view(model: &DemoModel, rev: u64) -> ListsView {
        let set = ListSet::load(model.lists());
        let footprints = Self::footprints(model);
        let conflicts = ListConflicts::new(&set, &footprints);
        let files: Vec<ListFile> = model
            .profiles
            .iter()
            .map(|p| Self::file(p, &set, &conflicts))
            .collect();
        ListsView {
            rev,
            text: CombinedListText::render(model.lists()),
            problems: Self::problems(model, &set),
            files,
        }
    }

    pub fn file(profile: &DemoProfile, set: &ListSet, conflicts: &ListConflicts) -> ListFile {
        let empty = RoutesList::default();
        let list = set.get(&profile.name).unwrap_or(&empty);
        let notes = conflicts.notes(&profile.name);
        let lines = profile
            .list
            .lines()
            .enumerate()
            .map(|(index, raw)| {
                let line = index + 1;
                let entry = list.entries.iter().find(|e| e.line == line);
                let problem = list.problems.iter().find(|p| p.line == line);
                let trimmed = raw.trim();
                let kind = match (entry, problem) {
                    (Some(entry), _) => match &entry.target {
                        ListTarget::Domain(_) => ListLineKind::Domain,
                        ListTarget::Network(net) if net.prefix() == 32 => ListLineKind::Address,
                        ListTarget::Network(_) => ListLineKind::Network,
                    },
                    (None, Some(_)) => ListLineKind::Problem,
                    _ if trimmed.is_empty() => ListLineKind::Blank,
                    _ => ListLineKind::Comment,
                };
                let note = problem.map(|p| p.reason.clone()).or_else(|| {
                    notes
                        .iter()
                        .find(|(l, _)| *l == line)
                        .map(|(_, note)| note.clone())
                });
                ListLine {
                    line,
                    text: raw.to_owned(),
                    kind,
                    note,
                    routes: Self::route_count(profile, line),
                }
            })
            .collect();
        ListFile {
            name: profile.name.clone(),
            lines,
        }
    }

    fn route_count(profile: &DemoProfile, line: usize) -> u64 {
        profile
            .routes
            .iter()
            .filter(|r| {
                r.line == Some(line)
                    && matches!(r.row, RouteRow::Learned { .. } | RouteRow::Static { .. })
            })
            .count() as u64
    }

    /// The file's lines without trailing blank ones, as the combined text holds them.
    fn trimmed_count(text: &str) -> usize {
        let lines: Vec<&str> = text.lines().collect();
        lines.len()
            - lines
                .iter()
                .rev()
                .take_while(|l| l.trim().is_empty())
                .count()
    }

    /// Skipped lines, with both numbers.
    fn problems(model: &DemoModel, set: &ListSet) -> Vec<TextProblem> {
        let mut problems = Vec::new();
        let mut header = 1;
        for profile in &model.profiles {
            if let Some(list) = set.get(&profile.name) {
                for problem in &list.problems {
                    problems.push(TextProblem {
                        text_line: Some(header + problem.line),
                        profile: Some(profile.name.clone()),
                        file_line: Some(problem.line),
                        text: problem.text.clone(),
                        reason: problem.reason.clone(),
                        other: None,
                        blocking: false,
                    });
                }
            }
            header += Self::trimmed_count(&profile.list) + 2;
        }
        problems
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::presentation::transport::demo::{Scenario, scenarios::DemoScenarios};

    #[test]
    fn lines_get_kinds_notes_and_route_counts() {
        let model = DemoScenarios::model(Scenario::Normal);
        let view = DemoLists::view(&model, 1);
        assert!(view.text.starts_with("[profile Contoso]\n# Contoso\n"));
        let contoso = &view.files[0];
        assert_eq!(contoso.lines[0].kind, ListLineKind::Comment);
        assert_eq!(contoso.lines[1].kind, ListLineKind::Domain);
        assert_eq!(contoso.lines[1].routes, 8);
        assert_eq!(contoso.lines[3].kind, ListLineKind::Network);
        assert_eq!(contoso.lines[4].kind, ListLineKind::Address);
        let acme = &view.files[1];
        assert!(acme.lines[1].note.as_deref().unwrap().contains("inside"));
        assert!(acme.lines[3].note.as_deref().unwrap().contains("overrides"));
        assert!(view.problems.is_empty());
    }
}
