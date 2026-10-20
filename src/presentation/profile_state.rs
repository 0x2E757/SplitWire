//! A profile's state as every surface words it: the dot, the label
//! and the pill with its tail.

use super::{Format, LocalTime};
use crate::ipc::{AttemptResult, ProfileState, ProfileSummary};

/// The state dot: accent with a ring, the status colour, hollow, or danger.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Dot {
    Ok,
    Busy,
    Off,
    Error,
}

/// A state pill: dot, label and an optional quiet tail after a `·`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pill {
    pub dot: Dot,
    pub label: String,
    pub tail: Option<String>,
}

impl Pill {
    pub fn new(dot: Dot, label: &str, tail: Option<String>) -> Self {
        Self {
            dot,
            label: label.to_owned(),
            tail: tail.filter(|t| !t.is_empty()),
        }
    }
}

pub struct StateText;

impl StateText {
    pub fn dot(state: &ProfileState) -> Dot {
        match state {
            ProfileState::Connected(_) => Dot::Ok,
            ProfileState::Connecting(_) | ProfileState::Disconnecting => Dot::Busy,
            ProfileState::Failed | ProfileState::Blocked => Dot::Error,
            ProfileState::Off | ProfileState::HeldOff => Dot::Off,
        }
    }

    pub fn label(state: &ProfileState) -> &'static str {
        match state {
            ProfileState::Connected(_) => "Connected",
            ProfileState::Connecting(_) => "Connecting",
            ProfileState::Disconnecting => "Disconnecting",
            ProfileState::Failed => "Connect failed",
            ProfileState::Blocked => "Can’t connect",
            ProfileState::Off | ProfileState::HeldOff => "Off",
        }
    }

    /// The menus' shorter word, leaving room for the profile's name: On and Off.
    pub fn short(state: &ProfileState) -> &'static str {
        match state {
            ProfileState::Connected(_) => "On",
            ProfileState::Failed => "Failed",
            _ => Self::label(state),
        }
    }

    /// The Connect switch is on while connected or connecting.
    pub fn switched_on(state: &ProfileState) -> bool {
        matches!(
            state,
            ProfileState::Connected(_) | ProfileState::Connecting(_)
        )
    }

    pub fn active(state: &ProfileState) -> bool {
        Self::switched_on(state)
    }

    /// Connect is refused while the profile's or the global settings are invalid.
    pub fn can_connect(state: &ProfileState, global_invalid: bool) -> bool {
        !global_invalid && !matches!(state, ProfileState::Blocked | ProfileState::HeldOff)
    }

    /// `step 5 of 10 · peer`.
    pub fn step(state: &ProfileState) -> Option<String> {
        match state {
            ProfileState::Connecting(step) => {
                Some(format!("step {} of 10 · {}", step.number(), step.id()))
            }
            _ => None,
        }
    }

    /// The pill of the profile page and the Routes cards.
    pub fn pill(profile: &ProfileSummary, time: &LocalTime) -> Pill {
        let state = &profile.state;
        let tail = match state {
            ProfileState::Connected(_) => profile.uptime_secs.map(Format::span),
            ProfileState::Connecting(_) => Self::step(state),
            ProfileState::Failed => profile
                .last_attempt
                .filter(|a| a.result == AttemptResult::Failed)
                .map(|a| time.clock(a.at)),
            ProfileState::Blocked => Some("profile.conf".into()),
            ProfileState::HeldOff => Some("splitwire.conf is invalid".into()),
            ProfileState::Off => Self::last_connected(profile, time),
            ProfileState::Disconnecting => None,
        };
        Pill::new(Self::dot(state), Self::label(state), tail)
    }

    /// `last connected yesterday 22:10`.
    pub fn last_connected(profile: &ProfileSummary, time: &LocalTime) -> Option<String> {
        profile
            .last_connected
            .map(|at| format!("last connected {}", time.day_clock(at)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ipc::ConnectStep;
    use crate::presentation::samples::Samples;
    use crate::tunnel::HandshakeHealth;

    #[test]
    fn dots_and_labels() {
        assert_eq!(
            StateText::dot(&ProfileState::Connected(HandshakeHealth::Ok)),
            Dot::Ok
        );
        assert_eq!(StateText::dot(&ProfileState::Blocked), Dot::Error);
        assert_eq!(StateText::label(&ProfileState::Blocked), "Can’t connect");
        assert_eq!(StateText::label(&ProfileState::HeldOff), "Off");
        assert_eq!(StateText::short(&ProfileState::Off), "Off");
        assert_eq!(StateText::short(&ProfileState::Blocked), "Can’t connect");
        assert!(StateText::switched_on(&ProfileState::Connecting(
            ConnectStep::Peer
        )));
        assert!(!StateText::can_connect(&ProfileState::Off, true));
        assert!(!StateText::can_connect(&ProfileState::Blocked, false));
        assert_eq!(
            StateText::step(&ProfileState::Connecting(ConnectStep::Peer)).unwrap(),
            "step 5 of 10 · peer"
        );
    }

    #[test]
    fn pills_carry_their_tail() {
        let time = Samples::time();
        let mut profile =
            Samples::summary("AcmeCorp", ProfileState::Connected(HandshakeHealth::Ok));
        profile.uptime_secs = Some(47 * 60);
        assert_eq!(
            StateText::pill(&profile, &time),
            Pill::new(Dot::Ok, "Connected", Some("47 min".into()))
        );
        profile.state = ProfileState::Off;
        profile.last_connected = Some(Samples::ago(13 * 3600 + 18 * 60 + 55));
        assert_eq!(
            StateText::pill(&profile, &time).tail.unwrap(),
            "last connected yesterday 22:10"
        );
        profile.state = ProfileState::HeldOff;
        assert_eq!(
            StateText::pill(&profile, &time).tail.unwrap(),
            "splitwire.conf is invalid"
        );
    }
}
