//! The import review: per file a summary card (one sentence, the Name validated as typed,
//! the colour as six swatches), a reconnect warning, and expanders for the values or changes, the
//! checks against the other profiles and the ignored lines. Keys never appear.

use super::InfoBar;
use crate::ipc::{ProfileState, StatusReport};
use crate::presentation::{Format, ImportBatch};
use crate::profile::{CheckLevel, LineOutcome, Pin};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReviewFileView {
    pub index: usize,
    pub file_name: String,
    pub name: String,
    pub replace: Option<String>,
    pub name_problem: Option<String>,
    pub name_hint: String,
    pub colour: Pin,
    /// Each pin and the profile that already uses it.
    pub colours: Vec<(Pin, Option<String>)>,
    pub sentence: String,
    pub reconnect: Option<InfoBar>,
    /// `(field, value)` of a new profile, or `(field, old → new)` of a replace.
    pub values: Vec<(String, String, Option<String>)>,
    pub checks: Vec<(CheckLevel, String, String)>,
    pub checks_summary: String,
    pub checks_open: bool,
    /// `(line as shown, why)`.
    pub ignored: Vec<(String, String)>,
    /// The preview has not arrived yet.
    pub waiting: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReviewView {
    pub title: String,
    pub subtitle: String,
    pub import_label: String,
    pub can_import: bool,
    pub importing: bool,
    pub note: Option<String>,
    pub failure: Option<String>,
    pub files: Vec<ReviewFileView>,
}

impl ReviewView {
    pub fn of(batch: &ImportBatch, status: &StatusReport) -> Self {
        let files: Vec<ReviewFileView> = (0..batch.files.len())
            .map(|i| Self::file(batch, i, status))
            .collect();
        let single_replace = files.len() == 1 && files[0].replace.is_some();
        Self {
            title: if single_replace {
                format!("Replace {}", files[0].name)
            } else {
                "Import profiles".into()
            },
            subtitle: batch
                .files
                .iter()
                .map(|f| f.file_name.as_str())
                .collect::<Vec<_>>()
                .join(" · "),
            import_label: match (files.len(), single_replace) {
                (_, true) => "Replace and reconnect".into(),
                (1, _) => "Import".into(),
                (n, _) => format!("Import {n} files"),
            },
            can_import: batch.ready(),
            importing: batch.importing.is_some(),
            note: (files.len() > 1).then(|| {
                "Each file is checked on its own and against your profiles; a replaced profile keeps its list and settings. A failure stops the batch; the files already imported stay.".into()
            }),
            failure: batch.failure.clone(),
            files,
        }
    }

    fn file(batch: &ImportBatch, index: usize, status: &StatusReport) -> ReviewFileView {
        let file = &batch.files[index];
        let preview = file.current();
        let replace = preview.filter(|p| p.replace).map(|p| p.name.clone());
        let name_problem = batch.name_problem(index);
        let target = replace
            .as_ref()
            .and_then(|n| status.profiles.iter().find(|p| p.name == *n));
        let connected = target.is_some_and(|p| {
            matches!(
                p.state,
                ProfileState::Connected(_) | ProfileState::Connecting(_)
            )
        });
        let values: Vec<(String, String, Option<String>)> = match preview {
            Some(p) if p.replace => p
                .changes
                .iter()
                .map(|c| (c.field.clone(), c.old.clone(), Some(c.new.clone())))
                .collect(),
            Some(p) => p
                .values
                .iter()
                .map(|(k, v)| (k.clone(), v.clone(), None))
                .collect(),
            None => Vec::new(),
        };
        let mut checks: Vec<(CheckLevel, String, String)> = preview
            .map(|p| {
                p.checks
                    .iter()
                    .map(|c| (c.level, c.title.clone(), c.detail.clone()))
                    .collect()
            })
            .unwrap_or_default();
        if let Some(problem) = &name_problem {
            checks.insert(
                0,
                (
                    CheckLevel::Bad,
                    format!("Name “{}”", file.name),
                    problem.clone(),
                ),
            );
        }
        for line in preview
            .map(|p| {
                p.lines
                    .iter()
                    .filter(|l| l.outcome == LineOutcome::Problem)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default()
        {
            checks.push((CheckLevel::Bad, line.shown.clone(), line.reason.clone()));
        }
        let worst = checks.iter().map(|c| c.0).max_by_key(|l| match l {
            CheckLevel::Ok => 0,
            CheckLevel::Info => 1,
            CheckLevel::Warn => 2,
            CheckLevel::Bad => 3,
        });
        let checks_summary = match worst {
            Some(CheckLevel::Bad) => "blocked: fix the problems".to_owned(),
            Some(CheckLevel::Warn) if connected => "will reconnect".to_owned(),
            Some(CheckLevel::Warn) => "see the warnings".to_owned(),
            _ => "nothing blocks".to_owned(),
        };
        let ignored = preview
            .map(|p| {
                p.lines
                    .iter()
                    .filter(|l| l.outcome == LineOutcome::Ignored)
                    .map(|l| (l.shown.clone(), l.reason.clone()))
                    .collect()
            })
            .unwrap_or_default();
        let sentence = match (&replace, preview) {
            (_, None) => "Reading the file…".to_owned(),
            (Some(_), Some(p)) => format!(
                "{} · keys are compared, never shown{}",
                Format::plural(p.changes.len(), "change", "changes"),
                if connected { " · will reconnect" } else { "" }
            ),
            (None, Some(p)) => format!(
                "A new profile · {} read · {} · starts disconnected, with an empty list",
                Format::plural(p.values.len(), "value", "values"),
                if name_problem.is_some() || p.blocks() {
                    "fix the problems"
                } else {
                    "nothing blocks"
                }
            ),
        };
        let colour = file.colour();
        ReviewFileView {
            index,
            file_name: file.file_name.clone(),
            name: file.name.clone(),
            name_hint: if replace.is_some() {
                "Same name as the stored profile: it is replaced".into()
            } else {
                format!("From the file name · adapter SplitWire-{}", file.name)
            },
            name_problem,
            colour,
            colours: (1..=Pin::COUNT)
                .filter_map(Pin::new)
                .map(|pin| {
                    let owner = status
                        .profiles
                        .iter()
                        .find(|p| p.colour == pin && Some(&p.name) != replace.as_ref())
                        .map(|p| p.name.clone());
                    (pin, owner)
                })
                .collect(),
            sentence,
            reconnect: target.filter(|_| connected).map(|t| {
                InfoBar::new(
                    &format!(
                        "{} is connected ({})",
                        t.name,
                        t.uptime_secs.map(Format::span).unwrap_or_default()
                    ),
                    "Replace disconnects it, stores the new keys and connects again. If anything fails, nothing changes.",
                )
            }),
            values,
            checks_open: matches!(worst, Some(CheckLevel::Warn | CheckLevel::Bad)),
            checks,
            checks_summary,
            ignored,
            waiting: preview.is_none(),
            replace,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ipc::{ConfText, Request, Response};
    use crate::presentation::PickedFile;
    use crate::presentation::samples::Samples;
    use crate::presentation::transport::EngineTransport;
    use crate::presentation::transport::demo::{DemoTransport, Scenario};
    use crate::profile::fixtures;
    use std::sync::Arc;

    #[test]
    fn a_previewed_file_reads_as_s6d() {
        let conf = fixtures::conf(
            fixtures::PRIVATE,
            "10.77.0.5/32",
            "10.77.0.53",
            "vpn.partner.example:51820",
        );
        let mut batch = ImportBatch::new(
            vec![PickedFile {
                file_name: "Partner.conf".into(),
                text: Ok(Arc::new(ConfText::new(conf.clone()))),
            }],
            None,
        );
        let mut demo = DemoTransport::new(Scenario::Normal);
        let Ok(Response::ImportPreview(preview)) = demo.exchange(&Request::PreviewImport {
            name: "Partner".into(),
            file_name: "Partner.conf".into(),
            conf_text: ConfText::new(conf),
        }) else {
            panic!("a preview");
        };
        batch.files[0].preview = Some(*preview);
        batch.files[0].previewed = Some("Partner".into());
        let status = Samples::status("normal");
        let view = ReviewView::of(&batch, &status);
        assert_eq!(view.title, "Import profiles");
        assert_eq!(view.import_label, "Import");
        let file = &view.files[0];
        assert!(file.sentence.starts_with("A new profile · "));
        assert_eq!(
            file.values[0],
            ("Adapter".into(), "SplitWire-Partner".into(), None)
        );
        assert_eq!(file.colours[2].1.as_deref(), Some("Contoso"));
        assert!(!file.waiting);
        assert!(view.can_import, "{:?}", file.checks);
    }
}
