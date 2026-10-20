//! List requests: every edit and save checked against every list and the revision.

use super::DemoEngine;
use crate::ipc::{ErrorCode, LineRef, RefusedProblem, Response};
use crate::list::{CombinedListText, ListSet, TextProblem};
use crate::presentation::transport::demo::lists::DemoLists;

impl DemoEngine {
    pub(super) fn lists(&self) -> Response {
        Response::Lists(Box::new(DemoLists::view(&self.model, self.revs.lists)))
    }

    pub(super) fn stale(&self, base_rev: u64) -> Option<Response> {
        (base_rev != self.revs.lists)
            .then(|| Self::error(ErrorCode::StaleRev, "the lists changed on disk; reloaded"))
    }

    pub(super) fn list_refused(problem: TextProblem) -> Response {
        let message = problem.to_string();
        Self::error(
            ErrorCode::Refused(vec![RefusedProblem::List(problem)]),
            &message,
        )
    }

    pub(super) fn edit_list(
        &mut self,
        base_rev: u64,
        name: &str,
        add: Option<String>,
        remove: Option<LineRef>,
    ) -> Response {
        if let Some(stale) = self.stale(base_rev) {
            return stale;
        }
        let set = ListSet::load(self.model.lists());
        let Some(profile) = self.model.profile_mut(name) else {
            return Self::unknown(name);
        };
        if let Some(text) = add {
            match set.check_add(&profile.name, &text) {
                Ok(target) => {
                    if !profile.list.is_empty() && !profile.list.ends_with('\n') {
                        profile.list.push('\n');
                    }
                    profile.list.push_str(&format!("{target}\n"));
                }
                Err(reason) => {
                    return Self::list_refused(TextProblem {
                        text_line: None,
                        profile: Some(profile.name.clone()),
                        file_line: None,
                        text: text.trim().to_owned(),
                        reason,
                        other: None,
                        blocking: true,
                    });
                }
            }
        }
        if let Some(line) = remove {
            let mut lines: Vec<&str> = profile.list.lines().collect();
            if lines.get(line.line - 1).is_none_or(|t| *t != line.text) {
                return Self::error(ErrorCode::StaleRev, "the lists changed on disk; reloaded");
            }
            lines.remove(line.line - 1);
            let mut text = lines.join("\n");
            if !text.is_empty() {
                text.push('\n');
            }
            profile.list = text;
            profile.routes.retain(|r| r.line != Some(line.line));
            for route in &mut profile.routes {
                if let Some(l) = route.line.as_mut().filter(|l| **l > line.line) {
                    *l -= 1;
                }
            }
        }
        self.revs.lists += 1;
        self.lists()
    }

    pub(super) fn save_list(&mut self, base_rev: u64, name: &str, text: String) -> Response {
        if let Some(stale) = self.stale(base_rev) {
            return stale;
        }
        let mut files: Vec<(String, String)> = self
            .model
            .lists()
            .into_iter()
            .map(|(n, t)| (n.to_owned(), t.to_owned()))
            .collect();
        let Some(file) = files.iter_mut().find(|(n, _)| n.eq_ignore_ascii_case(name)) else {
            return Self::unknown(name);
        };
        file.1 = text.clone();
        let set = ListSet::load(files.iter().map(|(n, t)| (n.as_str(), t.as_str())));
        if let Some(duplicate) = set.duplicates().iter().find(|d| {
            d.profile.eq_ignore_ascii_case(name) || d.other_profile.eq_ignore_ascii_case(name)
        }) {
            return Self::list_refused(TextProblem {
                text_line: None,
                profile: Some(duplicate.profile.clone()),
                file_line: Some(duplicate.line),
                text: String::new(),
                reason: format!(
                    "also in {} line {}: a name belongs to one profile",
                    duplicate.other_profile, duplicate.other_line
                ),
                other: Some((duplicate.other_profile.clone(), duplicate.other_line)),
                blocking: true,
            });
        }
        if let Some(profile) = self.model.profile_mut(name) {
            profile.list = text;
        }
        self.revs.lists += 1;
        self.lists()
    }

    pub(super) fn save_all_lists(&mut self, base_rev: u64, text: &str) -> Response {
        if let Some(stale) = self.stale(base_rev) {
            return stale;
        }
        let names: Vec<String> = self.model.profiles.iter().map(|p| p.name.clone()).collect();
        let names: Vec<&str> = names.iter().map(String::as_str).collect();
        let split = CombinedListText::split(text, &names);
        if split.refused() {
            let problems: Vec<RefusedProblem> = split
                .problems
                .iter()
                .filter(|p| p.blocking)
                .cloned()
                .map(RefusedProblem::List)
                .collect();
            return Self::error(
                ErrorCode::Refused(problems),
                "the save was refused: nothing was written",
            );
        }
        for (name, file) in &split.files {
            if let Some(profile) = self.model.profile_mut(name) {
                profile.list = file.clone();
            }
        }
        self.revs.lists += 1;
        self.lists()
    }
}

#[cfg(test)]
mod tests {
    use super::super::DemoEngine;
    use crate::ipc::{ErrorCode, ErrorReply, Request, Response};
    use crate::presentation::transport::demo::Scenario;

    #[test]
    pub(super) fn list_edits_check_every_list_and_the_revision() {
        let mut engine = DemoEngine::new(Scenario::Normal);
        let dup = engine.handle(Request::EditList {
            base_rev: 1,
            name: "AcmeCorp".into(),
            add: Some("contoso.example".into()),
            remove: None,
        });
        assert!(matches!(dup, Response::Error(_)));
        let Response::Lists(view) = engine.handle(Request::EditList {
            base_rev: 1,
            name: "AcmeCorp".into(),
            add: Some("Wiki.Acme.Example".into()),
            remove: None,
        }) else {
            panic!("lists expected");
        };
        assert_eq!(view.rev, 2);
        assert_eq!(
            view.files[1].lines.last().unwrap().text,
            "wiki.acme.example"
        );
        let stale = engine.handle(Request::GetAllLists);
        assert!(matches!(stale, Response::Lists(_)));
        assert!(matches!(
            engine.handle(Request::SaveAllLists {
                base_rev: 1,
                text: String::new()
            }),
            Response::Error(ErrorReply {
                code: ErrorCode::StaleRev,
                ..
            })
        ));
    }
}
