use std::fmt;

use super::ListSet;

/// A problem of the combined text, with both numbers: the line in the text and the line in the
/// profile's file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TextProblem {
    pub text_line: Option<usize>,
    pub profile: Option<String>,
    pub file_line: Option<usize>,
    pub text: String,
    pub reason: String,
    /// The line of another profile it collides with.
    pub other: Option<(String, usize)>,
    /// A blocking problem refuses the whole save; any other only skips its line.
    pub blocking: bool,
}

impl fmt::Display for TextProblem {
    /// `text line 14 (AcmeCorp line 4): …`.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match (self.text_line, &self.profile, self.file_line) {
            (Some(text), Some(profile), Some(file)) => {
                write!(
                    f,
                    "text line {text} ({profile} line {file}): {}",
                    self.reason
                )
            }
            (Some(text), _, _) => write!(f, "text line {text}: {}", self.reason),
            _ => f.write_str(&self.reason),
        }
    }
}

/// The combined text split back into files.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SplitText {
    /// `(profile, file text)` in the order of the sections.
    pub files: Vec<(String, String)>,
    pub problems: Vec<TextProblem>,
}

impl SplitText {
    /// Refused whole: nothing may be written.
    pub fn refused(&self) -> bool {
        self.problems.iter().any(|p| p.blocking)
    }

    /// The files whose text differs from `current` (`(profile, text)`), the only ones to write.
    /// Trailing blank lines and line endings do not count as a change.
    pub fn changed<'a>(&'a self, current: &[(&str, &str)]) -> Vec<&'a (String, String)> {
        self.files
            .iter()
            .filter(|(profile, text)| {
                let old = current
                    .iter()
                    .find(|(p, _)| p == profile)
                    .map_or("", |(_, t)| t);
                CombinedListText::trimmed_lines(old) != CombinedListText::trimmed_lines(text)
            })
            .collect()
    }
}

/// All lists as one text: per profile in the user's order a `[profile <name>]` header, then the
/// file's lines verbatim, so section line k is file line k; one blank line between sections.
pub struct CombinedListText;

struct Section {
    profile: String,
    header_line: usize,
    lines: Vec<String>,
    known: bool,
}

impl CombinedListText {
    const HEADER: &'static str = "[profile ";

    /// `files` are `(profile, routes.txt text)` in the user's order.
    pub fn render<'a>(files: impl IntoIterator<Item = (&'a str, &'a str)>) -> String {
        let sections: Vec<String> = files
            .into_iter()
            .map(|(profile, text)| {
                let mut section = format!("{}{profile}]\n", Self::HEADER);
                for line in Self::trimmed_lines(text) {
                    section.push_str(line);
                    section.push('\n');
                }
                section
            })
            .collect();
        sections.join("\n")
    }

    /// Splits `text` into the files of `profiles` (the existing profiles, in order).
    pub fn split(text: &str, profiles: &[&str]) -> SplitText {
        let lines: Vec<&str> = text.lines().collect();
        let mut sections: Vec<Section> = Vec::new();
        let mut problems = Vec::new();
        for (index, raw) in lines.iter().enumerate() {
            let text_line = index + 1;
            if let Some(name) = Self::header(raw) {
                let known = profiles.iter().find(|p| p.eq_ignore_ascii_case(name));
                let twice = sections
                    .iter()
                    .any(|s| s.known && s.profile.eq_ignore_ascii_case(name));
                let reason = match (known, twice) {
                    (None, _) => Some(format!("unknown section [profile {name}]")),
                    (Some(_), true) => Some(format!("section [profile {name}] appears twice")),
                    _ => None,
                };
                if let Some(reason) = reason {
                    problems.push(Self::blocking(Some(text_line), raw, reason));
                }
                sections.push(Section {
                    profile: known.map_or(name, |p| *p).to_owned(),
                    header_line: text_line,
                    lines: Vec::new(),
                    known: known.is_some() && !twice,
                });
                continue;
            }
            let separator = raw.trim().is_empty()
                && lines
                    .get(index + 1)
                    .is_some_and(|next| Self::header(next).is_some());
            match sections.last_mut() {
                _ if separator => {}
                Some(section) => section.lines.push((*raw).to_owned()),
                None if raw.trim().is_empty() => {}
                None => problems.push(Self::blocking(
                    Some(text_line),
                    raw,
                    "a line before the first [profile …] section".into(),
                )),
            }
        }
        for profile in profiles {
            if !sections.iter().any(|s| s.known && s.profile == *profile) {
                problems.push(Self::blocking(
                    None,
                    "",
                    format!(
                        "section [profile {profile}] is missing (a list cannot be deleted here; \
                         remove the profile instead)"
                    ),
                ));
            }
        }
        let known: Vec<&Section> = sections.iter().filter(|s| s.known).collect();
        let files: Vec<(String, String)> = known
            .iter()
            .map(|s| (s.profile.clone(), Self::file_text(&s.lines)))
            .collect();
        problems.extend(Self::list_problems(&known, &files));
        SplitText { files, problems }
    }

    /// The skipped lines of each section, and the exact duplicates across sections (blocking).
    fn list_problems(sections: &[&Section], files: &[(String, String)]) -> Vec<TextProblem> {
        let set = ListSet::load(files.iter().map(|(p, t)| (p.as_str(), t.as_str())));
        let mut problems = Vec::new();
        for (section, list) in sections.iter().zip(set.lists()) {
            for problem in &list.list.problems {
                let duplicate = set
                    .duplicates()
                    .iter()
                    .find(|d| d.profile == section.profile && d.line == problem.line);
                let reason = match duplicate {
                    Some(_) => problem.reason.clone(),
                    None => format!("{}; the line is skipped", problem.reason),
                };
                problems.push(TextProblem {
                    text_line: Some(section.header_line + problem.line),
                    profile: Some(section.profile.clone()),
                    file_line: Some(problem.line),
                    text: problem.text.clone(),
                    reason,
                    other: duplicate.map(|d| (d.other_profile.clone(), d.other_line)),
                    blocking: duplicate.is_some(),
                });
            }
        }
        problems
    }

    fn header(line: &str) -> Option<&str> {
        let name = line
            .trim()
            .strip_prefix(Self::HEADER)?
            .strip_suffix(']')?
            .trim();
        (!name.is_empty()).then_some(name)
    }

    fn file_text(lines: &[String]) -> String {
        let lines: Vec<&str> = Self::trimmed_lines_of(lines.iter().map(String::as_str));
        lines.iter().map(|line| format!("{line}\n")).collect()
    }

    /// The lines without trailing blank lines.
    fn trimmed_lines(text: &str) -> Vec<&str> {
        Self::trimmed_lines_of(text.lines())
    }

    fn trimmed_lines_of<'a>(lines: impl Iterator<Item = &'a str>) -> Vec<&'a str> {
        let mut lines: Vec<&str> = lines.collect();
        while lines.last().is_some_and(|line| line.trim().is_empty()) {
            lines.pop();
        }
        lines
    }

    fn blocking(text_line: Option<usize>, raw: &str, reason: String) -> TextProblem {
        TextProblem {
            text_line,
            profile: None,
            file_line: None,
            text: raw.trim().to_owned(),
            reason,
            other: None,
            blocking: true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ACME: &str = "# Acme\ncorp.com\n\n10.0.0.0/8\n\n\n";
    const CONTOSO: &str = "contoso.example\n";

    fn combined() -> String {
        CombinedListText::render([("AcmeCorp", ACME), ("Contoso", CONTOSO), ("Empty", "")])
    }

    #[test]
    fn renders_sections_in_order_with_one_blank_line_between() {
        assert_eq!(
            combined(),
            "[profile AcmeCorp]\n# Acme\ncorp.com\n\n10.0.0.0/8\n\n\
             [profile Contoso]\ncontoso.example\n\n[profile Empty]\n"
        );
    }

    #[test]
    fn splits_back_with_the_same_lines() {
        let split = CombinedListText::split(&combined(), &["AcmeCorp", "Contoso", "Empty"]);
        assert!(split.problems.is_empty(), "{:?}", split.problems);
        assert_eq!(
            split.files,
            [
                ("AcmeCorp".into(), "# Acme\ncorp.com\n\n10.0.0.0/8\n".into()),
                ("Contoso".into(), CONTOSO.into()),
                ("Empty".into(), String::new()),
            ]
        );
    }

    #[test]
    fn maps_problems_to_both_line_numbers() {
        let text = "[profile AcmeCorp]\ncorp.com\nintranet\n\n[profile Contoso]\n# b\nCorp.com\n";
        let split = CombinedListText::split(text, &["AcmeCorp", "Contoso"]);
        assert!(split.refused());
        let shown: Vec<String> = split.problems.iter().map(ToString::to_string).collect();
        assert_eq!(
            shown,
            [
                "text line 3 (AcmeCorp line 2): a domain needs at least 2 labels; the line is skipped",
                "text line 7 (Contoso line 2): also in AcmeCorp line 1: a name belongs to one profile",
            ]
        );
        assert!(!split.problems[0].blocking);
        assert_eq!(split.problems[1].other, Some(("AcmeCorp".into(), 1)));
    }

    #[test]
    fn only_changed_files_are_written() {
        let text = combined().replace(
            "contoso.example",
            "contoso.example
shop-contoso.example",
        );
        let split = CombinedListText::split(&text, &["AcmeCorp", "Contoso", "Empty"]);
        let current = [
            (
                "AcmeCorp",
                "# Acme
corp.com

10.0.0.0/8

",
            ),
            ("Contoso", CONTOSO),
        ];
        let changed: Vec<&str> = split
            .changed(&current)
            .iter()
            .map(|(p, _)| p.as_str())
            .collect();
        assert_eq!(changed, ["Contoso"]);
    }

    #[test]
    fn a_skipped_line_alone_does_not_refuse_the_save() {
        let split = CombinedListText::split("[profile A]\nintranet\n", &["A"]);
        assert!(!split.refused());
        assert_eq!(split.files, [("A".into(), "intranet\n".into())]);
    }

    #[test]
    fn refuses_unknown_doubled_missing_sections_and_stray_lines() {
        let text = "\nstray.com\n[profile A]\na.com\n[profile Ghost]\nx.com\n[profile a]\n";
        let split = CombinedListText::split(text, &["A", "B"]);
        assert!(split.refused());
        let shown: Vec<String> = split.problems.iter().map(ToString::to_string).collect();
        assert_eq!(
            shown,
            [
                "text line 2: a line before the first [profile …] section",
                "text line 5: unknown section [profile Ghost]",
                "text line 7: section [profile a] appears twice",
                "section [profile B] is missing (a list cannot be deleted here; remove the profile instead)",
            ]
        );
    }
}
