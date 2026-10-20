use std::time::{Duration, SystemTime};

use super::TunnelHealth;

/// How a connected tunnel's peer is doing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HandshakeHealth {
    Ok,
    /// No handshake within 30 s of up.
    Handshaking,
    /// The last handshake is older than 180 s while the tunnel kept sending.
    Stale,
}

/// Judges one tunnel's handshakes over successive health reads. It remembers the traffic sent
/// at the last handshake it saw, so "kept sending" means sent since then.
#[derive(Clone, Debug)]
pub struct HandshakePolicy {
    up_at: SystemTime,
    handshake: Option<SystemTime>,
    tx_at_handshake: u64,
}

impl HandshakePolicy {
    pub const HANDSHAKING_AFTER: Duration = Duration::from_secs(30);
    pub const STALE_AFTER: Duration = Duration::from_secs(180);

    pub fn new(up_at: SystemTime) -> Self {
        Self {
            up_at,
            handshake: None,
            tx_at_handshake: 0,
        }
    }

    pub fn judge(&mut self, now: SystemTime, health: &TunnelHealth) -> HandshakeHealth {
        let age = |since: SystemTime| now.duration_since(since).unwrap_or_default();
        let Some(handshake) = health.last_handshake else {
            return if age(self.up_at) >= Self::HANDSHAKING_AFTER {
                HandshakeHealth::Handshaking
            } else {
                HandshakeHealth::Ok
            };
        };
        if self.handshake != Some(handshake) {
            self.handshake = Some(handshake);
            self.tx_at_handshake = health.tx_bytes;
        }
        if age(handshake) > Self::STALE_AFTER && health.tx_bytes > self.tx_at_handshake {
            HandshakeHealth::Stale
        } else {
            HandshakeHealth::Ok
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::UNIX_EPOCH;

    fn at(secs: u64) -> SystemTime {
        UNIX_EPOCH + Duration::from_secs(secs)
    }

    fn health(handshake: Option<u64>, tx_bytes: u64) -> TunnelHealth {
        TunnelHealth {
            interface_public_key: [0; 32],
            last_handshake: handshake.map(at),
            rx_bytes: 0,
            tx_bytes,
            endpoint: None,
        }
    }

    #[test]
    fn handshaking_only_after_30_seconds_without_a_handshake() {
        let mut policy = HandshakePolicy::new(at(1_000));
        assert_eq!(
            policy.judge(at(1_029), &health(None, 0)),
            HandshakeHealth::Ok
        );
        assert_eq!(
            policy.judge(at(1_030), &health(None, 0)),
            HandshakeHealth::Handshaking
        );
        assert_eq!(
            policy.judge(at(1_031), &health(Some(1_031), 0)),
            HandshakeHealth::Ok
        );
    }

    #[test]
    fn stale_when_an_old_handshake_meets_new_traffic() {
        let mut policy = HandshakePolicy::new(at(1_000));
        assert_eq!(
            policy.judge(at(1_010), &health(Some(1_005), 100)),
            HandshakeHealth::Ok
        );
        // Old, but nothing sent since: an idle tunnel is not stale.
        assert_eq!(
            policy.judge(at(1_300), &health(Some(1_005), 100)),
            HandshakeHealth::Ok
        );
        assert_eq!(
            policy.judge(at(1_310), &health(Some(1_005), 160)),
            HandshakeHealth::Stale
        );
        // A new handshake resets the reference.
        assert_eq!(
            policy.judge(at(1_320), &health(Some(1_318), 200)),
            HandshakeHealth::Ok
        );
    }
}
