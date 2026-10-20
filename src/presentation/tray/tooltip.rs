use super::TrayMenu;
use crate::ipc::{ProfileState, StatusReport};
use crate::presentation::AlertList;

/// The icon's tooltip: "SplitWire · N of M connected", a line per failed, blocked
/// or connecting profile, and "N alerts". It must fit `szTip`; profiles that do not fit
/// become "+N more".
pub struct TrayTooltip;

impl TrayTooltip {
    /// `szTip` holds 128 UTF-16 units with the terminating NUL.
    pub const MAX: usize = 127;

    pub fn of(status: Option<&StatusReport>) -> String {
        let Some(status) = status else {
            return "SplitWire · service not running".into();
        };
        if status.profiles.is_empty() {
            return "SplitWire · no profile yet".into();
        }
        let connected = status
            .profiles
            .iter()
            .filter(|p| matches!(p.state, ProfileState::Connected(_)))
            .count();
        let head = format!(
            "SplitWire · {connected} of {} connected",
            status.profiles.len()
        );
        let lines: Vec<String> = status
            .profiles
            .iter()
            .filter(|p| {
                matches!(
                    p.state,
                    ProfileState::Blocked | ProfileState::Failed | ProfileState::Connecting(_)
                )
            })
            .map(|p| format!("{} {}", p.name, TrayMenu::state_key(p)))
            .collect();
        let alerts = AlertList::new(&status.alerts).active(None).len();
        let tail = (alerts > 0)
            .then(|| format!("{alerts} {}", if alerts == 1 { "alert" } else { "alerts" }));
        Self::fit(&head, &lines, tail.as_deref())
    }

    /// The head, as many profile lines as fit (the rest as "+N more"), and the tail.
    fn fit(head: &str, lines: &[String], tail: Option<&str>) -> String {
        let join = |shown: &[String], more: usize| {
            let mut all = vec![head.to_owned()];
            all.extend(shown.iter().cloned());
            if more > 0 {
                all.push(format!("+{more} more"));
            }
            all.extend(tail.map(str::to_owned));
            all.join("\n")
        };
        for shown in (0..=lines.len()).rev() {
            let text = join(&lines[..shown], lines.len() - shown);
            if text.encode_utf16().count() <= Self::MAX {
                return text;
            }
        }
        head.chars()
            .scan(0, |units, c| {
                *units += c.len_utf16();
                (*units <= Self::MAX).then_some(c)
            })
            .collect()
    }
}
