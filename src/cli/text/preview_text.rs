use std::fmt::Write;

use crate::profile::{CheckLevel, ImportPreview, LineOutcome};

/// `import --preview` as text: the profile's values or the changes of a replace, the
/// checks, every `.conf` line with its outcome (key values elided), and what blocks the import.
pub struct PreviewText;

impl PreviewText {
    pub fn preview(preview: &ImportPreview) -> String {
        let what = if preview.replace {
            "replaces the stored profile"
        } else {
            "a new profile"
        };
        let mut text = format!(
            "{} -> {} ({what}, colour pin {})\n",
            preview.file_name,
            preview.name,
            preview.colour.number()
        );
        for (field, value) in &preview.values {
            let _ = writeln!(text, "  {field:<16} {value}");
        }
        if !preview.changes.is_empty() {
            text.push_str("Changes:\n");
            for change in &preview.changes {
                let _ = writeln!(
                    text,
                    "  {:<16} {} -> {}",
                    change.field, change.old, change.new
                );
            }
        }
        if !preview.checks.is_empty() {
            text.push_str("Checks:\n");
            for check in &preview.checks {
                let _ = writeln!(
                    text,
                    "  {:<5} {}: {}",
                    Self::level(check.level),
                    check.title,
                    check.detail
                );
            }
        }
        let noted: Vec<_> = preview
            .lines
            .iter()
            .filter(|l| l.outcome != LineOutcome::Read)
            .collect();
        if !noted.is_empty() {
            text.push_str("Ignored lines and problems:\n");
            for line in noted {
                let outcome = match line.outcome {
                    LineOutcome::Ignored => "ignored",
                    LineOutcome::Problem => "PROBLEM",
                    LineOutcome::Read => "read",
                };
                let at = match line.line {
                    0 => "file".to_owned(),
                    n => format!("line {n}"),
                };
                let _ = writeln!(
                    text,
                    "  {at:<8} {outcome:<8} {}  ({})",
                    line.shown, line.reason
                );
            }
        }
        let problems = preview.problems();
        if problems.is_empty() {
            text.push_str("Ready to import.\n");
        } else {
            text.push_str("Cannot be imported:\n");
            for problem in problems {
                let _ = writeln!(text, "  {problem}");
            }
        }
        text
    }

    fn level(level: CheckLevel) -> &'static str {
        match level {
            CheckLevel::Ok => "ok",
            CheckLevel::Info => "info",
            CheckLevel::Warn => "warn",
            CheckLevel::Bad => "BAD",
        }
    }
}
