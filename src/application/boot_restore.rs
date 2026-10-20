use crate::config::{ProfileSettings, StateFile};
use crate::profile::ProfileName;

/// Which profiles connect at start: those in `connected=` with `connect_at_boot`;
/// after an unclean stop (the crash marker was found) or `--restore`, every listed one. With an
/// invalid `splitwire.conf` none, and `connected=` is left as is.
pub struct BootRestore;

impl BootRestore {
    /// `profiles` are the stored profiles with their `profile.conf`, `None` when it is invalid.
    pub fn plan(
        state: &StateFile,
        settings_valid: bool,
        restore_all: bool,
        profiles: &[(ProfileName, Option<ProfileSettings>)],
    ) -> Vec<ProfileName> {
        if !settings_valid {
            return Vec::new();
        }
        state
            .connected()
            .iter()
            .filter_map(|listed| profiles.iter().find(|(name, _)| name.same_as(listed)))
            .filter(|(_, settings)| {
                restore_all || settings.as_ref().is_some_and(|s| s.connect_at_boot)
            })
            .map(|(name, _)| name.clone())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn profiles() -> Vec<(ProfileName, Option<ProfileSettings>)> {
        let at_boot = |on: bool| {
            Some(ProfileSettings {
                connect_at_boot: on,
                ..ProfileSettings::default()
            })
        };
        vec![
            (ProfileName::parse("AcmeCorp").unwrap(), at_boot(true)),
            (ProfileName::parse("Contoso").unwrap(), at_boot(false)),
            (ProfileName::parse("HomeLab").unwrap(), None),
        ]
    }

    fn names(plan: Vec<ProfileName>) -> Vec<String> {
        plan.iter().map(ToString::to_string).collect()
    }

    #[test]
    fn a_clean_start_restores_only_connect_at_boot() {
        let state = StateFile::parse("connected=contoso,acmecorp,HomeLab,Gone\n");
        assert_eq!(
            names(BootRestore::plan(&state, true, false, &profiles())),
            ["AcmeCorp"]
        );
    }

    #[test]
    fn after_a_crash_every_listed_profile_comes_back() {
        let state = StateFile::parse("connected=Contoso,HomeLab,Gone\n");
        assert_eq!(
            names(BootRestore::plan(&state, true, true, &profiles())),
            ["Contoso", "HomeLab"]
        );
    }

    #[test]
    fn an_invalid_splitwire_conf_restores_nothing() {
        let state = StateFile::parse("connected=AcmeCorp\n");
        assert!(BootRestore::plan(&state, false, true, &profiles()).is_empty());
    }
}
