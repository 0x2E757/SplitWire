use std::collections::HashMap;
use std::fmt;

/// A problem of a settings file, with the line it is on (0 when it is about the whole file).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SettingProblem {
    pub file: String,
    pub line: usize,
    pub key: String,
    pub value: String,
    pub reason: String,
    /// A value that fixes it ("Use 1420"), filled into the field, never written by itself.
    pub suggest: Option<String>,
}

impl fmt::Display for SettingProblem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} line {}: {}", self.file, self.line, self.reason)
    }
}

/// One `key=value` line.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KeyValueEntry {
    pub line: usize,
    pub key: String,
    pub value: String,
}

/// The strict `key=value` format (TaskbarMetrics'): `#` starts a comment, blank lines are
/// allowed, keys and values are trimmed. A line without `=` and a repeated key are problems.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KeyValueFile {
    pub file: String,
    pub entries: Vec<KeyValueEntry>,
    pub problems: Vec<SettingProblem>,
}

impl KeyValueFile {
    /// `file` names the file in problems, such as `splitwire.conf`.
    pub fn parse(file: &str, text: &str) -> Self {
        let mut parsed = Self {
            file: file.to_owned(),
            entries: Vec::new(),
            problems: Vec::new(),
        };
        let mut first: HashMap<String, usize> = HashMap::new();
        for (index, raw) in text.lines().enumerate() {
            let line = index + 1;
            let content = raw.split('#').next().unwrap_or_default().trim();
            if content.is_empty() {
                continue;
            }
            let Some((key, value)) = content
                .split_once('=')
                .filter(|(key, _)| !key.trim().is_empty())
            else {
                parsed.problem(line, "", content, "expected key=value");
                continue;
            };
            let (key, value) = (key.trim(), value.trim());
            if let Some(earlier) = first.get(key) {
                parsed.problem(
                    line,
                    key,
                    value,
                    &format!("{key} is already on line {earlier}"),
                );
                continue;
            }
            first.insert(key.to_owned(), line);
            parsed.entries.push(KeyValueEntry {
                line,
                key: key.to_owned(),
                value: value.to_owned(),
            });
        }
        parsed
    }

    pub fn get(&self, key: &str) -> Option<&KeyValueEntry> {
        self.entries.iter().find(|entry| entry.key == key)
    }

    /// Records the error of `result` as a problem of `entry`, with the value that fixes it.
    pub fn check(
        &mut self,
        entry: &KeyValueEntry,
        result: Result<(), String>,
        suggest: Option<&str>,
    ) {
        if let Err(reason) = result {
            self.problem(entry.line, &entry.key, &entry.value, &reason);
            if let Some(problem) = self.problems.last_mut() {
                problem.suggest = suggest.map(str::to_owned);
            }
        }
    }

    /// `value` when the file has no problem.
    pub fn into_result<T>(self, value: T) -> Result<T, Vec<SettingProblem>> {
        if self.problems.is_empty() {
            Ok(value)
        } else {
            Err(self.problems)
        }
    }

    pub fn problem(&mut self, line: usize, key: &str, value: &str, reason: &str) {
        self.problems.push(SettingProblem {
            file: self.file.clone(),
            line,
            key: key.to_owned(),
            value: value.to_owned(),
            reason: reason.to_owned(),
            suggest: None,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_entries_with_their_lines() {
        let file = KeyValueFile::parse(
            "splitwire.conf",
            "# DNS\n\nmax_ttl = 300 # five minutes\nbypass=a.example.com, b.example.com\nfallback=\n",
        );
        assert!(file.problems.is_empty());
        let entries: Vec<(usize, &str, &str)> = file
            .entries
            .iter()
            .map(|e| (e.line, e.key.as_str(), e.value.as_str()))
            .collect();
        assert_eq!(
            entries,
            [
                (3, "max_ttl", "300"),
                (4, "bypass", "a.example.com, b.example.com"),
                (5, "fallback", ""),
            ]
        );
        assert_eq!(file.get("max_ttl").unwrap().line, 3);
    }

    #[test]
    fn reports_malformed_lines_and_duplicates() {
        let file = KeyValueFile::parse("x.conf", "max_ttl=300\njunk\n=5\nmax_ttl=60\n");
        let problems: Vec<String> = file.problems.iter().map(ToString::to_string).collect();
        assert_eq!(
            problems,
            [
                "x.conf line 2: expected key=value",
                "x.conf line 3: expected key=value",
                "x.conf line 4: max_ttl is already on line 1",
            ]
        );
        assert_eq!(file.problems[2].value, "60");
        assert_eq!(file.entries.len(), 1);
    }
}
