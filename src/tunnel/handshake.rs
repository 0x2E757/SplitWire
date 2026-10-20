use std::time::{Duration, SystemTime};

use super::TunnelHealth;

/// How a connected tunnel's peer is doing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HandshakeHealth {
    Ok,
    /// No handshake within 30 s of up.
    Handshaking,
    /// The last handshake is older than 180 s while the tunnel keeps sending.
    Stale,
}

/// Judges one tunnel's handshakes over successive health reads. It remembers the traffic sent
/// at the previous read: the tunnel keeps sending when that grew at this read and the one
/// before, so neither a keepalive just after a handshake nor a handshake still under way at one
/// read counts.
#[derive(Clone, Debug)]
pub struct HandshakePolicy {
    up_at: SystemTime,
    tx_bytes: u64,
    sending: bool,
}

impl HandshakePolicy {
    pub const HANDSHAKING_AFTER: Duration = Duration::from_secs(30);
    pub const STALE_AFTER: Duration = Duration::from_secs(180);

    pub fn new(up_at: SystemTime) -> Self {
        Self {
            up_at,
            tx_bytes: 0,
            sending: false,
        }
    }

    pub fn judge(&mut self, now: SystemTime, health: &TunnelHealth) -> HandshakeHealth {
        let sending = health.tx_bytes > self.tx_bytes;
        let kept_sending = sending && self.sending;
        (self.tx_bytes, self.sending) = (health.tx_bytes, sending);
        let age = |since: SystemTime| now.duration_since(since).unwrap_or_default();
        let Some(handshake) = health.last_handshake else {
            return if age(self.up_at) >= Self::HANDSHAKING_AFTER {
                HandshakeHealth::Handshaking
            } else {
                HandshakeHealth::Ok
            };
        };
        if age(handshake) > Self::STALE_AFTER && kept_sending {
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
    fn a_keepalive_just_after_a_handshake_leaves_an_idle_tunnel_ok() {
        let mut policy = HandshakePolicy::new(at(1_000));
        let ok = HandshakeHealth::Ok;
        assert_eq!(policy.judge(at(1_010), &health(Some(1_005), 100)), ok);
        // The passive keepalive, sent after the read that saw the handshake.
        assert_eq!(policy.judge(at(1_020), &health(Some(1_005), 132)), ok);
        for secs in [1_190, 1_200, 1_600] {
            assert_eq!(policy.judge(at(secs), &health(Some(1_005), 132)), ok);
        }
    }

    #[test]
    fn stale_while_the_tunnel_keeps_sending_without_a_handshake_until_one_comes() {
        let mut policy = HandshakePolicy::new(at(1_000));
        assert_eq!(
            policy.judge(at(1_010), &health(Some(1_005), 100)),
            HandshakeHealth::Ok
        );
        assert_eq!(
            policy.judge(at(1_290), &health(Some(1_005), 100)),
            HandshakeHealth::Ok
        );
        // Sending again after idle: a handshake under way at one read is not stale yet.
        assert_eq!(
            policy.judge(at(1_300), &health(Some(1_005), 400)),
            HandshakeHealth::Ok
        );
        assert_eq!(
            policy.judge(at(1_310), &health(Some(1_005), 700)),
            HandshakeHealth::Stale
        );
        assert_eq!(
            policy.judge(at(1_320), &health(Some(1_005), 900)),
            HandshakeHealth::Stale
        );
        // The handshake comes: ok, though the tunnel still sends.
        assert_eq!(
            policy.judge(at(1_330), &health(Some(1_325), 1_200)),
            HandshakeHealth::Ok
        );
        // Silent after the old handshake's read: no longer stale either.
        let mut quiet = HandshakePolicy::new(at(1_000));
        quiet.judge(at(1_290), &health(Some(1_005), 100));
        quiet.judge(at(1_300), &health(Some(1_005), 400));
        quiet.judge(at(1_310), &health(Some(1_005), 700));
        assert_eq!(
            quiet.judge(at(1_320), &health(Some(1_005), 700)),
            HandshakeHealth::Ok
        );
    }
}
