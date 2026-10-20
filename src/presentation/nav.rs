//! The navigation pane: Status, one item per profile, Add profile, then Routes,
//! Lists, Logs and Settings pinned to the bottom, each with its value on the right.

use super::{Command, Dot, Format, Page, StateText};
use crate::ipc::{
    ListLineKind, ListsView, ProfileRoutes, ProfileState, ProfileSummary, RouteGroupKey, RouteRow,
    StatusReport,
};
use crate::profile::{ImportPreview, Pin};

/// The icon or swatch at an item's start.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NavLead {
    Icon(&'static str),
    Swatch(Pin),
}

/// The tone of a value's icon.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tone {
    Accent,
    Danger,
    Warning,
    Muted,
}

/// The value on an item's right.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NavValue {
    None,
    Text(String),
    Dot(Dot, String),
    Icon(&'static str, Tone, String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NavItem {
    pub label: String,
    pub lead: NavLead,
    pub value: NavValue,
    pub selected: bool,
    pub enabled: bool,
    /// The icon-only navigation shows it: the name and the state.
    pub tooltip: String,
    pub command: Command,
    /// A profile item opens the profile menu on right click and can be dragged.
    pub profile: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NavView {
    pub status: NavItem,
    pub profiles: Vec<NavItem>,
    pub add: NavItem,
    pub routes: NavItem,
    pub lists: NavItem,
    pub logs: NavItem,
    pub settings: NavItem,
}

/// What the navigation is built from.
pub struct NavSource<'a> {
    pub page: &'a Page,
    /// `None` while the service is not running.
    pub status: Option<&'a StatusReport>,
    /// No answer yet: neither a service nor its absence to show.
    pub waiting: bool,
    pub lists: Option<&'a ListsView>,
    pub routes: Option<&'a [ProfileRoutes]>,
}

impl NavSource<'_> {
    fn item(
        &self,
        label: &str,
        icon: &'static str,
        page: Page,
        value: NavValue,
        enabled: bool,
    ) -> NavItem {
        NavItem {
            label: label.into(),
            lead: NavLead::Icon(icon),
            value,
            selected: *self.page == page,
            enabled,
            tooltip: label.into(),
            command: Command::Go(page),
            profile: None,
        }
    }

    pub fn view(&self) -> NavView {
        let service = self.status.is_some();
        let profiles: &[ProfileSummary] = self.status.map_or(&[], |s| &s.profiles);
        let any = !profiles.is_empty();
        let connected = profiles
            .iter()
            .filter(|p| matches!(p.state, ProfileState::Connected(_)))
            .count();
        let status_value = match self.status {
            None if self.waiting => NavValue::None,
            None => NavValue::Dot(Dot::Error, "No service".into()),
            Some(_) if !any => NavValue::None,
            Some(_) => NavValue::Text(format!("{connected} of {}", profiles.len())),
        };
        let mut status = self.item("Status", "shield", Page::Status, status_value, true);
        status.selected &= service && any;
        let mut add = self.item(
            "Add profile",
            "plus",
            Page::Add,
            if any {
                NavValue::Text(format!("{} of {}", profiles.len(), ImportPreview::LIMIT))
            } else {
                NavValue::None
            },
            service,
        );
        add.selected = matches!(self.page, Page::Add | Page::Review)
            || (service && !any && *self.page == Page::Status);
        let pages_on = service && any;
        let number = |n: usize| {
            if pages_on {
                NavValue::Text(Format::count(n as u64))
            } else {
                NavValue::None
            }
        };
        let problems = self.status.map_or(0, |s| s.problems.len());
        NavView {
            status,
            profiles: profiles.iter().map(|p| self.profile(p)).collect(),
            add,
            routes: self.item(
                "Routes",
                "routes",
                Page::Routes,
                number(self.installed_routes()),
                pages_on,
            ),
            lists: self.item(
                "Lists",
                "list",
                Page::Lists,
                number(self.entries()),
                pages_on,
            ),
            logs: self.item("Logs", "logs", Page::Logs, NavValue::None, pages_on),
            settings: self.item(
                "Settings",
                "gear",
                Page::Settings,
                if problems > 0 {
                    NavValue::Icon("error", Tone::Danger, problems.to_string())
                } else {
                    NavValue::None
                },
                service,
            ),
        }
    }

    fn profile(&self, profile: &ProfileSummary) -> NavItem {
        let value = match &profile.state {
            ProfileState::Blocked => NavValue::Icon("error", Tone::Danger, "Can’t connect".into()),
            ProfileState::Failed => NavValue::Icon("error", Tone::Danger, "Failed".into()),
            state => NavValue::Dot(StateText::dot(state), StateText::short(state).into()),
        };
        let page = Page::Profile(profile.name.clone());
        NavItem {
            label: profile.name.clone(),
            lead: NavLead::Swatch(profile.colour),
            value,
            selected: *self.page == page,
            enabled: true,
            tooltip: format!("{} · {}", profile.name, StateText::label(&profile.state)),
            command: Command::Go(page),
            profile: Some(profile.name.clone()),
        }
    }

    /// Learned, static and upstream /32s over the connected profiles; refused and held ones are
    /// not installed. Without the routes the learned count of `Status` stands in.
    fn installed_routes(&self) -> usize {
        match self.routes {
            Some(routes) => routes
                .iter()
                .flat_map(|p| &p.groups)
                .filter(|g| g.key != RouteGroupKey::Refused)
                .flat_map(|g| &g.rows)
                .filter(|r| {
                    matches!(
                        r,
                        RouteRow::Learned { .. }
                            | RouteRow::Static { .. }
                            | RouteRow::Upstream { .. }
                    )
                })
                .count(),
            None => self
                .status
                .map_or(0, |s| s.profiles.iter().map(|p| p.learned as usize).sum()),
        }
    }

    /// List entries (lines that are not comments or blank) over all profiles.
    fn entries(&self) -> usize {
        self.lists.map_or(0, |lists| {
            lists
                .files
                .iter()
                .flat_map(|f| &f.lines)
                .filter(|l| !matches!(l.kind, ListLineKind::Comment | ListLineKind::Blank))
                .count()
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::presentation::samples::Samples;

    #[test]
    fn values_follow_s9a() {
        let status = Samples::status("normal");
        let lists = Samples::lists("normal");
        let routes = Samples::routes("normal");
        let page = Page::Profile("AcmeCorp".into());
        let nav = NavSource {
            page: &page,
            status: Some(&status),
            lists: Some(&lists),
            routes: Some(&routes),
            waiting: false,
        }
        .view();
        assert_eq!(nav.status.value, NavValue::Text("2 of 3".into()));
        assert_eq!(nav.profiles[0].value, NavValue::Dot(Dot::Ok, "On".into()));
        assert_eq!(nav.profiles[1].value, NavValue::Dot(Dot::Ok, "On".into()));
        assert!(nav.profiles[1].selected);
        assert_eq!(
            nav.profiles[2].value,
            NavValue::Icon("error", Tone::Danger, "Can’t connect".into())
        );
        assert_eq!(nav.profiles[2].tooltip, "HomeLab · Can’t connect");
        assert_eq!(nav.add.value, NavValue::Text("3 of 16".into()));
        assert_eq!(nav.routes.value, NavValue::Text("27".into()));
        assert_eq!(nav.lists.value, NavValue::Text("9".into()));
        assert_eq!(nav.settings.value, NavValue::None);
    }

    #[test]
    fn without_the_service_only_status_is_enabled() {
        let nav = NavSource {
            page: &Page::Status,
            status: None,
            lists: None,
            routes: None,
            waiting: false,
        }
        .view();
        assert_eq!(
            nav.status.value,
            NavValue::Dot(Dot::Error, "No service".into())
        );
        assert!(!nav.add.enabled && !nav.routes.enabled && !nav.settings.enabled);
    }

    #[test]
    fn the_first_run_selects_add_and_keeps_settings() {
        let status = Samples::status("firstrun");
        let nav = NavSource {
            page: &Page::Status,
            status: Some(&status),
            lists: None,
            routes: None,
            waiting: false,
        }
        .view();
        assert!(nav.add.selected && !nav.status.selected);
        assert!(nav.add.enabled && nav.settings.enabled);
        assert!(!nav.routes.enabled && !nav.lists.enabled && !nav.logs.enabled);
        assert_eq!(nav.status.value, NavValue::None);
    }

    #[test]
    fn global_problems_mark_settings() {
        let status = Samples::status("settingsinvalid");
        let nav = NavSource {
            page: &Page::Settings,
            status: Some(&status),
            lists: None,
            routes: None,
            waiting: false,
        }
        .view();
        assert_eq!(
            nav.settings.value,
            NavValue::Icon("error", Tone::Danger, "1".into())
        );
        assert!(nav.settings.selected);
    }
}
