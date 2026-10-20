use std::net::Ipv4Addr;

use super::{AlertAction, AlertKind, AlertRaise, Suggestion};
use crate::config::SettingProblem;
use crate::list::ListProblem;
use crate::tunnel::ConnectAttempt;

/// The wording of every alert, one constructor per condition.
pub struct AlertTexts;

impl AlertTexts {
    pub fn settings_invalid(problems: &[SettingProblem]) -> AlertRaise {
        let first = &problems[0];
        let mut raised = Self::base(
            AlertKind::SettingsInvalid,
            None,
            "",
            "splitwire.conf is invalid".into(),
            format!(
                "Line {}: {}. Every profile stays disconnected until it is fixed.",
                first.line,
                Self::problem_text(first)
            ),
            Self::fingerprint_of(problems),
        );
        raised.action = Self::action("Open settings", format!("settings#global.{}", first.key));
        raised.suggest = Self::suggestion("global", first);
        raised
    }

    pub fn profile_settings_invalid(profile: &str, problems: &[SettingProblem]) -> AlertRaise {
        let first = &problems[0];
        let mut raised = Self::base(
            AlertKind::ProfileSettingsInvalid,
            Some(profile),
            "",
            format!("{profile} cannot connect"),
            format!(
                "profile.conf line {}: {}. Only {profile} is blocked.",
                first.line,
                Self::problem_text(first)
            ),
            Self::fingerprint_of(problems),
        );
        raised.action = Self::action("Open settings", format!("profile:{profile}#{}", first.key));
        raised.suggest = Self::suggestion(&format!("profile:{profile}"), first);
        raised
    }

    pub fn connect_failed(profile: &str, attempt: &ConnectAttempt) -> AlertRaise {
        let (step, error) = attempt
            .error
            .clone()
            .unwrap_or((crate::tunnel::ConnectStep::Profile, "unknown error".into()));
        let undone: Vec<&str> = attempt.undone().iter().map(|s| s.id()).collect();
        let undone = if undone.is_empty() {
            "nothing".to_owned()
        } else {
            undone.join(", ")
        };
        let mut raised = Self::base(
            AlertKind::ConnectFailed,
            Some(profile),
            "",
            format!(
                "Connect {profile} failed at step {} ({step})",
                step.number()
            ),
            format!("{error}. Undone: {undone}. Nothing else changed."),
            format!("{step}:{error}"),
        );
        raised.action = Self::open_profile(profile);
        raised
    }

    pub fn recover_failed(profile: &str, error: &str) -> AlertRaise {
        let mut raised = Self::base(
            AlertKind::RecoverFailed,
            Some(profile),
            "",
            format!("{profile} could not be restored"),
            format!(
                "The adapter could not be brought back: {error}. It is tried again every 10 s."
            ),
            error.into(),
        );
        raised.action = Self::action("Show in log", format!("logs:{profile}"));
        raised
    }

    pub fn upstream_unreachable(profile: &str, servers: &str, error: &str) -> AlertRaise {
        let mut raised = Self::base(
            AlertKind::UpstreamUnreachable,
            Some(profile),
            "",
            format!("{profile}'s DNS does not answer"),
            format!("{servers}: {error}. Listed names of {profile} fail until it answers."),
            servers.into(),
        );
        raised.action = Self::open_profile(profile);
        raised
    }

    pub fn nrpt_write_failed(error: &str) -> AlertRaise {
        let mut raised = Self::base(
            AlertKind::NrptWriteFailed,
            None,
            "",
            "NRPT rules could not be written".into(),
            format!("{error}. Listed names may not reach SplitWire."),
            error.into(),
        );
        raised.action = Self::action("Show in log", "logs".into());
        raised
    }

    pub fn handshake_stale(profile: &str) -> AlertRaise {
        let mut raised = Self::base(
            AlertKind::HandshakeStale,
            Some(profile),
            "",
            "Stale handshake".into(),
            "No handshake for 3 min while traffic was sent; the endpoint was re-resolved.".into(),
            "stale".into(),
        );
        raised.action = Self::open_profile(profile);
        raised
    }

    /// Connect finished without the peer's first handshake: the same condition as a stale one,
    /// cleared by a handshake.
    pub fn handshake_missing(profile: &str, waited_secs: u64) -> AlertRaise {
        let mut raised = Self::base(
            AlertKind::HandshakeStale,
            Some(profile),
            "",
            "No handshake yet".into(),
            format!(
                "The peer did not answer within {waited_secs} s of Connect; listed names of \
                 {profile} may fail until it does."
            ),
            "missing".into(),
        );
        raised.action = Self::open_profile(profile);
        raised
    }

    pub fn address_held(profile: &str, ip: Ipv4Addr, name: &str, holder: &str) -> AlertRaise {
        let mut raised = Self::base(
            AlertKind::AddressHeld,
            Some(profile),
            &ip.to_string(),
            format!("{ip} is held by {holder}"),
            format!(
                "Learned by {holder} and also answered for {profile} ({name}): {holder} holds it, \
                 so that traffic goes through {holder}."
            ),
            holder.into(),
        );
        raised.action = Self::open_profile(profile);
        raised
    }

    pub fn list_problem(profile: &str, problems: &[ListProblem]) -> AlertRaise {
        let lines: Vec<String> = problems
            .iter()
            .map(|p| format!("line {}: {}", p.line, p.reason))
            .collect();
        let count = problems.len();
        let mut raised = Self::base(
            AlertKind::ListProblem,
            Some(profile),
            "",
            format!(
                "{profile}'s list skips {count} line{}",
                if count == 1 { "" } else { "s" }
            ),
            format!("{}.", lines.join("; ")),
            lines.join("\n"),
        );
        raised.action = Self::action("Open list", format!("profile:{profile}#list"));
        raised
    }

    pub fn gp_nrpt() -> AlertRaise {
        let mut raised = Self::base(
            AlertKind::GpNrpt,
            None,
            "",
            "NRPT rules from Group Policy override SplitWire".into(),
            "Listed names are not routed while a policy defines its own rules.".into(),
            "gp".into(),
        );
        raised.action = Self::action("Show in log", "logs".into());
        raised
    }

    pub fn route_refused(profile: &str, net: &str, origin: &str, reason: &str) -> AlertRaise {
        let mut raised = Self::base(
            AlertKind::RouteRefused,
            Some(profile),
            net,
            format!("Route refused: {net}"),
            format!("{origin}: {reason}."),
            reason.into(),
        );
        raised.action = Self::open_profile(profile);
        raised
    }

    fn base(
        kind: AlertKind,
        profile: Option<&str>,
        subject: &str,
        title: String,
        message: String,
        fingerprint: String,
    ) -> AlertRaise {
        AlertRaise {
            kind,
            profile: profile.map(Into::into),
            subject: subject.into(),
            title,
            message,
            action: None,
            suggest: None,
            fingerprint,
        }
    }

    fn problem_text(problem: &SettingProblem) -> String {
        if problem.key.is_empty() {
            problem.reason.clone()
        } else {
            format!("{} = {} {}", problem.key, problem.value, problem.reason)
        }
    }

    fn fingerprint_of(problems: &[SettingProblem]) -> String {
        problems
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn suggestion(scope: &str, problem: &SettingProblem) -> Option<Suggestion> {
        problem.suggest.as_ref().map(|value| Suggestion {
            scope: scope.into(),
            key: problem.key.clone(),
            value: value.clone(),
        })
    }

    fn action(label: &str, target: String) -> Option<AlertAction> {
        Some(AlertAction {
            label: label.into(),
            target,
        })
    }

    fn open_profile(profile: &str) -> Option<AlertAction> {
        Self::action("Open profile", format!("profile:{profile}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tunnel::{AttemptKind, ConnectStep, StepResult};
    use std::time::UNIX_EPOCH;

    #[test]
    fn a_failed_connect_names_its_step_and_what_was_undone() {
        let mut attempt = ConnectAttempt::new(UNIX_EPOCH, AttemptKind::Connect);
        attempt.set(ConnectStep::Profile, StepResult::Done, "keys decrypted");
        attempt.set(ConnectStep::Proxy, StepResult::Skipped, "running");
        attempt.set(
            ConnectStep::Guard,
            StepResult::Failed,
            "home.example.net did not resolve",
        );
        attempt.set(
            ConnectStep::Profile,
            StepResult::Undone,
            "keys zeroed again",
        );
        let raised = AlertTexts::connect_failed("HomeLab", &attempt);
        assert_eq!(raised.title, "Connect HomeLab failed at step 3 (guard)");
        assert_eq!(
            raised.message,
            "home.example.net did not resolve. Undone: profile. Nothing else changed."
        );
        assert_eq!(raised.id(), "connect_failed:HomeLab:");
    }

    #[test]
    fn settings_problems_carry_their_line_and_fix() {
        let problem = SettingProblem {
            file: "profiles\\HomeLab\\profile.conf".into(),
            line: 2,
            key: "mtu".into(),
            value: "9000".into(),
            reason: "must be a number 1280-1500".into(),
            suggest: Some("1420".into()),
        };
        let raised =
            AlertTexts::profile_settings_invalid("HomeLab", std::slice::from_ref(&problem));
        assert_eq!(
            raised.message,
            "profile.conf line 2: mtu = 9000 must be a number 1280-1500. Only HomeLab is blocked."
        );
        assert_eq!(raised.suggest.unwrap().value, "1420");
        let global = AlertTexts::settings_invalid(&[problem]);
        assert!(
            global
                .message
                .ends_with("Every profile stays disconnected until it is fixed.")
        );
        assert_eq!(global.action.unwrap().target, "settings#global.mtu");
    }

    #[test]
    fn a_missing_handshake_is_the_stale_handshake_condition() {
        let raised = AlertTexts::handshake_missing("AcmeCorp", 10);
        assert_eq!(raised.title, "No handshake yet");
        assert_eq!(
            raised.message,
            "The peer did not answer within 10 s of Connect; listed names of AcmeCorp may fail \
             until it does."
        );
        assert_eq!(raised.id(), AlertTexts::handshake_stale("AcmeCorp").id());
    }

    #[test]
    fn a_held_address_names_the_holder() {
        let raised = AlertTexts::address_held(
            "AcmeCorp",
            Ipv4Addr::new(192, 0, 2, 93),
            "cdn.acme.example",
            "Contoso",
        );
        assert_eq!(raised.title, "192.0.2.93 is held by Contoso");
        assert_eq!(raised.id(), "address_held:AcmeCorp:192.0.2.93");
    }
}
