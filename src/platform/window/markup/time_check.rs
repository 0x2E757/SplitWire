//! The markup half of the time check: while the clock runs and the service's numbers move on, no
//! region with a button, switch, menu or field changes its markup, so the window reloads none of
//! them and the focus and an open menu stay. Ages, uptimes and numbers live in parts of their own.

use std::net::Ipv4Addr;
use std::sync::Arc;
use std::time::Duration;

use super::typing_check::difference;
use super::{Frame, Region};
use crate::clock::{Clock, ManualClock};
use crate::ipc::{ProfileSummary, Request, Response, RouteGroup, RouteRow};
use crate::presentation::WidthClass;
use crate::presentation::showcase::{Driver, Showcase};
use crate::presentation::transport::demo::{DemoTransport, Scenario};
use crate::presentation::transport::{EngineTransport, TransportError};

/// The demo service with a running clock: uptimes, handshakes, rates, totals and counters move on
/// as the real service's do, a route is learned every half minute, and the routes' ages follow the
/// clock by themselves.
struct RunningService {
    demo: DemoTransport,
    clock: Arc<ManualClock>,
}

impl RunningService {
    fn elapsed(&self) -> u64 {
        self.clock
            .now()
            .duration_since(Scenario::now())
            .map_or(0, |d| d.as_secs())
    }

    fn age(profile: &mut ProfileSummary, secs: u64) {
        profile.uptime_secs = profile.uptime_secs.map(|u| u + secs);
        profile.handshake_age_secs = profile.handshake_age_secs.map(|h| h + secs % 25);
        profile.rx_rate += secs % 7 * 1_300;
        profile.tx_rate += secs % 5 * 400;
        profile.rx_total += secs * 90_000;
        profile.tx_total += secs * 20_000;
        profile.learned += secs / 10;
        profile.dns_rtt_ms = profile.dns_rtt_ms.map(|r| r + secs % 4);
    }

    /// The routes learned since the start, into the first group that learns any.
    fn learn(&self, groups: &mut [RouteGroup], secs: u64) {
        let learns =
            |g: &&mut RouteGroup| g.rows.iter().any(|r| matches!(r, RouteRow::Learned { .. }));
        let Some(group) = groups.iter_mut().find(learns) else {
            return;
        };
        let now = self.clock.now();
        for n in 0..secs / 30 {
            group.rows.push(RouteRow::Learned {
                ip: Ipv4Addr::new(198, 51, 100, (n % 250) as u8 + 1),
                name: format!("new{n}.example"),
                last_seen: now,
                expires: now + Duration::from_secs(86_400),
            });
        }
    }
}

impl EngineTransport for RunningService {
    fn exchange(&mut self, request: &Request) -> Result<Response, TransportError> {
        let secs = self.elapsed();
        let mut response = self.demo.exchange(request)?;
        match &mut response {
            Response::Status(status) => {
                for profile in &mut status.profiles {
                    Self::age(profile, secs);
                }
                status.counters.queries.total += secs * 3;
                status.counters.queries.listed += secs;
                status.counters.cache_entries += secs / 2;
                status.counters.learned_routes += secs / 10;
            }
            Response::ProfileStatus(detail) => {
                Self::age(&mut detail.summary, secs);
                self.learn(&mut detail.routes, secs);
            }
            Response::Routes(profiles) => {
                for profile in profiles {
                    self.learn(&mut profile.groups, secs);
                }
            }
            _ => {}
        }
        Ok(response)
    }
}

/// A region holds something the user acts on: a bind or a field, its own or a child's.
fn controls(region: &Region) -> bool {
    !region.binds.is_empty() || !region.inputs.is_empty() || region.children.iter().any(controls)
}

/// The regions with controls whose markup or binds changed, as the window would reload them.
fn reloaded(before: &[Region], after: &[Region], out: &mut Vec<String>) {
    let binds = |r: &Region| {
        r.binds
            .iter()
            .map(|b| (b.element.clone(), b.event.clone()))
            .collect::<Vec<_>>()
    };
    for old in before {
        let Some(new) = after.iter().find(|r| r.host == old.host) else {
            continue;
        };
        if new.markup != old.markup || binds(new) != binds(old) {
            if controls(old) {
                out.push(format!(
                    "{} changed{}",
                    old.host,
                    difference(&old.markup, &new.markup)
                ));
            }
        } else {
            reloaded(&old.children, &new.children, out);
        }
    }
}

#[test]
fn time_reloads_no_region_with_controls() {
    let mut failures = Vec::new();
    for scenario in Scenario::ALL {
        for width in WidthClass::ALL {
            let clock = Arc::new(ManualClock::new(Scenario::now()));
            let service = RunningService {
                demo: DemoTransport::new(scenario),
                clock: clock.clone(),
            };
            let driver = Driver::over(
                Box::new(service),
                Box::new(clock.clone()),
                Scenario::OFFSET_SECS,
            );
            Showcase::walk_with(driver, scenario, |label, driver| {
                // Seconds under a minute, then a minute: both kinds of span move.
                for step in [1, 1, 60] {
                    let step = Duration::from_secs(step);
                    let before = Frame::regions(&driver.session.view(), width);
                    clock.advance(step);
                    driver.later(step);
                    let after = Frame::regions(&driver.session.view(), width);
                    let mut changed = Vec::new();
                    reloaded(&before, &after, &mut changed);
                    failures.extend(
                        changed
                            .into_iter()
                            .map(|c| format!("{label} {width:?}: {} s later {c}", step.as_secs())),
                    );
                }
            });
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
