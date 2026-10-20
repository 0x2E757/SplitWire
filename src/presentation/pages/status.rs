//! Status: global stats and alerts, one large card per connected or connecting profile with
//! its own chart, one slim line per other profile. Without the service: a message that says so.

use std::time::SystemTime;

use super::{Stat, SwitchView};
use crate::ipc::{AttemptResult, ProfileState, ProfileSummary, StatusReport, TrafficSample};
use crate::presentation::{
    AlertList, AlertView, ChartView, Command, Format, LocalTime, Page, Pill, StateText,
    TrafficChart,
};
use crate::profile::Pin;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BigCard {
    pub name: String,
    pub colour: Pin,
    pub pill: Pill,
    /// `connects at boot`.
    pub note: Option<String>,
    pub switch: SwitchView,
    pub stats: Vec<Stat>,
    pub chart: ChartView,
    pub alerts: Vec<AlertView>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SlimCard {
    pub name: String,
    pub colour: Pin,
    pub pill: Pill,
    /// Fix or Details, both to the profile page.
    pub action: Option<(String, Command)>,
    pub switch: SwitchView,
    pub alerts: Vec<AlertView>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StatusPageView {
    pub subtitle: String,
    pub stats: Vec<Stat>,
    pub alerts: Vec<AlertView>,
    pub big: Vec<BigCard>,
    pub slim: Vec<SlimCard>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ServiceDownView {
    /// `Last answer from the service 11:26:40`.
    pub subtitle: Option<String>,
    pub command: String,
}

impl ServiceDownView {
    pub const COMMAND: &'static str = "SplitWire.exe install";

    pub fn new(last_answer: Option<SystemTime>, time: &LocalTime) -> Self {
        Self {
            subtitle: last_answer
                .map(|at| format!("Last answer from the service {}", time.clock_seconds(at))),
            command: Self::COMMAND.into(),
        }
    }
}

/// What the Status page is built from.
pub struct StatusSource<'a> {
    pub status: &'a StatusReport,
    pub traffic: &'a dyn Fn(&str) -> Vec<TrafficSample>,
    /// The proxy's address, from the settings when they are known.
    pub listen: String,
    /// Whether a profile connects at boot, from its `profile.conf` when the settings are known.
    pub boot: &'a dyn Fn(&str) -> bool,
    pub time: &'a LocalTime,
}

impl StatusSource<'_> {
    pub fn view(&self) -> StatusPageView {
        let status = self.status;
        let connected: Vec<&ProfileSummary> = status
            .profiles
            .iter()
            .filter(|p| matches!(p.state, ProfileState::Connected(_)))
            .collect();
        let alerts = AlertList::new(&status.alerts);
        let counters = &status.counters;
        let stats = vec![
            Stat::new(
                "DNS proxy",
                &self.listen,
                &status
                    .fallback
                    .first()
                    .map(|ip| format!("fallback {ip}"))
                    .unwrap_or_else(|| "fallback: none".into()),
            ),
            Stat::new(
                "Queries",
                &Format::count(counters.queries.total),
                &format!("{} listed", Format::count(counters.queries.listed)),
            ),
            Stat::new(
                "Learned routes",
                &Format::count(counters.learned_routes),
                &connected
                    .iter()
                    .map(|p| format!("{} {}", p.name, p.learned))
                    .collect::<Vec<_>>()
                    .join(" · "),
            ),
            Stat::new("Cache", &Format::count(counters.cache_entries), "entries"),
            Stat::new(
                "NRPT",
                &format!("{} rules", status.nrpt.rules),
                status.nrpt.note.as_deref().unwrap_or_default(),
            ),
        ];
        let global_invalid = !status.problems.is_empty();
        let (active, rest): (Vec<&ProfileSummary>, Vec<&ProfileSummary>) = status
            .profiles
            .iter()
            .partition(|p| StateText::active(&p.state));
        StatusPageView {
            subtitle: format!(
                "{} of {} profiles connected",
                connected.len(),
                status.profiles.len()
            ),
            stats,
            alerts: alerts.views(Some(None), self.time),
            big: active
                .into_iter()
                .map(|p| self.big(p, &alerts, global_invalid))
                .collect(),
            slim: rest
                .into_iter()
                .map(|p| self.slim(p, &alerts, global_invalid))
                .collect(),
        }
    }

    fn big(&self, p: &ProfileSummary, alerts: &AlertList, global_invalid: bool) -> BigCard {
        let connecting = matches!(p.state, ProfileState::Connecting(_));
        let pill = if connecting {
            Pill::new(
                StateText::dot(&p.state),
                "Connecting",
                StateText::step(&p.state),
            )
        } else {
            Pill::new(
                StateText::dot(&p.state),
                "Connected",
                p.uptime_secs.map(Format::span),
            )
        };
        let stats = if connecting {
            let dash = |label: &str, caption: &str| Stat::new(label, "—", caption);
            vec![
                dash("Handshake", &Self::keepalive(p.keepalive)),
                dash("Received", ""),
                dash("Sent", ""),
                dash("Routes (DNS)", ""),
            ]
        } else {
            vec![
                Stat::new(
                    "Handshake",
                    &p.handshake_age_secs
                        .map_or_else(|| "—".to_owned(), Format::ago),
                    &Self::keepalive(p.keepalive),
                ),
                Stat::new(
                    "Received",
                    &Format::rate(p.rx_rate),
                    &format!("{} total", Format::bytes(p.rx_total)),
                ),
                Stat::new(
                    "Sent",
                    &Format::rate(p.tx_rate),
                    &format!("{} total", Format::bytes(p.tx_total)),
                ),
                Stat::new("Routes (DNS)", &Format::count(p.learned), &Self::dns(p)),
            ]
        };
        BigCard {
            name: p.name.clone(),
            colour: p.colour,
            pill,
            note: (self.boot)(&p.name).then(|| "connects at boot".to_owned()),
            switch: SwitchView::of(p, global_invalid),
            stats,
            chart: if connecting {
                TrafficChart::empty("Handshaking…", "waiting for the first handshake", self.time)
            } else {
                TrafficChart::of(&(self.traffic)(&p.name), self.time)
            },
            alerts: alerts.views(Some(Some(&p.name)), self.time),
        }
    }

    /// `keepalive 25 s`, `keepalive off`.
    fn keepalive(secs: u16) -> String {
        match secs {
            0 => "keepalive off".into(),
            secs => format!("keepalive {secs} s"),
        }
    }

    /// The tunnel's DNS server and its last answer's time: `DNS 10.66.0.53 · 18 ms`.
    fn dns(p: &ProfileSummary) -> String {
        let server = p.upstream.map(|s| format!("DNS {}", s.ip()));
        let rtt = p.dns_rtt_ms.map(|ms| format!("{ms} ms"));
        match (server, rtt) {
            (Some(server), Some(rtt)) => format!("{server} · {rtt}"),
            (Some(server), None) => server,
            (None, Some(rtt)) => format!("DNS {rtt}"),
            (None, None) => String::new(),
        }
    }

    fn slim(&self, p: &ProfileSummary, alerts: &AlertList, global_invalid: bool) -> SlimCard {
        let page = Command::Go(Page::Profile(p.name.clone()));
        let (label, tail, action) = match &p.state {
            _ if global_invalid && p.state != ProfileState::Blocked => {
                ("Off", Some("splitwire.conf is invalid".to_owned()), None)
            }
            ProfileState::Blocked => (
                "Can’t connect",
                p.problem.clone(),
                Some(("Fix".to_owned(), page)),
            ),
            ProfileState::Failed => (
                "Connect failed",
                p.last_attempt
                    .filter(|a| a.result == AttemptResult::Failed)
                    .map(|a| {
                        let step = a.step.unwrap_or(crate::ipc::ConnectStep::Profile);
                        format!(
                            "step {} {} · {}",
                            step.number(),
                            step.id(),
                            self.time.clock(a.at)
                        )
                    }),
                Some(("Details".to_owned(), page)),
            ),
            state => (
                StateText::label(state),
                StateText::last_connected(p, self.time),
                None,
            ),
        };
        SlimCard {
            name: p.name.clone(),
            colour: p.colour,
            pill: Pill::new(StateText::dot(&p.state), label, tail),
            action,
            switch: SwitchView::of(p, global_invalid),
            alerts: alerts.views(Some(Some(&p.name)), self.time),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::presentation::Dot;
    use crate::presentation::samples::Samples;

    fn view(key: &str) -> StatusPageView {
        let status = Samples::status(key);
        let traffic = |_: &str| -> Vec<TrafficSample> {
            (1..=300)
                .map(|seq| TrafficSample {
                    seq,
                    rx_bps: 30_000,
                    tx_bps: 5_000,
                })
                .collect()
        };
        StatusSource {
            status: &status,
            traffic: &traffic,
            listen: "127.0.0.53".into(),
            boot: &|name: &str| name == "Contoso",
            time: &Samples::time(),
        }
        .view()
    }

    #[test]
    fn normal_shows_two_cards_and_one_line() {
        let view = view("normal");
        assert_eq!(view.subtitle, "2 of 3 profiles connected");
        assert_eq!(view.stats[0].caption, "fallback 192.168.1.1");
        assert_eq!(view.stats[1].value, "6 240");
        assert_eq!(view.stats[2].caption, "Contoso 14 · AcmeCorp 8");
        assert_eq!(view.stats[4].value, "4 rules");
        assert_eq!(view.alerts.len(), 1);
        assert_eq!(view.big.len(), 2);
        let acme = &view.big[1];
        assert_eq!(view.big[0].note.as_deref(), Some("connects at boot"));
        assert_eq!(acme.note, None);
        let labels: Vec<&str> = acme.stats.iter().map(|s| s.label.as_str()).collect();
        assert_eq!(labels, ["Handshake", "Received", "Sent", "Routes (DNS)"]);
        assert_eq!(acme.stats[0].value, "1 min ago");
        assert_eq!(acme.stats[0].caption, "keepalive 25 s");
        assert_eq!(acme.stats[1].value, "12 KB/s");
        assert_eq!(acme.stats[1].caption, "86.4 MB total");
        assert_eq!(acme.stats[2].value, "1.7 KB/s");
        assert_eq!(acme.stats[2].caption, "12.9 MB total");
        assert_eq!(acme.stats[3].value, "8");
        assert_eq!(acme.stats[3].caption, "DNS 10.88.0.53 · 24 ms");
        assert_eq!(acme.switch.tip, "Disconnect AcmeCorp");
        assert_eq!(acme.alerts.len(), 1);
        let home = &view.slim[0];
        assert_eq!(home.pill.dot, Dot::Error);
        assert!(
            home.pill
                .tail
                .as_deref()
                .unwrap()
                .starts_with("profile.conf line 2")
        );
        assert_eq!(home.action.as_ref().unwrap().0, "Fix");
        assert!(!home.switch.enabled);
    }

    #[test]
    fn a_failed_connect_offers_details() {
        let view = view("connectfail");
        let home = &view.slim[0];
        assert_eq!(home.pill.label, "Connect failed");
        assert_eq!(home.pill.tail.as_deref(), Some("step 3 guard · 11:27"));
        assert_eq!(home.action.as_ref().unwrap().0, "Details");
        assert!(home.switch.enabled);
    }

    #[test]
    fn invalid_settings_slim_every_card() {
        let view = view("settingsinvalid");
        assert!(view.big.is_empty());
        assert_eq!(
            view.slim[0].pill.tail.as_deref(),
            Some("splitwire.conf is invalid")
        );
        assert!(!view.slim[0].switch.enabled);
        assert_eq!(view.alerts[0].title, "splitwire.conf is invalid");
    }

    #[test]
    fn the_service_down_view_names_the_last_answer() {
        let down = ServiceDownView::new(Some(Samples::ago(135)), &Samples::time());
        assert_eq!(
            down.subtitle.unwrap(),
            "Last answer from the service 11:26:40"
        );
    }
}
