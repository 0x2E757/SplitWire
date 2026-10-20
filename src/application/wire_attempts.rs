use crate::ipc;
use crate::profile::ProfileState;
use crate::tunnel::{AttemptKind, ConnectAttempt, ConnectStep, HandshakeHealth, StepResult};

/// A profile's state and its last Connect or Recover as the pipe carries them.
pub struct WireAttempts;

impl WireAttempts {
    /// `health` is the connected profile's handshake health as last judged.
    pub fn state(state: ProfileState, health: Option<HandshakeHealth>) -> ipc::ProfileState {
        match state {
            ProfileState::Off => ipc::ProfileState::Off,
            ProfileState::Connecting { step } => ipc::ProfileState::Connecting(Self::step(step)),
            ProfileState::Connected { health: at_connect } => {
                ipc::ProfileState::Connected(health.unwrap_or(at_connect))
            }
            ProfileState::Disconnecting => ipc::ProfileState::Disconnecting,
            ProfileState::Failed => ipc::ProfileState::Failed,
            ProfileState::Blocked => ipc::ProfileState::Blocked,
            ProfileState::HeldOff => ipc::ProfileState::HeldOff,
        }
    }

    pub fn attempt(attempt: &ConnectAttempt) -> ipc::Attempt {
        ipc::Attempt {
            at: attempt.at,
            kind: match attempt.kind {
                AttemptKind::Connect => ipc::AttemptKind::Connect,
                AttemptKind::Recover => ipc::AttemptKind::Recover,
            },
            result: if attempt.succeeded() {
                ipc::AttemptResult::Succeeded
            } else {
                ipc::AttemptResult::Failed
            },
            step: attempt.error.as_ref().map(|(step, _)| Self::step(*step)),
            error: attempt.error.as_ref().map(|(_, error)| error.clone()),
            steps: attempt
                .steps
                .iter()
                .map(|outcome| ipc::StepOutcome {
                    step: Self::step(outcome.step),
                    result: Self::result(outcome.result),
                    detail: outcome.detail.clone(),
                })
                .collect(),
        }
    }

    pub fn step(step: ConnectStep) -> ipc::ConnectStep {
        ipc::ConnectStep::ALL[step as usize]
    }

    fn result(result: StepResult) -> ipc::StepResult {
        match result {
            StepResult::Done => ipc::StepResult::Done,
            StepResult::Undone => ipc::StepResult::Undone,
            StepResult::Skipped => ipc::StepResult::Skipped,
            StepResult::Failed => ipc::StepResult::Failed,
            StepResult::NotRun => ipc::StepResult::NotRun,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::UNIX_EPOCH;

    #[test]
    fn steps_keep_their_ids_and_a_failure_its_step() {
        for step in ConnectStep::ALL {
            assert_eq!(WireAttempts::step(step).id(), step.id());
        }
        let mut attempt = ConnectAttempt::new(UNIX_EPOCH, AttemptKind::Connect);
        attempt.set(ConnectStep::Profile, StepResult::Done, "keys decrypted");
        attempt.set(ConnectStep::Proxy, StepResult::Failed, "port 53 busy");
        attempt.set(ConnectStep::Profile, StepResult::Undone, "");
        let wire = WireAttempts::attempt(&attempt);
        assert_eq!(wire.result, ipc::AttemptResult::Failed);
        assert_eq!(wire.step, Some(ipc::ConnectStep::Proxy));
        assert_eq!(wire.error.as_deref(), Some("port 53 busy"));
        assert_eq!(wire.steps[0].result, ipc::StepResult::Undone);
        assert_eq!(wire.steps[0].detail, "keys decrypted");
        assert_eq!(wire.steps[9].result, ipc::StepResult::NotRun);
        assert_eq!(wire.summary().kind, ipc::AttemptKind::Connect);
    }

    #[test]
    fn a_connected_state_reports_the_current_health() {
        let state = ProfileState::Connected {
            health: HandshakeHealth::Ok,
        };
        assert_eq!(
            WireAttempts::state(state, Some(HandshakeHealth::Stale)),
            ipc::ProfileState::Connected(HandshakeHealth::Stale)
        );
        assert_eq!(
            WireAttempts::state(
                ProfileState::Connecting {
                    step: ConnectStep::Peer
                },
                None
            ),
            ipc::ProfileState::Connecting(ipc::ConnectStep::Peer)
        );
        assert_eq!(
            WireAttempts::state(ProfileState::HeldOff, None),
            ipc::ProfileState::HeldOff
        );
    }
}
