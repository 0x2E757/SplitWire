//! A profile's routes grouped by list line, shared by the profile page and Routes: a group
//! per list line with the routes it produced, a held-by-another-profile address in place, then
//! the Upstream DNS and Refused groups.

use crate::ipc::{ListLine, ListLineKind, RouteGroup, RouteGroupKey, RouteRow};
use crate::presentation::{Format, LocalTime};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GroupIcon {
    Domain,
    Network,
    Upstream,
    Refused,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RouteCellView {
    Learned {
        ip: String,
        name: String,
        last: String,
        expires: String,
    },
    Static {
        net: String,
    },
    Upstream {
        net: String,
    },
    Held {
        ip: String,
        name: String,
        by: String,
        detail: String,
    },
    Refused {
        net: String,
        name: String,
        reason: String,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RouteGroupView {
    /// The id its chevron toggles.
    pub id: String,
    pub title: String,
    pub icon: GroupIcon,
    pub caption: String,
    pub rows: Vec<RouteCellView>,
    /// Shown instead of rows when there are none.
    pub empty: String,
    /// The list line a remove button takes away.
    pub line: Option<(usize, String)>,
}

pub struct RouteGroups<'a> {
    pub profile: &'a str,
    pub lines: &'a [ListLine],
    pub groups: &'a [RouteGroup],
    pub connected: bool,
    pub search: &'a str,
    pub time: &'a LocalTime,
}

impl RouteGroups<'_> {
    fn cell(&self, row: &RouteRow) -> RouteCellView {
        match row {
            RouteRow::Learned {
                ip,
                name,
                last_seen,
                expires,
            } => RouteCellView::Learned {
                ip: ip.to_string(),
                name: name.clone(),
                last: Format::ago(self.time.age(*last_seen)),
                expires: Format::within(self.time.until(*expires)),
            },
            RouteRow::Static { net } => RouteCellView::Static {
                net: net.to_string(),
            },
            RouteRow::Upstream { ip } => RouteCellView::Upstream {
                net: format!("{ip}/32"),
            },
            RouteRow::Held {
                ip,
                name,
                by,
                by_name,
            } => RouteCellView::Held {
                ip: ip.to_string(),
                name: name.clone(),
                by: by.clone(),
                detail: format!("first learner holds it ({by_name})"),
            },
            RouteRow::Refused { net, name, reason } => RouteCellView::Refused {
                net: if net.prefix() == 32 {
                    net.addr().to_string()
                } else {
                    net.to_string()
                },
                name: name.clone().unwrap_or_default(),
                reason: reason.clone(),
            },
        }
    }

    fn matches(&self, row: &RouteCellView) -> bool {
        let query = self.search.trim().to_lowercase();
        if query.is_empty() {
            return true;
        }
        let texts: Vec<&str> = match row {
            RouteCellView::Learned { ip, name, .. }
            | RouteCellView::Held { ip, name, .. }
            | RouteCellView::Refused { net: ip, name, .. } => vec![ip, name],
            RouteCellView::Static { net } | RouteCellView::Upstream { net } => vec![net],
        };
        texts.iter().any(|t| t.to_lowercase().contains(&query))
    }

    fn rows(&self, key: &RouteGroupKey) -> Vec<RouteCellView> {
        self.groups
            .iter()
            .filter(|g| match (&g.key, key) {
                (RouteGroupKey::Line { line: a, .. }, RouteGroupKey::Line { line: b, .. }) => {
                    a == b
                }
                (a, b) => a == b,
            })
            .flat_map(|g| &g.rows)
            .map(|r| self.cell(r))
            .filter(|r| self.matches(r))
            .collect()
    }

    /// `hide_unmatched`: a search hides groups with no match (Routes); the profile page keeps them.
    pub fn views(&self, hide_unmatched: bool) -> Vec<RouteGroupView> {
        let searching = !self.search.trim().is_empty();
        let mut views = Vec::new();
        for line in self
            .lines
            .iter()
            .filter(|l| !matches!(l.kind, ListLineKind::Comment | ListLineKind::Blank))
        {
            let key = RouteGroupKey::Line {
                line: line.line,
                text: line.text.trim().to_owned(),
            };
            let rows = self.rows(&key);
            let title_match = line
                .text
                .to_lowercase()
                .contains(&self.search.trim().to_lowercase());
            if hide_unmatched && searching && rows.is_empty() && !title_match {
                continue;
            }
            let learned = rows
                .iter()
                .filter(|r| matches!(r, RouteCellView::Learned { .. }))
                .count();
            let held = rows
                .iter()
                .filter(|r| matches!(r, RouteCellView::Held { .. }))
                .count();
            let mut bits = vec![format!("line {}", line.line)];
            match line.kind {
                ListLineKind::Domain => {
                    if self.connected {
                        bits.push(Format::plural(learned, "route", "routes"));
                    }
                    if held > 0 {
                        bits.push(format!("{held} held by another profile"));
                    }
                    bits.push("and its subdomains".into());
                }
                ListLineKind::Network => bits.push("network".into()),
                ListLineKind::Address => bits.push("address".into()),
                _ => bits.push("skipped".into()),
            }
            if let Some(note) = &line.note {
                bits.push(note.clone());
            }
            views.push(RouteGroupView {
                id: format!("group:{}:{}", self.profile, line.line),
                title: line.text.trim().to_owned(),
                icon: if line.kind == ListLineKind::Domain {
                    GroupIcon::Domain
                } else {
                    GroupIcon::Network
                },
                caption: bits.join(" · "),
                rows,
                empty: self.empty(searching),
                line: Some((line.line, line.text.clone())),
            });
        }
        for (key, title, icon, caption) in [
            (
                RouteGroupKey::Upstream,
                "Upstream DNS",
                GroupIcon::Upstream,
                "a /32 on this adapter while connected",
            ),
            (
                RouteGroupKey::Refused,
                "Refused",
                GroupIcon::Refused,
                "answers that would break the LAN or another tunnel",
            ),
        ] {
            let rows = self.rows(&key);
            if rows.is_empty() || !self.connected {
                continue;
            }
            views.push(RouteGroupView {
                id: format!("group:{}:{title}", self.profile),
                title: title.into(),
                icon,
                caption: format!("{} · {caption}", rows.len()),
                rows,
                empty: String::new(),
                line: None,
            });
        }
        views
    }

    fn empty(&self, searching: bool) -> String {
        match (self.connected, searching) {
            (true, true) => "no match".into(),
            (true, false) => "no answers yet".into(),
            (false, _) => format!("Routes appear while {} is connected", self.profile),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::presentation::samples::Samples;

    #[test]
    fn groups_follow_the_list_lines() {
        let detail = Samples::detail("normal", "AcmeCorp");
        let groups = RouteGroups {
            profile: "AcmeCorp",
            lines: &detail.list,
            groups: &detail.routes,
            connected: true,
            search: "",
            time: &Samples::time(),
        };
        let views = groups.views(false);
        assert_eq!(views.len(), 5);
        assert_eq!(views[0].title, "acme.example");
        assert_eq!(
            views[0].caption,
            "line 1 · 4 routes · 1 held by another profile · and its subdomains"
        );
        assert!(matches!(&views[0].rows[4], RouteCellView::Held { by, .. } if by == "Contoso"));
        assert!(
            matches!(&views[0].rows[0], RouteCellView::Learned { last, expires, .. }
            if last == "30 s ago" && expires == "in 23 h 59 min")
        );
        assert_eq!(views[2].caption, "line 3 · network");
        assert!(
            views[3]
                .caption
                .ends_with("overrides Contoso line 2 while both are connected")
        );
        assert_eq!(views[4].title, "Upstream DNS");
    }

    #[test]
    fn search_filters_rows_and_off_profiles_explain() {
        let detail = Samples::detail("normal", "Contoso");
        let groups = RouteGroups {
            profile: "Contoso",
            lines: &detail.list,
            groups: &detail.routes,
            connected: true,
            search: "jira",
            time: &Samples::time(),
        };
        let views = groups.views(true);
        assert_eq!(views.len(), 1);
        assert_eq!(views[0].rows.len(), 1);
        let off = RouteGroups {
            connected: false,
            search: "",
            groups: &[],
            ..groups
        };
        let views = off.views(false);
        assert_eq!(views[0].empty, "Routes appear while Contoso is connected");
        assert_eq!(
            views[0].caption,
            "line 2 · and its subdomains · AcmeCorp line 4 (partners.contoso.example) overrides it while both are connected"
        );
    }
}
