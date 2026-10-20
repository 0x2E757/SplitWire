//! The last Connect's 10 steps: the page message, the table and the text that
//! Copy details puts on the clipboard.

use super::LocalTime;
use crate::ipc::{Attempt, AttemptKind, AttemptResult, ConnectStep, StepResult};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StepRow {
    pub number: usize,
    pub id: &'static str,
    pub what: &'static str,
    pub result: StepResult,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StepTableView {
    /// `Connect failed at step 3 of 10 · guard`.
    pub title: String,
    /// The error, then what it means for the other profiles.
    pub message: String,
    /// `11:27:41 · failed at step 3, rolled back`.
    pub caption: String,
    pub rows: Vec<StepRow>,
    /// `Undone in reverse: profile. Contoso and AcmeCorp kept …`.
    pub footer: String,
    /// The table as text, for Copy details.
    pub details: String,
}

impl StepTableView {
    /// What each step does.
    pub fn what(step: ConnectStep) -> &'static str {
        match step {
            ConnectStep::Profile => "decrypt the keys, check the profile settings",
            ConnectStep::Proxy => "bind 127.0.0.53:53 (first connected profile only)",
            ConnectStep::Guard => "never-routes: base, other profiles, resolve the endpoint",
            ConnectStep::Adapter => "load the DLL, create the adapter, address, MTU",
            ConnectStep::Peer => "configure the peer, bring the tunnel up",
            ConnectStep::Routes => "upstream DNS /32s and static entries",
            ConnectStep::Dns => "bound upstream exchange, register the resolver",
            ConnectStep::Matcher => "rebuild; re-home learned routes",
            ConnectStep::Nrpt => "sync the rules to the union, flush the cache",
            ConnectStep::State => "add to connected=",
        }
    }

    pub fn result_label(result: StepResult) -> &'static str {
        match result {
            StepResult::Done => "Done",
            StepResult::Undone => "Undone",
            StepResult::Skipped => "Skipped",
            StepResult::Failed => "Failed",
            StepResult::NotRun => "Not run",
        }
    }

    /// `others` are the profiles that stay connected.
    pub fn of(attempt: &Attempt, others: &[&str], time: &LocalTime) -> Self {
        let failed = attempt.result == AttemptResult::Failed;
        let kind = match attempt.kind {
            AttemptKind::Connect => "Connect",
            AttemptKind::Recover => "Recover",
        };
        let step = attempt.step.unwrap_or(ConnectStep::Profile);
        let rows: Vec<StepRow> = attempt
            .steps
            .iter()
            .map(|s| StepRow {
                number: s.step.number(),
                id: s.step.id(),
                what: Self::what(s.step),
                result: s.result,
                detail: s.detail.clone(),
            })
            .collect();
        let undone: Vec<&str> = rows
            .iter()
            .rev()
            .filter(|r| r.result == StepResult::Undone)
            .map(|r| r.id)
            .collect();
        let kept = match others {
            [] => String::new(),
            [one] => format!(" {one} kept its tunnel, routes and NRPT rules."),
            [rest @ .., last] => format!(
                " {} and {last} kept their tunnels, routes and NRPT rules.",
                rest.join(", ")
            ),
        };
        let footer = if undone.is_empty() {
            format!("Nothing to undo.{kept}")
        } else {
            format!("Undone in reverse: {}.{kept}", undone.join(", "))
        };
        let caption = if failed {
            format!(
                "{} · failed at step {}, rolled back",
                time.clock_seconds(attempt.at),
                step.number()
            )
        } else {
            format!("{} · succeeded", time.clock_seconds(attempt.at))
        };
        let error = attempt.error.clone().unwrap_or_default();
        let details = std::iter::once(format!("{kind} {caption}"))
            .chain(std::iter::once(error.clone()).filter(|e| !e.is_empty()))
            .chain(rows.iter().map(|r| {
                format!(
                    "{:>2} {:<8} {:<8} {}",
                    r.number,
                    r.id,
                    Self::result_label(r.result),
                    r.detail
                )
                .trim_end()
                .to_owned()
            }))
            .chain(std::iter::once(footer.clone()))
            .collect::<Vec<_>>()
            .join("\n");
        Self {
            title: format!(
                "{kind} failed at step {} of 10 · {}",
                step.number(),
                step.id()
            ),
            message: format!(
                "{}. The steps already done were undone; the other profiles are not affected.",
                error.trim_end_matches('.')
            ),
            caption,
            rows,
            footer,
            details,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::presentation::samples::Samples;

    #[test]
    fn the_failed_connect_names_its_step_and_what_was_undone() {
        let detail = Samples::detail("connectfail", "HomeLab");
        let attempt = detail.last_attempt.unwrap();
        let view = StepTableView::of(&attempt, &["Contoso", "AcmeCorp"], &Samples::time());
        assert_eq!(view.title, "Connect failed at step 3 of 10 · guard");
        assert_eq!(view.caption, "11:27:41 · failed at step 3, rolled back");
        assert_eq!(
            view.footer,
            "Undone in reverse: profile. Contoso and AcmeCorp kept their tunnels, routes and NRPT rules."
        );
        assert_eq!(view.rows[2].result, StepResult::Failed);
        assert_eq!(view.rows[9].result, StepResult::NotRun);
        assert!(view.message.starts_with("home.example.net did not resolve"));
        assert!(
            view.details
                .contains(" 3 guard    Failed   home.example.net: NXDOMAIN")
        );
    }
}
