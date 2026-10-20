use super::profile_wire::ProfileWire;
use super::settings_wire::SettingsWire;
use super::{ConfText, LineRef, Reader, WireError, Writer};
use crate::config::SettingChange;
use crate::profile::Pin;

/// Which profiles a `Disconnect` is for.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProfileTarget {
    One(String),
    All,
}

/// Every request a client sends. Names are sent as typed: the service validates them
/// and answers `unknown_profile` or `refused`.
#[derive(Debug, PartialEq, Eq)]
pub enum Request {
    Status,
    ProfileStatus {
        name: String,
    },
    Routes,
    Traffic {
        after_seq: u64,
    },
    Logs {
        after: u64,
    },
    AlertHistory,
    DismissAlert {
        id: String,
    },
    DismissAll,
    /// One alert, or all of them.
    RestoreAlerts {
        id: Option<String>,
    },
    GetAllLists,
    EditList {
        base_rev: u64,
        name: String,
        add: Option<String>,
        remove: Option<LineRef>,
    },
    SaveList {
        base_rev: u64,
        name: String,
        text: String,
    },
    SaveAllLists {
        base_rev: u64,
        text: String,
    },
    GetSettings,
    SaveSettings {
        base_rev: u64,
        changes: Vec<SettingChange>,
    },
    /// One profile, or all of them.
    ResetCache {
        name: Option<String>,
    },
    Connect {
        name: String,
    },
    Disconnect {
        target: ProfileTarget,
    },
    PreviewImport {
        name: String,
        file_name: String,
        conf_text: ConfText,
    },
    Import {
        name: String,
        conf_text: ConfText,
        colour: Pin,
        replace: bool,
    },
    RemoveProfile {
        name: String,
    },
    SetProfileColor {
        name: String,
        colour: Pin,
    },
    MoveProfile {
        name: String,
        index: usize,
    },
}

impl Request {
    /// The whole frame. A frame of `PreviewImport` or `Import` holds a `.conf`: it is built
    /// without reallocating, and the pipe zeroes it after sending.
    pub fn encode(&self) -> Result<Vec<u8>, WireError> {
        let mut w = Writer::frame_with_capacity(self.tag(), self.size_hint());
        self.write_fields(&mut w);
        w.finish()
    }

    /// A frame body as `Frame::read` returns it.
    pub fn decode(body: &[u8]) -> Result<Self, WireError> {
        let (tag, mut r) = Reader::open(body)?;
        let request = Self::read_fields(tag, &mut r)?;
        r.finish()?;
        Ok(request)
    }

    pub fn tag(&self) -> u8 {
        match self {
            Self::Status => 1,
            Self::ProfileStatus { .. } => 2,
            Self::Routes => 3,
            Self::Traffic { .. } => 4,
            Self::Logs { .. } => 5,
            Self::AlertHistory => 6,
            Self::DismissAlert { .. } => 7,
            Self::DismissAll => 8,
            Self::RestoreAlerts { .. } => 9,
            Self::GetAllLists => 10,
            Self::EditList { .. } => 11,
            Self::SaveList { .. } => 12,
            Self::SaveAllLists { .. } => 13,
            Self::GetSettings => 14,
            Self::SaveSettings { .. } => 15,
            Self::ResetCache { .. } => 16,
            Self::Connect { .. } => 17,
            Self::Disconnect { .. } => 18,
            Self::PreviewImport { .. } => 20,
            Self::Import { .. } => 21,
            Self::RemoveProfile { .. } => 22,
            Self::SetProfileColor { .. } => 23,
            Self::MoveProfile { .. } => 24,
        }
    }

    /// More than the fields of a request that carries a `.conf` take: the text, its length and
    /// room for the names.
    fn size_hint(&self) -> usize {
        match self {
            Self::PreviewImport {
                name,
                file_name,
                conf_text,
            } => conf_text.expose().len() + name.len() + file_name.len() + 64,
            Self::Import {
                name, conf_text, ..
            } => conf_text.expose().len() + name.len() + 64,
            _ => 0,
        }
    }

    fn write_fields(&self, w: &mut Writer) {
        match self {
            Self::Status
            | Self::Routes
            | Self::AlertHistory
            | Self::DismissAll
            | Self::GetAllLists
            | Self::GetSettings => {}
            Self::ProfileStatus { name }
            | Self::Connect { name }
            | Self::RemoveProfile { name } => w.str(name),
            Self::Traffic { after_seq: after } | Self::Logs { after } => w.u64(*after),
            Self::DismissAlert { id } => w.str(id),
            Self::RestoreAlerts { id } => w.opt_str(id.as_ref()),
            Self::EditList {
                base_rev,
                name,
                add,
                remove,
            } => {
                w.u64(*base_rev);
                w.str(name);
                w.opt_str(add.as_ref());
                w.option(remove.as_ref(), LineRef::write);
            }
            Self::SaveList {
                base_rev,
                name,
                text,
            } => {
                w.u64(*base_rev);
                w.str(name);
                w.str(text);
            }
            Self::SaveAllLists { base_rev, text } => {
                w.u64(*base_rev);
                w.str(text);
            }
            Self::SaveSettings { base_rev, changes } => {
                w.u64(*base_rev);
                w.list(changes, SettingsWire::change);
            }
            Self::ResetCache { name } => w.opt_str(name.as_ref()),
            Self::Disconnect { target } => w.option(
                match target {
                    ProfileTarget::One(name) => Some(name),
                    ProfileTarget::All => None,
                },
                |name, w| w.str(name),
            ),
            Self::PreviewImport {
                name,
                file_name,
                conf_text,
            } => {
                w.str(name);
                w.str(file_name);
                conf_text.write(w);
            }
            Self::Import {
                name,
                conf_text,
                colour,
                replace,
            } => {
                w.str(name);
                conf_text.write(w);
                ProfileWire::pin(colour, w);
                w.bool(*replace);
            }
            Self::SetProfileColor { name, colour } => {
                w.str(name);
                ProfileWire::pin(colour, w);
            }
            Self::MoveProfile { name, index } => {
                w.str(name);
                w.usize(*index);
            }
        }
    }

    fn read_fields(tag: u8, r: &mut Reader) -> Result<Self, WireError> {
        Ok(match tag {
            1 => Self::Status,
            2 => Self::ProfileStatus { name: r.string()? },
            3 => Self::Routes,
            4 => Self::Traffic {
                after_seq: r.u64()?,
            },
            5 => Self::Logs { after: r.u64()? },
            6 => Self::AlertHistory,
            7 => Self::DismissAlert { id: r.string()? },
            8 => Self::DismissAll,
            9 => Self::RestoreAlerts {
                id: r.opt_string()?,
            },
            10 => Self::GetAllLists,
            11 => Self::EditList {
                base_rev: r.u64()?,
                name: r.string()?,
                add: r.opt_string()?,
                remove: r.option(LineRef::read)?,
            },
            12 => Self::SaveList {
                base_rev: r.u64()?,
                name: r.string()?,
                text: r.string()?,
            },
            13 => Self::SaveAllLists {
                base_rev: r.u64()?,
                text: r.string()?,
            },
            14 => Self::GetSettings,
            15 => Self::SaveSettings {
                base_rev: r.u64()?,
                changes: r.list(SettingsWire::read_change)?,
            },
            16 => Self::ResetCache {
                name: r.opt_string()?,
            },
            17 => Self::Connect { name: r.string()? },
            18 => Self::Disconnect {
                target: r
                    .opt_string()?
                    .map_or(ProfileTarget::All, ProfileTarget::One),
            },
            20 => Self::PreviewImport {
                name: r.string()?,
                file_name: r.string()?,
                conf_text: ConfText::read(r)?,
            },
            21 => Self::Import {
                name: r.string()?,
                conf_text: ConfText::read(r)?,
                colour: ProfileWire::read_pin(r)?,
                replace: r.bool()?,
            },
            22 => Self::RemoveProfile { name: r.string()? },
            23 => Self::SetProfileColor {
                name: r.string()?,
                colour: ProfileWire::read_pin(r)?,
            },
            24 => Self::MoveProfile {
                name: r.string()?,
                index: r.usize()?,
            },
            _ => return Err(WireError::UnknownTag(tag)),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ipc::Frame;
    use crate::profile::fixtures::full_conf;

    #[test]
    fn a_frame_carrying_a_conf_never_grows() {
        let requests = [
            Request::PreviewImport {
                name: "Partner".into(),
                file_name: "Partner.conf".into(),
                conf_text: ConfText::new(full_conf().repeat(50)),
            },
            Request::Import {
                name: "Partner".into(),
                conf_text: ConfText::new(full_conf()),
                colour: Pin::new(3).unwrap(),
                replace: false,
            },
        ];
        for request in requests {
            let frame = request.encode().unwrap();
            // The buffer is the one allocated up front: no copy was left behind by a regrowth.
            assert_eq!(
                frame.capacity(),
                4 + Frame::HEADER_LEN + request.size_hint()
            );
            assert!(frame.len() <= frame.capacity());
        }
    }
}
