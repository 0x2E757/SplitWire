use super::profile_wire::ProfileWire;
use super::{
    Alert, Attempt, ErrorReply, ListsView, LogsPage, ProfileDetail, ProfileRoutes, ProfileSummary,
    ProfileTraffic, Reader, SettingsSaved, SettingsView, StatusReport, WireError, Writer,
};
use crate::profile::ImportPreview;

/// Every answer of the service. Tags start at 128, so a response is never read as a
/// request. No response carries key material: profiles appear as their facts, public keys and
/// previews only.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Response {
    /// Any request may get it.
    Error(ErrorReply),
    Done,
    Status(Box<StatusReport>),
    ProfileStatus(Box<ProfileDetail>),
    /// Every profile's grouped routes, in order.
    Routes(Vec<ProfileRoutes>),
    Traffic(Vec<ProfileTraffic>),
    Logs(LogsPage),
    /// Today's cleared and dismissed alerts.
    AlertHistory(Vec<Alert>),
    /// `GetAllLists`, `EditList`, `SaveList`, `SaveAllLists`.
    Lists(Box<ListsView>),
    Settings(Box<SettingsView>),
    SettingsSaved(Box<SettingsSaved>),
    CacheReset {
        entries: u64,
        routes: u64,
    },
    ConnectFailed {
        name: String,
        attempt: Attempt,
    },
    ImportPreview(Box<ImportPreview>),
    /// `Done{profile summary}` of an `Import`.
    Imported(Box<ProfileSummary>),
}

impl Response {
    pub fn encode(&self) -> Result<Vec<u8>, WireError> {
        let mut w = Writer::frame(self.tag());
        self.write_fields(&mut w);
        w.finish()
    }

    /// A frame body as `Frame::read` returns it.
    pub fn decode(body: &[u8]) -> Result<Self, WireError> {
        let (tag, mut r) = Reader::open(body)?;
        let response = Self::read_fields(tag, &mut r)?;
        r.finish()?;
        Ok(response)
    }

    pub fn tag(&self) -> u8 {
        match self {
            Self::Error(_) => 128,
            Self::Done => 129,
            Self::Status(_) => 130,
            Self::ProfileStatus(_) => 131,
            Self::Routes(_) => 132,
            Self::Traffic(_) => 133,
            Self::Logs(_) => 134,
            Self::AlertHistory(_) => 135,
            Self::Lists(_) => 136,
            Self::Settings(_) => 137,
            Self::SettingsSaved(_) => 138,
            Self::CacheReset { .. } => 139,
            Self::ConnectFailed { .. } => 140,
            Self::ImportPreview(_) => 141,
            Self::Imported(_) => 142,
        }
    }

    fn write_fields(&self, w: &mut Writer) {
        match self {
            Self::Error(error) => error.write(w),
            Self::Done => {}
            Self::Status(status) => status.write(w),
            Self::ProfileStatus(detail) => detail.write(w),
            Self::Routes(routes) => w.list(routes, ProfileRoutes::write),
            Self::Traffic(traffic) => w.list(traffic, ProfileTraffic::write),
            Self::Logs(page) => page.write(w),
            Self::AlertHistory(alerts) => w.list(alerts, Alert::write),
            Self::Lists(lists) => lists.write(w),
            Self::Settings(settings) => settings.write(w),
            Self::SettingsSaved(saved) => saved.write(w),
            Self::CacheReset { entries, routes } => {
                w.u64(*entries);
                w.u64(*routes);
            }
            Self::ConnectFailed { name, attempt } => {
                w.str(name);
                attempt.write(w);
            }
            Self::ImportPreview(preview) => ProfileWire::preview(preview, w),
            Self::Imported(summary) => summary.write(w),
        }
    }

    fn read_fields(tag: u8, r: &mut Reader) -> Result<Self, WireError> {
        Ok(match tag {
            128 => Self::Error(ErrorReply::read(r)?),
            129 => Self::Done,
            130 => Self::Status(Box::new(StatusReport::read(r)?)),
            131 => Self::ProfileStatus(Box::new(ProfileDetail::read(r)?)),
            132 => Self::Routes(r.list(ProfileRoutes::read)?),
            133 => Self::Traffic(r.list(ProfileTraffic::read)?),
            134 => Self::Logs(LogsPage::read(r)?),
            135 => Self::AlertHistory(r.list(Alert::read)?),
            136 => Self::Lists(Box::new(ListsView::read(r)?)),
            137 => Self::Settings(Box::new(SettingsView::read(r)?)),
            138 => Self::SettingsSaved(Box::new(SettingsSaved::read(r)?)),
            139 => Self::CacheReset {
                entries: r.u64()?,
                routes: r.u64()?,
            },
            140 => Self::ConnectFailed {
                name: r.string()?,
                attempt: Attempt::read(r)?,
            },
            141 => Self::ImportPreview(Box::new(ProfileWire::read_preview(r)?)),
            142 => Self::Imported(Box::new(ProfileSummary::read(r)?)),
            _ => return Err(WireError::UnknownTag(tag)),
        })
    }
}
