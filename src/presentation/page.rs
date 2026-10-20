/// The pages of the window. A profile page is named by its profile.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Page {
    Status,
    Profile(String),
    Routes,
    Lists,
    Logs,
    Settings,
    /// The drop zone; the first run shows it as the Welcome page.
    Add,
    /// The import review of the files picked or dropped.
    Review,
}

impl Page {
    /// A short key for element names and tests: `status`, `profile:AcmeCorp`.
    pub fn key(&self) -> String {
        match self {
            Self::Status => "status".into(),
            Self::Profile(name) => format!("profile:{name}"),
            Self::Routes => "routes".into(),
            Self::Lists => "lists".into(),
            Self::Logs => "logs".into(),
            Self::Settings => "settings".into(),
            Self::Add => "add".into(),
            Self::Review => "review".into(),
        }
    }

    /// Pages that need the service; without it the window shows Status only.
    pub fn needs_service(&self) -> bool {
        !matches!(self, Self::Status)
    }

    /// Pages that need at least one profile.
    pub fn needs_profiles(&self) -> bool {
        matches!(
            self,
            Self::Profile(_) | Self::Routes | Self::Lists | Self::Logs
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_and_requirements() {
        assert_eq!(Page::Profile("AcmeCorp".into()).key(), "profile:AcmeCorp");
        assert!(!Page::Status.needs_service());
        assert!(Page::Settings.needs_service());
        assert!(Page::Logs.needs_profiles());
        assert!(!Page::Add.needs_profiles());
    }
}
