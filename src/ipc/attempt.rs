use std::time::SystemTime;

use super::{Reader, WireError, Writer};

/// The ten steps of a Connect, in order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
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
    pub const ALL: [Self; 10] = [
        Self::Profile,
        Self::Proxy,
        Self::Guard,
        Self::Adapter,
        Self::Peer,
        Self::Routes,
        Self::Dns,
        Self::Matcher,
        Self::Nrpt,
        Self::State,
    ];

    /// "step N of 10", from 1.
    pub fn number(self) -> usize {
        Self::ALL.iter().position(|step| *step == self).unwrap_or(0) + 1
    }

    pub fn id(self) -> &'static str {
        match self {
            Self::Profile => "profile",
            Self::Proxy => "proxy",
            Self::Guard => "guard",
            Self::Adapter => "adapter",
            Self::Peer => "peer",
            Self::Routes => "routes",
            Self::Dns => "dns",
            Self::Matcher => "matcher",
            Self::Nrpt => "nrpt",
            Self::State => "state",
        }
    }

    pub fn write(&self, w: &mut Writer) {
        w.choice(self, &Self::ALL);
    }

    pub fn read(r: &mut Reader) -> Result<Self, WireError> {
        r.choice(&Self::ALL)
    }
}

/// What became of one step of an attempt.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StepResult {
    Done,
    Undone,
    Skipped,
    Failed,
    NotRun,
}

impl StepResult {
    const ALL: [Self; 5] = [
        Self::Done,
        Self::Undone,
        Self::Skipped,
        Self::Failed,
        Self::NotRun,
    ];
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AttemptKind {
    Connect,
    /// `TunnelWatch` restoring a connected profile's adapter.
    Recover,
}

impl AttemptKind {
    const ALL: [Self; 2] = [Self::Connect, Self::Recover];
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AttemptResult {
    Succeeded,
    Failed,
}

impl AttemptResult {
    const ALL: [Self; 2] = [Self::Succeeded, Self::Failed];
}

/// One row of the step table.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StepOutcome {
    pub step: ConnectStep,
    pub result: StepResult,
    pub detail: String,
}

impl StepOutcome {
    fn write(&self, w: &mut Writer) {
        self.step.write(w);
        w.choice(&self.result, &StepResult::ALL);
        w.str(&self.detail);
    }

    fn read(r: &mut Reader) -> Result<Self, WireError> {
        Ok(Self {
            step: ConnectStep::read(r)?,
            result: r.choice(&StepResult::ALL)?,
            detail: r.string()?,
        })
    }
}

/// A profile's last Connect or Recover, kept until the next one: the step table of the profile page.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Attempt {
    pub at: SystemTime,
    pub kind: AttemptKind,
    pub result: AttemptResult,
    /// The step that failed.
    pub step: Option<ConnectStep>,
    pub error: Option<String>,
    pub steps: Vec<StepOutcome>,
}

impl Attempt {
    pub fn summary(&self) -> AttemptSummary {
        AttemptSummary {
            at: self.at,
            kind: self.kind,
            result: self.result,
            step: self.step,
        }
    }

    pub fn write(&self, w: &mut Writer) {
        self.summary().write(w);
        w.opt_str(self.error.as_ref());
        w.list(&self.steps, StepOutcome::write);
    }

    pub fn read(r: &mut Reader) -> Result<Self, WireError> {
        let summary = AttemptSummary::read(r)?;
        Ok(Self {
            at: summary.at,
            kind: summary.kind,
            result: summary.result,
            step: summary.step,
            error: r.opt_string()?,
            steps: r.list(StepOutcome::read)?,
        })
    }
}

/// The last attempt as `Status` reports it for every profile.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AttemptSummary {
    pub at: SystemTime,
    pub kind: AttemptKind,
    pub result: AttemptResult,
    pub step: Option<ConnectStep>,
}

impl AttemptSummary {
    pub fn write(&self, w: &mut Writer) {
        w.time(self.at);
        w.choice(&self.kind, &AttemptKind::ALL);
        w.choice(&self.result, &AttemptResult::ALL);
        w.option(self.step.as_ref(), ConnectStep::write);
    }

    pub fn read(r: &mut Reader) -> Result<Self, WireError> {
        Ok(Self {
            at: r.time()?,
            kind: r.choice(&AttemptKind::ALL)?,
            result: r.choice(&AttemptResult::ALL)?,
            step: r.option(ConnectStep::read)?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn steps_are_numbered_from_one_in_order() {
        assert_eq!(ConnectStep::Profile.number(), 1);
        assert_eq!(ConnectStep::Adapter.number(), 4);
        assert_eq!(ConnectStep::State.number(), 10);
        let ids: Vec<&str> = ConnectStep::ALL.iter().map(|s| s.id()).collect();
        assert_eq!(
            ids,
            [
                "profile", "proxy", "guard", "adapter", "peer", "routes", "dns", "matcher", "nrpt",
                "state"
            ]
        );
    }
}
