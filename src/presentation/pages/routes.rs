//! Routes (view-only): totals, a search, one summary card per profile that opens in place to
//! its grouped routes. A search opens the matching cards.

use super::{InfoBar, RouteGroupView, RouteGroups};
use crate::ipc::{ListLineKind, ListsView, ProfileRoutes, ProfileState, RouteRow, StatusReport};
use crate::presentation::{Format, LocalTime, Pill, StateText};
use crate::profile::Pin;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RouteCardView {
    pub name: String,
    pub colour: Pin,
    pub pill: Pill,
    pub caption: String,
    /// Entries, Learned, Static, Refused or Held by other, Last learned.
    pub numbers: Vec<(String, String)>,
    pub open: bool,
    pub groups: Vec<RouteGroupView>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RoutesPageView {
    pub subtitle: String,
    pub message: Option<InfoBar>,
    pub cards: Vec<RouteCardView>,
    /// No card matches the search.
    pub no_match: Option<String>,
}

pub struct RoutesSource<'a> {
    pub status: &'a StatusReport,
    pub routes: &'a [ProfileRoutes],
    pub lists: Option<&'a ListsView>,
    pub search: &'a str,
    pub open: &'a dyn Fn(&str) -> bool,
    pub time: &'a LocalTime,
}

impl RoutesSource<'_> {
    pub fn view(&self) -> RoutesPageView {
        let rows = || {
            self.routes
                .iter()
                .flat_map(|p| &p.groups)
                .flat_map(|g| &g.rows)
        };
        let routed = rows()
            .filter(|r| !matches!(r, RouteRow::Refused { .. } | RouteRow::Held { .. }))
            .count();
        let held = rows()
            .filter(|r| matches!(r, RouteRow::Held { .. }))
            .count();
        let refused = rows()
            .filter(|r| matches!(r, RouteRow::Refused { .. }))
            .count();
        let message = self.status.problems.first().map(|p| {
            InfoBar::new(
                "No routes: every profile is disconnected",
                &format!(
                    "splitwire.conf is invalid (line {}: {} = {} {}).",
                    p.line, p.key, p.value, p.reason
                ),
            )
        });
        let searching = !self.search.trim().is_empty();
        let cards: Vec<RouteCardView> = self
            .status
            .profiles
            .iter()
            .filter_map(|p| {
                let groups = self
                    .routes
                    .iter()
                    .find(|r| r.name == p.name)
                    .map_or(&[][..], |r| &r.groups[..]);
                let lines = self
                    .lists
                    .and_then(|l| l.files.iter().find(|f| f.name == p.name))
                    .map_or(&[][..], |f| &f.lines[..]);
                let connected = matches!(p.state, ProfileState::Connected(_));
                let views = RouteGroups {
                    profile: &p.name,
                    lines,
                    groups,
                    connected,
                    search: self.search,
                    time: self.time,
                }
                .views(true);
                if searching && views.is_empty() {
                    return None;
                }
                let all = groups.iter().flat_map(|g| &g.rows);
                let count = |f: fn(&RouteRow) -> bool| all.clone().filter(|r| f(r)).count();
                let learned = count(|r| matches!(r, RouteRow::Learned { .. }));
                let statics = count(|r| matches!(r, RouteRow::Static { .. }));
                let held = count(|r| matches!(r, RouteRow::Held { .. }));
                let refused = count(|r| matches!(r, RouteRow::Refused { .. }));
                let last = all
                    .clone()
                    .filter_map(|r| match r {
                        RouteRow::Learned { last_seen, .. } => Some(*last_seen),
                        _ => None,
                    })
                    .max()
                    .map(|at| Format::ago(self.time.age(at)));
                let entries = lines
                    .iter()
                    .filter(|l| !matches!(l.kind, ListLineKind::Comment | ListLineKind::Blank))
                    .count();
                let live = |v: String| if connected { v } else { "—".into() };
                let mut numbers = vec![
                    ("Entries".to_owned(), entries.to_string()),
                    ("Learned".to_owned(), live(learned.to_string())),
                    ("Static".to_owned(), live(statics.to_string())),
                ];
                numbers.push(if held > 0 {
                    ("Held by other".to_owned(), live(held.to_string()))
                } else {
                    ("Refused".to_owned(), live(refused.to_string()))
                });
                numbers.push((
                    "Last learned".to_owned(),
                    live(last.unwrap_or_else(|| "—".into())),
                ));
                let caption = match &p.state {
                    ProfileState::Connected(_) => format!(
                        "since {} · {}",
                        p.since.map(|s| self.time.clock(s)).unwrap_or_default(),
                        p.uptime_secs.map(Format::span).unwrap_or_default()
                    ),
                    _ if !self.status.problems.is_empty() => {
                        "off: splitwire.conf is invalid".into()
                    }
                    ProfileState::Blocked => p.problem.clone().unwrap_or_default(),
                    ProfileState::Failed => StateText::pill(p, self.time)
                        .tail
                        .map(|t| format!("connect failed at {t}"))
                        .unwrap_or_default(),
                    _ => StateText::last_connected(p, self.time).unwrap_or_default(),
                };
                Some(RouteCardView {
                    name: p.name.clone(),
                    colour: p.colour,
                    pill: Pill::new(StateText::dot(&p.state), StateText::label(&p.state), None),
                    caption,
                    numbers,
                    open: searching || (self.open)(&p.name),
                    groups: views,
                })
            })
            .collect();
        RoutesPageView {
            subtitle: format!(
                "{} · {routed} routed · {held} held by another profile · {refused} refused",
                Format::plural(self.status.profiles.len(), "profile", "profiles")
            ),
            message,
            no_match: (cards.is_empty() && searching)
                .then(|| format!("No route or list entry contains “{}”.", self.search.trim())),
            cards,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::presentation::samples::Samples;

    #[test]
    fn totals_cards_and_search() {
        let status = Samples::status("normal");
        let routes = Samples::routes("normal");
        let lists = Samples::lists("normal");
        let closed = |_: &str| false;
        let source = RoutesSource {
            status: &status,
            routes: &routes,
            lists: Some(&lists),
            search: "",
            open: &closed,
            time: &Samples::time(),
        };
        let view = source.view();
        assert_eq!(
            view.subtitle,
            "3 profiles · 27 routed · 1 held by another profile · 1 refused"
        );
        assert_eq!(view.cards.len(), 3);
        let contoso = &view.cards[0];
        assert_eq!(contoso.caption, "since 09:14 · 2 h 14 min");
        assert_eq!(contoso.numbers[1], ("Learned".into(), "14".into()));
        assert_eq!(contoso.numbers[3], ("Refused".into(), "1".into()));
        assert_eq!(
            contoso.numbers[4],
            ("Last learned".into(), "12 s ago".into())
        );
        assert_eq!(
            view.cards[1].numbers[3],
            ("Held by other".into(), "1".into())
        );
        assert_eq!(view.cards[2].numbers[1].1, "—");
        assert!(!contoso.open);
        let found = RoutesSource {
            search: "sso",
            ..source
        }
        .view();
        assert_eq!(found.cards.len(), 1);
        assert!(found.cards[0].open);
        let none = RoutesSource {
            search: "nothing",
            ..source
        }
        .view();
        assert_eq!(
            none.no_match.unwrap(),
            "No route or list entry contains “nothing”."
        );
    }
}
