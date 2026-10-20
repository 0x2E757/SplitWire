use std::net::Ipv4Addr;
use std::time::SystemTime;

use super::{Reader, WireError, Writer};
use crate::net::Ipv4Net;

/// One route of a profile, as the Routes page and the profile page list it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RouteRow {
    /// A /32 learned from a DNS answer for `name`.
    Learned {
        ip: Ipv4Addr,
        name: String,
        last_seen: SystemTime,
        expires: SystemTime,
    },
    /// A listed address or network.
    Static { net: Ipv4Net },
    /// A /32 to one of the profile's upstream DNS servers.
    Upstream { ip: Ipv4Addr },
    /// An answer for `name` that profile `by` holds, learned there for `by_name`.
    Held {
        ip: Ipv4Addr,
        name: String,
        by: String,
        by_name: String,
    },
    /// A route the guard refused.
    Refused {
        net: Ipv4Net,
        name: Option<String>,
        reason: String,
    },
}

impl RouteRow {
    fn write(&self, w: &mut Writer) {
        match self {
            Self::Learned {
                ip,
                name,
                last_seen,
                expires,
            } => {
                w.u64(0);
                w.ipv4(*ip);
                w.str(name);
                w.time(*last_seen);
                w.time(*expires);
            }
            Self::Static { net } => {
                w.u64(1);
                w.net(*net);
            }
            Self::Upstream { ip } => {
                w.u64(2);
                w.ipv4(*ip);
            }
            Self::Held {
                ip,
                name,
                by,
                by_name,
            } => {
                w.u64(3);
                w.ipv4(*ip);
                w.str(name);
                w.str(by);
                w.str(by_name);
            }
            Self::Refused { net, name, reason } => {
                w.u64(4);
                w.net(*net);
                w.opt_str(name.as_ref());
                w.str(reason);
            }
        }
    }

    fn read(r: &mut Reader) -> Result<Self, WireError> {
        Ok(match r.u64()? {
            0 => Self::Learned {
                ip: r.ipv4()?,
                name: r.string()?,
                last_seen: r.time()?,
                expires: r.time()?,
            },
            1 => Self::Static { net: r.net()? },
            2 => Self::Upstream { ip: r.ipv4()? },
            3 => Self::Held {
                ip: r.ipv4()?,
                name: r.string()?,
                by: r.string()?,
                by_name: r.string()?,
            },
            4 => Self::Refused {
                net: r.net()?,
                name: r.opt_string()?,
                reason: r.string()?,
            },
            _ => return Err(WireError::Invalid("an unknown route kind")),
        })
    }
}

/// What a group of routes hangs under: a list line, or the Upstream DNS and Refused groups at the
/// end.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RouteGroupKey {
    Line { line: usize, text: String },
    Upstream,
    Refused,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RouteGroup {
    pub key: RouteGroupKey,
    pub rows: Vec<RouteRow>,
}

impl RouteGroup {
    pub fn write(&self, w: &mut Writer) {
        match &self.key {
            RouteGroupKey::Line { line, text } => {
                w.u64(0);
                w.usize(*line);
                w.str(text);
            }
            RouteGroupKey::Upstream => w.u64(1),
            RouteGroupKey::Refused => w.u64(2),
        }
        w.list(&self.rows, RouteRow::write);
    }

    pub fn read(r: &mut Reader) -> Result<Self, WireError> {
        let key = match r.u64()? {
            0 => RouteGroupKey::Line {
                line: r.usize()?,
                text: r.string()?,
            },
            1 => RouteGroupKey::Upstream,
            2 => RouteGroupKey::Refused,
            _ => return Err(WireError::Invalid("an unknown route group")),
        };
        Ok(Self {
            key,
            rows: r.list(RouteRow::read)?,
        })
    }
}

/// One profile's grouped routes: the answer to `Routes` holds one per profile, in order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProfileRoutes {
    pub name: String,
    pub groups: Vec<RouteGroup>,
}

impl ProfileRoutes {
    pub fn write(&self, w: &mut Writer) {
        w.str(&self.name);
        w.list(&self.groups, RouteGroup::write);
    }

    pub fn read(r: &mut Reader) -> Result<Self, WireError> {
        Ok(Self {
            name: r.string()?,
            groups: r.list(RouteGroup::read)?,
        })
    }
}
