//! Logs (newest on top): the service's ring buffer (Live) or `splitwire.log` (warnings and
//! errors, kept across restarts), filtered by level, profile and text. Paused, new lines collect
//! above the view as "+N new" with Back to latest.

use crate::ipc::{LogLine, StatusReport};
use crate::logs::Level;
use crate::presentation::{Format, LevelFilter, LocalTime, LogSource, ProfileFilter};
use crate::profile::Pin;

/// The page's own state.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LogsUi {
    pub source: LogSource,
    pub level: LevelFilter,
    pub profile: ProfileFilter,
    /// Follow off: the newest line shown when it was paused.
    pub paused_at: Option<u64>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LogRowView {
    pub day: String,
    pub time: String,
    pub level: Level,
    pub profile: Option<(String, Option<Pin>)>,
    pub source: String,
    pub message: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LogsPageView {
    pub source: LogSource,
    pub live_label: String,
    pub file_label: String,
    /// `(filter, label)` of the level Segmented.
    pub levels: Vec<(LevelFilter, String)>,
    pub level: LevelFilter,
    pub profile_label: String,
    pub profile_colour: Option<Pin>,
    pub profile_filtered: bool,
    /// `(filter, label, colour)` of the profile menu.
    pub profiles: Vec<(ProfileFilter, String, Option<Pin>)>,
    pub follow: bool,
    /// `+38 new` and when the view was paused.
    pub paused: Option<(String, String)>,
    pub rows: Vec<LogRowView>,
    /// The file view's day separators: `(index of the first row, title)`.
    pub days: Vec<(usize, String)>,
    pub footer: String,
    pub footer_end: String,
    pub copy_text: String,
}

pub struct LogsSource<'a> {
    pub status: Option<&'a StatusReport>,
    pub live: &'a [LogLine],
    pub file: &'a [LogLine],
    pub file_bytes: usize,
    pub filter: &'a str,
    pub ui: &'a LogsUi,
    pub time: &'a LocalTime,
}

impl LogsSource<'_> {
    /// Rows are created for the newest lines only; the copy and the counts cover all.
    pub const MAX_ROWS: usize = 200;

    fn colour(&self, name: &str) -> Option<Pin> {
        self.status?
            .profiles
            .iter()
            .find(|p| p.name == name)
            .map(|p| p.colour)
    }

    fn keep(&self, line: &LogLine, level: bool) -> bool {
        let profile = match &self.ui.profile {
            ProfileFilter::All => true,
            ProfileFilter::Global => line.profile.is_none(),
            ProfileFilter::Profile(name) => line.profile.as_deref() == Some(name),
        };
        let level = !level
            || match self.ui.level {
                LevelFilter::All => true,
                LevelFilter::Info => line.level == Level::Info,
                LevelFilter::Warnings => line.level == Level::Warn,
                LevelFilter::Errors => line.level == Level::Error,
            };
        let query = self.filter.trim().to_lowercase();
        let text = query.is_empty()
            || format!(
                "{} {} {}",
                line.profile.as_deref().unwrap_or_default(),
                line.source,
                line.message
            )
            .to_lowercase()
            .contains(&query);
        profile && level && text
    }

    pub fn view(&self) -> LogsPageView {
        let live = self.ui.source == LogSource::Live;
        let lines = if live { self.live } else { self.file };
        let visible: Vec<&LogLine> = lines
            .iter()
            .filter(|l| self.ui.paused_at.is_none_or(|p| !live || l.seq <= p))
            .collect();
        let base: Vec<&&LogLine> = visible.iter().filter(|l| self.keep(l, false)).collect();
        let count = |level: Level| base.iter().filter(|l| l.level == level).count();
        let levels = if live {
            vec![
                (LevelFilter::All, "All".to_owned()),
                (LevelFilter::Info, "Info".to_owned()),
                (
                    LevelFilter::Warnings,
                    format!("Warnings · {}", count(Level::Warn)),
                ),
                (
                    LevelFilter::Errors,
                    format!("Errors · {}", count(Level::Error)),
                ),
            ]
        } else {
            vec![
                (LevelFilter::All, "Warnings and errors".to_owned()),
                (
                    LevelFilter::Errors,
                    format!("Errors · {}", count(Level::Error)),
                ),
            ]
        };
        let level = if live || self.ui.level == LevelFilter::Errors {
            self.ui.level
        } else {
            LevelFilter::All
        };
        let shown: Vec<&LogLine> = visible
            .iter()
            .rev()
            .filter(|l| self.keep(l, true))
            .copied()
            .collect();
        let rows: Vec<LogRowView> = shown
            .iter()
            .take(Self::MAX_ROWS)
            .map(|l| LogRowView {
                day: self.time.iso_date(l.time),
                time: self.time.clock_millis(l.time),
                level: l.level,
                profile: l.profile.as_ref().map(|p| (p.clone(), self.colour(p))),
                source: l.source.clone(),
                message: l.message.clone(),
            })
            .collect();
        let mut days = Vec::new();
        if !live {
            for (index, row) in rows.iter().enumerate() {
                if index == 0 || rows[index - 1].day != row.day {
                    let at = shown[index].time;
                    days.push((index, self.time.day_title(at)));
                }
            }
        }
        let names: Vec<(String, Option<Pin>)> = self
            .status
            .map(|s| {
                s.profiles
                    .iter()
                    .map(|p| (p.name.clone(), Some(p.colour)))
                    .collect()
            })
            .unwrap_or_default();
        let mut profiles = vec![(ProfileFilter::All, "All profiles".to_owned(), None)];
        profiles.extend(
            names
                .iter()
                .map(|(n, c)| (ProfileFilter::Profile(n.clone()), n.clone(), *c)),
        );
        profiles.push((
            ProfileFilter::Global,
            "Global (service, DNS proxy, NRPT)".to_owned(),
            None,
        ));
        let (profile_label, profile_colour) = match &self.ui.profile {
            ProfileFilter::All => ("All profiles".to_owned(), None),
            ProfileFilter::Global => ("Global".to_owned(), None),
            ProfileFilter::Profile(name) => (name.clone(), self.colour(name)),
        };
        let paused = self.ui.paused_at.filter(|_| live).map(|at| {
            let new = self.live.iter().filter(|l| l.seq > at).count();
            let when = self
                .live
                .iter()
                .find(|l| l.seq == at)
                .map(|l| self.time.clock_seconds(l.time))
                .unwrap_or_default();
            (
                format!("+{new} new"),
                format!("since you paused at {when}; the view holds still"),
            )
        });
        let since = self
            .status
            .map(|s| self.time.clock(s.since))
            .unwrap_or_default();
        let copy_text = shown
            .iter()
            .map(|l| {
                format!(
                    "{} {} {:<5} {} {}: {}",
                    self.time.iso_date(l.time),
                    self.time.clock_millis(l.time),
                    l.level,
                    l.profile.as_deref().unwrap_or("-"),
                    l.source,
                    l.message
                )
            })
            .collect::<Vec<_>>()
            .join("\n");
        LogsPageView {
            source: self.ui.source,
            live_label: format!("Live · {}", Format::count(self.live.len() as u64)),
            file_label: format!("splitwire.log · {}", Format::count(self.file.len() as u64)),
            levels,
            level,
            profile_filtered: self.ui.profile != ProfileFilter::All,
            profile_label,
            profile_colour,
            profiles,
            follow: self.ui.paused_at.is_none(),
            paused,
            footer: if live {
                format!("Newest on top · ring buffer of this service run since {since}")
            } else {
                format!(
                    "Newest on top · warnings and errors only, kept across restarts · {} KB of 1 MB",
                    self.file_bytes.div_ceil(1000)
                )
            },
            footer_end: format!("{} of {} lines shown", rows.len(), lines.len()),
            rows,
            days,
            copy_text,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ipc::{Request, Response};
    use crate::presentation::LogFile;
    use crate::presentation::samples::Samples;
    use crate::presentation::transport::EngineTransport;
    use crate::presentation::transport::demo::{DemoTransport, Scenario};

    fn live() -> Vec<LogLine> {
        match DemoTransport::new(Scenario::Normal).exchange(&Request::Logs { after: 0 }) {
            Ok(Response::Logs(page)) => page.entries,
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn live_is_newest_on_top_with_counts() {
        let status = Samples::status("normal");
        let live = live();
        let ui = LogsUi::default();
        let source = LogsSource {
            status: Some(&status),
            live: &live,
            file: &[],
            file_bytes: 0,
            filter: "",
            ui: &ui,
            time: &Samples::time(),
        };
        let view = source.view();
        assert_eq!(
            view.rows[0].message,
            "health: Connected, handshake 1 min 18 s ago"
        );
        assert_eq!(view.rows[0].time, "11:28:55.391");
        assert_eq!(view.levels[2].1, "Warnings · 4");
        assert_eq!(view.levels[3].1, "Errors · 2");
        assert_eq!(
            view.footer,
            "Newest on top · ring buffer of this service run since 09:12"
        );
        assert!(view.follow && view.paused.is_none());
        let ui = LogsUi {
            profile: ProfileFilter::Profile("HomeLab".into()),
            level: LevelFilter::Errors,
            paused_at: Some(live[40].seq),
            ..LogsUi::default()
        };
        let view = LogsSource { ui: &ui, ..source }.view();
        assert_eq!(view.rows.len(), 1);
        assert_eq!(view.profile_label, "HomeLab");
        assert_eq!(view.paused.unwrap().0, format!("+{} new", live.len() - 41));
        let filtered = LogsSource {
            filter: "re-homed",
            ..source
        }
        .view();
        assert_eq!(filtered.rows.len(), 1);
    }

    #[test]
    fn the_file_view_groups_days() {
        let file = LogFile::parse(&DemoTransport::log_file(Scenario::Normal));
        let ui = LogsUi {
            source: LogSource::File,
            ..LogsUi::default()
        };
        let view = LogsSource {
            status: None,
            live: &[],
            file: &file,
            file_bytes: 2100,
            filter: "",
            ui: &ui,
            time: &Samples::time(),
        }
        .view();
        assert_eq!(view.levels[0].1, "Warnings and errors");
        assert_eq!(view.days[0], (0, "Today 19.10.2026".to_owned()));
        assert_eq!(view.days.len(), 3);
        assert!(view.footer.ends_with("3 KB of 1 MB"));
    }
}
