use std::time::{Duration, SystemTime};

use super::Tunnel;
use crate::clock::Clock;

/// Connect waits for the peer's first handshake before NRPT sends listed names to the tunnel:
/// the driver's last-handshake time is read until it is set, for at most `WAIT`.
pub struct HandshakeWait<'a> {
    tunnel: &'a dyn Tunnel,
    clock: &'a dyn Clock,
}

impl<'a> HandshakeWait<'a> {
    pub const WAIT: Duration = Duration::from_secs(10);
    pub const POLL: Duration = Duration::from_millis(250);

    pub fn new(tunnel: &'a dyn Tunnel, clock: &'a dyn Clock) -> Self {
        Self { tunnel, clock }
    }

    /// The time of `name`'s last handshake as the driver reports it now; `None` before the first,
    /// or when the adapter cannot be read.
    pub fn current(&self, name: &str) -> Option<SystemTime> {
        self.tunnel.health(name).ok()?.last_handshake
    }

    /// The time of `name`'s first handshake; `None` when none came within `WAIT` of `since`
    /// (the start of the wait, before any probe), or the adapter cannot be read.
    pub fn wait(&self, name: &str, since: SystemTime) -> Option<SystemTime> {
        loop {
            if let Some(at) = self.tunnel.health(name).ok()?.last_handshake {
                return Some(at);
            }
            let waited = self.clock.now().duration_since(since).unwrap_or_default();
            if waited >= Self::WAIT {
                return None;
            }
            self.clock.sleep(Self::POLL.min(Self::WAIT - waited));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::clock::ManualClock;
    use crate::tunnel::{DriverVersion, TunnelHealth, TunnelLink, TunnelSpec};
    use std::io;
    use std::sync::Mutex;
    use std::time::UNIX_EPOCH;

    /// A tunnel whose peer handshakes at the `handshake_at`-th health read (never when `None`).
    struct Handshaking {
        handshake_at: Option<u32>,
        reads: Mutex<u32>,
        broken: bool,
    }

    impl Handshaking {
        fn new(handshake_at: Option<u32>) -> Self {
            Self {
                handshake_at,
                reads: Mutex::new(0),
                broken: false,
            }
        }
    }

    impl Tunnel for Handshaking {
        fn up(&self, _spec: &TunnelSpec) -> io::Result<TunnelLink> {
            unreachable!()
        }
        fn down(&self, _name: &str) -> io::Result<()> {
            unreachable!()
        }
        fn health(&self, _name: &str) -> io::Result<TunnelHealth> {
            if self.broken {
                return Err(io::ErrorKind::NotFound.into());
            }
            let mut reads = self.reads.lock().unwrap();
            *reads += 1;
            let done = self.handshake_at.is_some_and(|at| *reads >= at);
            Ok(TunnelHealth {
                interface_public_key: [0; 32],
                last_handshake: done.then_some(UNIX_EPOCH + Duration::from_secs(7)),
                rx_bytes: 0,
                tx_bytes: 0,
                endpoint: None,
            })
        }
        fn refresh(&self, _spec: &TunnelSpec) -> io::Result<TunnelLink> {
            unreachable!()
        }
        fn driver_version(&self) -> Option<DriverVersion> {
            None
        }
    }

    fn clock() -> ManualClock {
        ManualClock::new(UNIX_EPOCH + Duration::from_secs(1_000))
    }

    #[test]
    fn returns_the_handshake_once_the_driver_reports_one() {
        let clock = clock();
        let tunnel = Handshaking::new(Some(6));
        let wait = HandshakeWait::new(&tunnel, &clock);
        assert_eq!(wait.current("AcmeCorp"), None);
        let at = wait.wait("AcmeCorp", clock.now());
        assert_eq!(at, Some(UNIX_EPOCH + Duration::from_secs(7)));
        assert_eq!(*tunnel.reads.lock().unwrap(), 6);
        // Four polls of 250 ms between the five reads.
        assert_eq!(
            clock.now(),
            UNIX_EPOCH + Duration::from_secs(1_000) + Duration::from_secs(1)
        );
    }

    #[test]
    fn gives_up_after_ten_seconds_or_at_once_when_the_adapter_is_gone() {
        let clock = clock();
        let tunnel = Handshaking::new(None);
        // Two seconds went to a probe first: they count toward the ten.
        let since = clock.now();
        clock.advance(Duration::from_secs(2));
        assert_eq!(
            HandshakeWait::new(&tunnel, &clock).wait("AcmeCorp", since),
            None
        );
        assert_eq!(clock.now(), UNIX_EPOCH + Duration::from_secs(1_010));
        assert_eq!(*tunnel.reads.lock().unwrap(), 33);

        let mut broken = Handshaking::new(Some(1));
        broken.broken = true;
        let wait = HandshakeWait::new(&broken, &clock);
        assert_eq!(wait.current("AcmeCorp"), None);
        assert_eq!(wait.wait("AcmeCorp", clock.now()), None);
        assert_eq!(clock.now(), UNIX_EPOCH + Duration::from_secs(1_010));
    }
}
