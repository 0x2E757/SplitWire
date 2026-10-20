use std::fmt;
use std::time::SystemTime;

/// The ten steps of a Connect, in order. Each one registers its undo.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ConnectStep {
    Profile,
    Proxy,
    Guard,
    Adapter,
    Peer,
    Routes,
    Dns,
    Matcher,
    Nrpt,
    State,
}

impl ConnectStep {
    pub const ALL: [ConnectStep; 10] = [
        ConnectStep::Profile,
        ConnectStep::Proxy,
        ConnectStep::Guard,
        ConnectStep::Adapter,
        ConnectStep::Peer,
        ConnectStep::Routes,
        ConnectStep::Dns,
        ConnectStep::Matcher,
        ConnectStep::Nrpt,
        ConnectStep::State,
    ];

    pub fn id(self) -> &'static str {
        match self {
            ConnectStep::Profile => "profile",
            ConnectStep::Proxy => "proxy",
            ConnectStep::Guard => "guard",
            ConnectStep::Adapter => "adapter",
            ConnectStep::Peer => "peer",
            ConnectStep::Routes => "routes",
            ConnectStep::Dns => "dns",
            ConnectStep::Matcher => "matcher",
            ConnectStep::Nrpt => "nrpt",
            ConnectStep::State => "state",
        }
    }

    /// 1 to 10.
    pub fn number(self) -> usize {
        self as usize + 1
    }
}

impl fmt::Display for ConnectStep {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.id())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StepResult {
    Done,
    Undone,
    Skipped,
    Failed,
    NotRun,
}

impl StepResult {
    pub fn id(self) -> &'static str {
        match self {
            StepResult::Done => "done",
            StepResult::Undone => "undone",
            StepResult::Skipped => "skipped",
            StepResult::Failed => "failed",
            StepResult::NotRun => "not_run",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StepOutcome {
    pub step: ConnectStep,
    pub result: StepResult,
    pub detail: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AttemptKind {
    Connect,
    Recover,
}

/// The last Connect or Recover of a profile: one outcome per step, kept until the next attempt
/// and shown as the step table.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConnectAttempt {
    pub at: SystemTime,
    pub kind: AttemptKind,
    pub steps: Vec<StepOutcome>,
    /// The failed step and its error.
    pub error: Option<(ConnectStep, String)>,
}

impl ConnectAttempt {
    pub fn new(at: SystemTime, kind: AttemptKind) -> Self {
        Self {
            at,
            kind,
            steps: ConnectStep::ALL
                .iter()
                .map(|step| StepOutcome {
                    step: *step,
                    result: StepResult::NotRun,
                    detail: String::new(),
                })
                .collect(),
            error: None,
        }
    }

    pub fn set(&mut self, step: ConnectStep, result: StepResult, detail: impl Into<String>) {
        let outcome = &mut self.steps[step as usize];
        outcome.result = result;
        let detail = detail.into();
        if !detail.is_empty() {
            outcome.detail = detail;
        }
        if result == StepResult::Failed {
            self.error = Some((step, outcome.detail.clone()));
        }
    }

    pub fn result(&self, step: ConnectStep) -> StepResult {
        self.steps[step as usize].result
    }

    pub fn succeeded(&self) -> bool {
        self.error.is_none()
    }

    /// The steps undone after a failure, in the order they were undone.
    pub fn undone(&self) -> Vec<ConnectStep> {
        self.steps
            .iter()
            .rev()
            .filter(|s| s.result == StepResult::Undone)
            .map(|s| s.step)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::UNIX_EPOCH;

    #[test]
    fn numbers_and_names_the_ten_steps() {
        let ids: Vec<&str> = ConnectStep::ALL.iter().map(|s| s.id()).collect();
        assert_eq!(
            ids,
            [
                "profile", "proxy", "guard", "adapter", "peer", "routes", "dns", "matcher", "nrpt",
                "state"
            ]
        );
        assert_eq!(ConnectStep::Profile.number(), 1);
        assert_eq!(ConnectStep::State.number(), 10);
    }

    #[test]
    fn records_a_failure_and_what_was_undone() {
        let mut attempt = ConnectAttempt::new(UNIX_EPOCH, AttemptKind::Connect);
        assert!(attempt.succeeded());
        attempt.set(ConnectStep::Profile, StepResult::Done, "keys decrypted");
        attempt.set(ConnectStep::Proxy, StepResult::Skipped, "already running");
        attempt.set(ConnectStep::Guard, StepResult::Failed, "no address");
        attempt.set(ConnectStep::Profile, StepResult::Undone, "");
        assert_eq!(
            attempt.error,
            Some((ConnectStep::Guard, "no address".into()))
        );
        assert_eq!(attempt.undone(), [ConnectStep::Profile]);
        assert_eq!(attempt.steps[0].detail, "keys decrypted");
        assert_eq!(attempt.result(ConnectStep::Adapter), StepResult::NotRun);
        assert_eq!(StepResult::NotRun.id(), "not_run");
    }
}
