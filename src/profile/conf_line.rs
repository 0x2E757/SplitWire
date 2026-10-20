/// What became of one `.conf` line.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LineOutcome {
    Read,
    Ignored,
    Problem,
}

impl LineOutcome {
    pub fn as_str(self) -> &'static str {
        match self {
            LineOutcome::Read => "read",
            LineOutcome::Ignored => "ignored",
            LineOutcome::Problem => "problem",
        }
    }
}

/// One `.conf` line as the import review shows it. `shown` elides every value that is read, every
/// key and script, and anything unknown; only harmless ignored values (`AllowedIPs = 0.0.0.0/0`)
/// appear. `line` 0 is a problem of the whole file, such as a missing `PrivateKey`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConfLine {
    pub line: usize,
    pub key: String,
    pub shown: String,
    pub outcome: LineOutcome,
    pub reason: String,
}

impl ConfLine {
    pub const ELIDED: &'static str = "…";

    pub fn read(line: usize, key: &str) -> Self {
        Self::new(
            line,
            key,
            format!("{key} = {}", Self::ELIDED),
            LineOutcome::Read,
            "",
        )
    }

    pub fn ignored(line: usize, key: &str, shown: String, reason: &str) -> Self {
        Self::new(line, key, shown, LineOutcome::Ignored, reason)
    }

    /// A problem never shows the value: it may be a key.
    pub fn problem(line: usize, key: &str, reason: impl Into<String>) -> Self {
        let shown = if key.is_empty() {
            Self::ELIDED.to_owned()
        } else {
            format!("{key} = {}", Self::ELIDED)
        };
        Self::new(line, key, shown, LineOutcome::Problem, reason)
    }

    /// A problem with no line, such as a missing key.
    pub fn missing(key: &str, section: &str) -> Self {
        Self::new(
            0,
            key,
            String::new(),
            LineOutcome::Problem,
            format!("{key} is missing from {section}"),
        )
    }

    fn new(
        line: usize,
        key: &str,
        shown: String,
        outcome: LineOutcome,
        reason: impl Into<String>,
    ) -> Self {
        Self {
            line,
            key: key.to_owned(),
            shown,
            outcome,
            reason: reason.into(),
        }
    }
}
