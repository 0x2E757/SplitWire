use std::collections::BTreeMap;
use std::sync::Mutex;
use std::time::SystemTime;

use crate::profile::ProfileState;
use crate::tunnel::ConnectAttempt;

/// What status shows about one profile besides its tunnel.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProfileRecord {
    pub state: ProfileState,
    /// Kept until the next attempt (the step table).
    pub last_attempt: Option<ConnectAttempt>,
    /// When it was last disconnected ("Off · last connected …").
    pub last_connected: Option<SystemTime>,
}

impl Default for ProfileRecord {
    fn default() -> Self {
        Self {
            state: ProfileState::Off,
            last_attempt: None,
            last_connected: None,
        }
    }
}

/// Every profile's state, readable at any time: the coordinator writes it under its lock, status
/// and the tray state read it without waiting for a Connect.
#[derive(Default)]
pub struct ProfileStates {
    records: Mutex<BTreeMap<String, ProfileRecord>>,
}

impl ProfileStates {
    pub fn set(&self, name: &str, state: ProfileState) {
        self.edit(name, |record| record.state = state);
    }

    pub fn attempt(&self, name: &str, attempt: ConnectAttempt) {
        self.edit(name, |record| record.last_attempt = Some(attempt));
    }

    pub fn disconnected(&self, name: &str, at: SystemTime) {
        self.edit(name, |record| {
            record.state = ProfileState::Off;
            record.last_connected = Some(at);
        });
    }

    pub fn get(&self, name: &str) -> ProfileRecord {
        self.records
            .lock()
            .unwrap()
            .get(name)
            .cloned()
            .unwrap_or_default()
    }

    pub fn state(&self, name: &str) -> ProfileState {
        self.get(name).state
    }

    /// Forgets the profiles not in `names` (removed ones).
    pub fn retain(&self, names: &[String]) {
        self.records
            .lock()
            .unwrap()
            .retain(|name, _| names.contains(name));
    }

    fn edit(&self, name: &str, change: impl FnOnce(&mut ProfileRecord)) {
        change(
            self.records
                .lock()
                .unwrap()
                .entry(name.to_owned())
                .or_default(),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tunnel::AttemptKind;
    use std::time::UNIX_EPOCH;

    #[test]
    fn keeps_state_attempt_and_last_connected() {
        let states = ProfileStates::default();
        assert_eq!(states.state("A"), ProfileState::Off);
        states.set("A", ProfileState::Failed);
        states.attempt("A", ConnectAttempt::new(UNIX_EPOCH, AttemptKind::Connect));
        assert_eq!(states.get("A").state, ProfileState::Failed);
        assert!(states.get("A").last_attempt.is_some());
        states.disconnected("A", UNIX_EPOCH);
        assert_eq!(states.get("A").last_connected, Some(UNIX_EPOCH));
        states.retain(&[]);
        assert_eq!(states.get("A"), ProfileRecord::default());
    }
}
