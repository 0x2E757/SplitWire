use crate::tunnel::{ConnectStep, HandshakeHealth};

/// Where a profile stands; what status reports and the tray state reads.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProfileState {
    Off,
    Connecting {
        step: ConnectStep,
    },
    Connected {
        health: HandshakeHealth,
    },
    Disconnecting,
    /// The last Connect or Recover failed; its attempt is kept.
    Failed,
    /// Its `profile.conf` is invalid.
    Blocked,
    /// `splitwire.conf` is invalid: every profile stays off.
    HeldOff,
}

impl ProfileState {
    pub fn id(&self) -> &'static str {
        match self {
            ProfileState::Off => "off",
            ProfileState::Connecting { .. } => "connecting",
            ProfileState::Connected { .. } => "connected",
            ProfileState::Disconnecting => "disconnecting",
            ProfileState::Failed => "failed",
            ProfileState::Blocked => "blocked",
            ProfileState::HeldOff => "held_off",
        }
    }

    pub fn is_connected(&self) -> bool {
        matches!(self, ProfileState::Connected { .. })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_each_state() {
        let connecting = ProfileState::Connecting {
            step: ConnectStep::Guard,
        };
        assert_eq!(connecting.id(), "connecting");
        assert_eq!(ProfileState::HeldOff.id(), "held_off");
        assert!(
            ProfileState::Connected {
                health: HandshakeHealth::Stale
            }
            .is_connected()
        );
        assert!(!ProfileState::Failed.is_connected());
    }
}
