use super::{TrayAction, TrayEntry, TrayItem, TrayLead};
use crate::ipc::{ProfileState, ProfileSummary, StatusReport, TrayState};
use crate::presentation::{AlertList, Command, Format, Page, StateText};

/// The right-click menu: a header with the aggregate state, an item per
/// profile that switches it on or off, the alerts, Open SplitWire and Exit.
pub struct TrayMenu;

impl TrayMenu {
    pub fn of(status: Option<&StatusReport>) -> Vec<TrayItem> {
        let open = TrayItem::Entry(TrayEntry {
            default: true,
            ..TrayEntry::action("Open SplitWire", TrayAction::Open(None))
        });
        let exit = TrayItem::Entry(TrayEntry::action("Exit", TrayAction::Exit));
        let Some(status) = status else {
            return vec![
                Self::header("Service not running", TrayState::Service),
                TrayItem::Separator,
                open,
                exit,
            ];
        };
        let profiles = &status.profiles;
        let connected = profiles
            .iter()
            .filter(|p| matches!(p.state, ProfileState::Connected(_)))
            .count();
        let title = if profiles.is_empty() {
            "No profile yet".to_owned()
        } else {
            format!("{connected} of {} connected", profiles.len())
        };
        let mut items = vec![Self::header(&title, status.tray_state), TrayItem::Separator];
        let global_invalid = !status.problems.is_empty();
        items.extend(profiles.iter().map(|p| Self::profile(p, global_invalid)));
        if profiles.is_empty() {
            items.push(TrayItem::Entry(TrayEntry::action(
                "Add profile…",
                TrayAction::Open(Some(Command::Go(Page::Add))),
            )));
        }
        items.push(TrayItem::Separator);
        items.push(Self::alerts(status));
        items.extend([TrayItem::Separator, open, exit]);
        items
    }

    fn header(label: &str, state: TrayState) -> TrayItem {
        TrayItem::Entry(TrayEntry::header(label, TrayLead::State(state)))
    }

    /// What the tooltip says after a profile's name: `2 h 14 min`, `can’t connect`.
    pub fn state_key(profile: &ProfileSummary) -> String {
        match profile.state {
            ProfileState::Connected(_) => profile
                .uptime_secs
                .map_or_else(|| "connected".into(), Format::span),
            ProfileState::Connecting(_) => "connecting".into(),
            ProfileState::Disconnecting => "disconnecting".into(),
            ProfileState::Blocked => "can’t connect".into(),
            ProfileState::Failed => "connect failed".into(),
            ProfileState::Off | ProfileState::HeldOff => "off".into(),
        }
    }

    /// One item per profile: a click connects it when off and disconnects it when on. A profile
    /// Connect would refuse is disabled, with the reason in its state text.
    fn profile(profile: &ProfileSummary, global_invalid: bool) -> TrayItem {
        let state = &profile.state;
        let on = StateText::active(state);
        let (key, enabled) = match state {
            _ if on => (StateText::short(state).to_owned(), true),
            ProfileState::Disconnecting => (StateText::short(state).to_owned(), false),
            ProfileState::Blocked => ("Can’t connect · profile.conf is invalid".to_owned(), false),
            _ if global_invalid => ("Off · splitwire.conf is invalid".to_owned(), false),
            _ => (
                StateText::short(state).to_owned(),
                StateText::can_connect(state, global_invalid),
            ),
        };
        TrayItem::Entry(
            TrayEntry::action(
                profile.name.clone(),
                TrayAction::Session(Command::SetConnected(profile.name.clone(), !on)),
            )
            .lead(TrayLead::Dot(StateText::dot(state)))
            .key(key)
            .enabled(enabled),
        )
    }

    /// "Alerts · N" with one item per active alert (each opens where it is fixed) and Dismiss all,
    /// or "No alerts".
    fn alerts(status: &StatusReport) -> TrayItem {
        let active = AlertList::new(&status.alerts).active(None);
        if active.is_empty() {
            return TrayItem::Entry(TrayEntry::header("No alerts", TrayLead::None));
        }
        let mut items: Vec<TrayItem> = active
            .iter()
            .map(|alert| {
                TrayItem::Entry(
                    TrayEntry::action(
                        alert.title.clone(),
                        TrayAction::Open(Some(Command::FollowAlert(alert.id.clone()))),
                    )
                    .key(alert.profile.as_deref().unwrap_or("Global"))
                    .lead(TrayLead::Severity(alert.severity)),
                )
            })
            .collect();
        if active.iter().any(|a| !a.blocking) {
            items.push(TrayItem::Separator);
            items.push(TrayItem::Entry(TrayEntry::action(
                "Dismiss all",
                TrayAction::Session(Command::DismissAll),
            )));
        }
        let entry =
            TrayEntry::header(format!("Alerts · {}", active.len()), TrayLead::None).enabled(true);
        TrayItem::Submenu(entry, items)
    }
}
