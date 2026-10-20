use std::fmt::Write;

use super::Times;
use crate::ipc::{
    Attempt, AttemptKind, AttemptResult, ProfileDetail, ProfileState, ProfileSummary,
    RouteGroupKey, RouteRow, StatusReport, StepResult, TrayState,
};
use crate::tunnel::HandshakeHealth;

/// `list` and `status` as text: the profiles table, the engine's status, and one profile with
/// its routes and the step table of its last attempt.
pub struct StatusText {
    times: Times,
}

impl StatusText {
    pub fn new(times: Times) -> Self {
        Self { times }
    }

    /// The UI's wording of a state.
    pub fn state(&self, profile: &ProfileSummary) -> String {
        match profile.state {
            ProfileState::Off => match profile.last_connected {
                Some(at) => format!("off · last connected {}", self.times.at(at)),
                None => "off".into(),
            },
            ProfileState::Connecting(step) => {
                format!("connecting · step {} of 10 · {}", step.number(), step.id())
            }
            ProfileState::Connected(health) => {
                let up = profile
                    .uptime_secs
                    .map_or(String::new(), |secs| format!(" · {}", Times::span(secs)));
                match health {
                    HandshakeHealth::Ok => format!("connected{up}"),
                    HandshakeHealth::Handshaking => format!("connected (handshaking){up}"),
                    HandshakeHealth::Stale => format!("connected (handshake stale){up}"),
                }
            }
            ProfileState::Disconnecting => "disconnecting".into(),
            ProfileState::Failed => "connect failed".into(),
            ProfileState::Blocked => "can't connect".into(),
            ProfileState::HeldOff => "off · splitwire.conf is invalid".into(),
        }
    }

    /// One line per profile, in the user's order: state and handshake.
    pub fn profiles(&self, profiles: &[ProfileSummary]) -> String {
        if profiles.is_empty() {
            return "No profiles. Import one with: SplitWire.exe import <file.conf>\n".into();
        }
        let width = profiles
            .iter()
            .map(|p| p.name.len())
            .max()
            .unwrap_or(0)
            .max(7);
        let mut text = format!("{:<width$}  {:<38}  Handshake\n", "Profile", "State");
        for profile in profiles {
            let handshake = profile
                .handshake_age_secs
                .map_or("-".into(), |secs| format!("{} ago", Times::span(secs)));
            let _ = writeln!(
                text,
                "{:<width$}  {:<38}  {handshake}",
                profile.name,
                self.state(profile)
            );
            if let Some(problem) = &profile.problem {
                let _ = writeln!(text, "{:<width$}  {problem}", "");
            }
        }
        text
    }

    pub fn status(&self, status: &StatusReport) -> String {
        let mut text = String::new();
        let _ = writeln!(
            text,
            "SplitWire {}, running since {}",
            status.version,
            self.times.at(status.since)
        );
        let driver = status
            .wgnt_version
            .map_or("not loaded yet".into(), |v| v.to_string());
        let _ = writeln!(text, "wireguard-nt:   {driver}");
        let _ = writeln!(text, "Tray state:     {}", Self::tray(status.tray_state));
        let q = &status.counters.queries;
        let _ = writeln!(
            text,
            "Queries:        {} (listed {}, bypassed {}, other {}, failed {}, cache hits {})",
            q.total, q.listed, q.bypassed, q.other, q.failed, q.cache_hits
        );
        let _ = writeln!(
            text,
            "Cache:          {} entries, {} learned routes",
            status.counters.cache_entries, status.counters.learned_routes
        );
        let note = status
            .nrpt
            .note
            .as_ref()
            .map_or(String::new(), |n| format!(" ({n})"));
        let _ = writeln!(text, "NRPT rules:     {}{note}", status.nrpt.rules);
        let fallback: Vec<String> = status.fallback.iter().map(ToString::to_string).collect();
        let _ = writeln!(
            text,
            "Fallback DNS:   {}",
            Self::or_none(&fallback.join(", "))
        );
        for problem in &status.problems {
            let _ = writeln!(text, "Problem:        {problem}");
        }
        let dismissed = status.alerts.iter().filter(|a| a.dismissed).count();
        let _ = writeln!(
            text,
            "Alerts:         {} active, {dismissed} dismissed (SplitWire.exe alerts)",
            status.alerts.len() - dismissed
        );
        let _ = writeln!(
            text,
            "Revisions:      profiles {}, lists {:x}, settings {:x}, alerts {}",
            status.revs.profiles, status.revs.lists, status.revs.settings, status.revs.alerts
        );
        text.push('\n');
        text.push_str(&self.profiles(&status.profiles));
        text
    }

    pub fn detail(&self, detail: &ProfileDetail) -> String {
        let summary = &detail.summary;
        let mut text = format!("{}: {}\n", summary.name, self.state(summary));
        let mut row = |label: &str, value: String| {
            let _ = writeln!(text, "  {label:<14} {value}");
        };
        row("Colour", format!("pin {}", summary.colour.number()));
        row("Adapter", detail.adapter.clone());
        row("Address", detail.address.to_string());
        let mtu = match detail.mtu_override {
            Some(_) => format!("{} (profile.conf)", detail.mtu),
            None => detail.mtu.to_string(),
        };
        row("MTU", mtu);
        let resolved: Vec<String> = detail
            .endpoint_addresses
            .iter()
            .map(ToString::to_string)
            .collect();
        let endpoint = match resolved.is_empty() {
            true => detail.endpoint.to_string(),
            false => format!("{} -> {}", detail.endpoint, resolved.join(", ")),
        };
        row("Endpoint", endpoint);
        row("Peer key", detail.peer_public_key.clone());
        if let Some(key) = &detail.interface_public_key {
            row("Interface key", key.clone());
        }
        let upstream: Vec<String> = detail.upstream.iter().map(ToString::to_string).collect();
        let rtt = summary
            .dns_rtt_ms
            .map_or(String::new(), |ms| format!(" (DNS {ms} ms)"));
        row(
            "Upstream DNS",
            format!("{}{rtt}", Self::or_none(&upstream.join(", "))),
        );
        row(
            "Keepalive",
            match detail.keepalive {
                0 => "off".into(),
                secs => format!("{secs} s"),
            },
        );
        row("Imported", self.times.at(detail.imported));
        if summary.state != ProfileState::Off || summary.rx_total > 0 {
            row(
                "Traffic",
                format!(
                    "in {} out {} (total {} / {})",
                    Times::rate(summary.rx_rate),
                    Times::rate(summary.tx_rate),
                    Times::bytes(summary.rx_total),
                    Times::bytes(summary.tx_total)
                ),
            );
        }
        for problem in &detail.settings_problems {
            row("Problem", problem.to_string());
        }
        text.push_str(&self.list_and_routes(detail));
        if let Some(attempt) = &detail.last_attempt {
            text.push_str(&self.attempt(attempt));
        }
        text
    }

    /// The step table of an attempt.
    pub fn attempt(&self, attempt: &Attempt) -> String {
        let kind = match attempt.kind {
            AttemptKind::Connect => "Connect",
            AttemptKind::Recover => "Recover",
        };
        let result = match (attempt.result, attempt.step) {
            (AttemptResult::Succeeded, _) => "succeeded".to_owned(),
            (AttemptResult::Failed, Some(step)) => {
                format!("failed at step {} ({})", step.number(), step.id())
            }
            (AttemptResult::Failed, None) => "failed".to_owned(),
        };
        let mut text = format!("{kind} at {}: {result}\n", self.times.at(attempt.at));
        if let Some(error) = &attempt.error {
            let _ = writeln!(text, "  {error}");
        }
        for outcome in &attempt.steps {
            let row = format!(
                "  {:>2}  {:<8} {:<8} {}",
                outcome.step.number(),
                outcome.step.id(),
                Self::step_result(outcome.result),
                outcome.detail
            );
            let _ = writeln!(text, "{}", row.trim_end());
        }
        text
    }

    fn list_and_routes(&self, detail: &ProfileDetail) -> String {
        let mut text = String::from("List (routes.txt):\n");
        if detail.list.is_empty() {
            text.push_str("  (empty)\n");
        }
        for line in &detail.list {
            let routes = match line.routes {
                0 => String::new(),
                1 => "  1 route".into(),
                n => format!("  {n} routes"),
            };
            let note = line
                .note
                .as_ref()
                .map_or(String::new(), |n| format!("  [{n}]"));
            let _ = writeln!(text, "  {:>3}  {}{routes}{note}", line.line, line.text);
        }
        if detail.routes.iter().all(|g| g.rows.is_empty()) {
            return text;
        }
        text.push_str("Routes:\n");
        for group in detail.routes.iter().filter(|g| !g.rows.is_empty()) {
            let title = match &group.key {
                RouteGroupKey::Line { line, text } => format!("line {line}: {text}"),
                RouteGroupKey::Upstream => "upstream DNS".into(),
                RouteGroupKey::Refused => "refused".into(),
            };
            let _ = writeln!(text, "  {title}");
            for row in &group.rows {
                let _ = writeln!(text, "    {}", self.route(row));
            }
        }
        text
    }

    fn route(&self, row: &RouteRow) -> String {
        match row {
            RouteRow::Learned {
                ip, name, expires, ..
            } => format!(
                "{ip}/32  learned from {name}, expires {}",
                self.times.at(*expires)
            ),
            RouteRow::Static { net } => format!("{net}  static"),
            RouteRow::Upstream { ip } => format!("{ip}/32  upstream DNS"),
            RouteRow::Held {
                ip,
                name,
                by,
                by_name,
            } => format!("{ip}  {name}: held by {by} ({by_name})"),
            RouteRow::Refused { net, name, reason } => match name {
                Some(name) => format!("{net}  {name}: refused, {reason}"),
                None => format!("{net}  refused, {reason}"),
            },
        }
    }

    fn step_result(result: StepResult) -> &'static str {
        match result {
            StepResult::Done => "done",
            StepResult::Undone => "undone",
            StepResult::Skipped => "skipped",
            StepResult::Failed => "FAILED",
            StepResult::NotRun => "-",
        }
    }

    fn tray(state: TrayState) -> &'static str {
        match state {
            TrayState::Service => "service not running",
            TrayState::Error => "error",
            TrayState::Warning => "warning",
            TrayState::Connected => "connected",
            TrayState::Off => "off",
        }
    }

    fn or_none(text: &str) -> &str {
        if text.is_empty() { "none" } else { text }
    }
}
