//! The demo service's state for each scenario.

use super::Scenario;
use super::alerts::DemoAlerts;
use super::logs::DemoLogs;
use super::model::DemoModel;
use super::profiles::DemoProfiles;
use crate::ipc::{
    Attempt, AttemptKind, AttemptResult, ConnectStep, ProfileState, StepOutcome, StepResult,
};

pub struct DemoScenarios;

impl DemoScenarios {
    const GLOBAL: &'static str = "# splitwire.conf\nlisten = 127.0.0.53:53\nbypass = mail.contoso.example\nmax_ttl = 300\nidle_expiry = 86400\n";

    pub fn model(scenario: Scenario) -> DemoModel {
        let mut model = Self::normal(scenario);
        match scenario {
            Scenario::Normal => {}
            Scenario::FirstRun => Self::first_run(&mut model),
            Scenario::NoService => model.service = false,
            Scenario::ConnectFail => Self::connect_fail(&mut model),
            Scenario::SettingsInvalid => Self::settings_invalid(&mut model),
        }
        model
    }

    fn normal(scenario: Scenario) -> DemoModel {
        DemoModel {
            service: true,
            since: Scenario::at(9, 12, 3),
            queries: 6240,
            listed: 1870,
            cache_entries: 418,
            nrpt_rules: 4,
            nrpt_note: Some("overridden by Group Policy".into()),
            fallback: vec!["192.168.1.1".parse().expect("an address")],
            global: Self::GLOBAL.into(),
            profiles: vec![
                DemoProfiles::contoso(),
                DemoProfiles::acme_corp(),
                DemoProfiles::home_lab(),
            ],
            alerts: DemoAlerts::normal(),
            history: DemoAlerts::history(),
            logs: DemoLogs::live(scenario),
            log_file: DemoLogs::file(scenario),
        }
    }

    fn first_run(model: &mut DemoModel) {
        model.profiles.clear();
        model.alerts.clear();
        model.history.clear();
        model.queries = 0;
        model.listed = 0;
        model.cache_entries = 0;
        model.nrpt_rules = 0;
        model.nrpt_note = Some("no rules".into());
    }

    fn connect_fail(model: &mut DemoModel) {
        let home = model.profile_mut("HomeLab").expect("HomeLab");
        home.state = ProfileState::Failed;
        home.settings = "connect_at_boot = false\n# mtu = 1420\n# upstream = 10.9.0.1\n".into();
        home.attempt = Some(Self::failed_attempt());
        model
            .alerts
            .retain(|a| a.profile.as_deref() != Some("HomeLab"));
        model.alerts.insert(0, DemoAlerts::connect_failed());
    }

    /// HomeLab's Connect, failed at step 3: the endpoint does not resolve.
    pub fn failed_attempt() -> Attempt {
        let results = [
            (StepResult::Undone, "keys zeroed again"),
            (StepResult::Skipped, "Contoso already runs the proxy"),
            (
                StepResult::Failed,
                "home.example.net: NXDOMAIN via 192.168.1.1",
            ),
        ];
        Attempt {
            at: Scenario::at(11, 27, 41),
            kind: AttemptKind::Connect,
            result: AttemptResult::Failed,
            step: Some(ConnectStep::Guard),
            error: Some(
                "home.example.net did not resolve through the fallback DNS 192.168.1.1 (NXDOMAIN)"
                    .into(),
            ),
            steps: ConnectStep::ALL
                .iter()
                .enumerate()
                .map(|(index, step)| {
                    let (result, detail) = results
                        .get(index)
                        .copied()
                        .unwrap_or((StepResult::NotRun, ""));
                    StepOutcome {
                        step: *step,
                        result,
                        detail: detail.into(),
                    }
                })
                .collect(),
        }
    }

    fn settings_invalid(model: &mut DemoModel) {
        model.global = Self::GLOBAL.replace("max_ttl = 300", "max_ttl = 10");
        let off = Scenario::at(11, 25, 1);
        for profile in &mut model.profiles {
            if profile.connected() {
                profile.disconnect(off);
                profile.state = ProfileState::HeldOff;
            }
            profile.routes.clear();
        }
        model.cache_entries = 0;
        model.nrpt_rules = 0;
        model.nrpt_note = Some("no rules while disconnected".into());
        model
            .alerts
            .retain(|a| a.profile.as_deref() == Some("HomeLab"));
        model.alerts.insert(0, DemoAlerts::settings_invalid());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_scenario_has_its_profiles_and_alerts() {
        let normal = DemoScenarios::model(Scenario::Normal);
        assert_eq!(normal.profiles.len(), 3);
        assert_eq!(normal.alerts.len(), 5);
        assert!(normal.global_problems().is_empty());
        assert_eq!(normal.profiles[2].settings_problems()[0].line, 2);
        assert!(DemoScenarios::model(Scenario::FirstRun).profiles.is_empty());
        assert!(!DemoScenarios::model(Scenario::NoService).service);
        let failed = DemoScenarios::model(Scenario::ConnectFail);
        assert_eq!(failed.profiles[2].state, ProfileState::Failed);
        assert!(failed.profiles[2].settings_problems().is_empty());
        let invalid = DemoScenarios::model(Scenario::SettingsInvalid);
        assert_eq!(invalid.global_problems()[0].line, 4);
        assert_eq!(invalid.profiles[0].state, ProfileState::HeldOff);
        assert_eq!(invalid.profiles[2].state, ProfileState::Blocked);
        assert!(invalid.alerts[0].blocking);
    }
}
