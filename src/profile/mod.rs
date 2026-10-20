//! Profiles: names, the WireGuard `.conf` parser with its per-line outcomes, the profile and its
//! keys, the plaintext codec of the blob, the import preview, the `SecretStore` boundary, and
//! the state a profile is in.

mod adapter_guid;
mod base64;
mod conf_line;
mod endpoint;
#[cfg(test)]
pub(crate) mod fixtures;
mod import_checks;
mod import_preview;
mod pin;
#[allow(clippy::module_inception)]
mod profile;
mod profile_codec;
mod profile_fields;
mod profile_name;
mod profile_state;
mod secret_bytes;
mod secret_key;
mod secret_store;
mod tunnel_address;
mod wg_conf_parser;

pub use adapter_guid::AdapterGuid;
pub use base64::Base64;
pub use conf_line::{ConfLine, LineOutcome};
pub use endpoint::{Endpoint, EndpointHost};
pub use import_checks::{CheckLevel, ImportCheck, ImportChecks, OtherProfile};
pub use import_preview::{ImportContext, ImportPreview, ImportRequest, KnownProfile};
pub use pin::Pin;
pub use profile::{Profile, ProfileFacts};
pub use profile_codec::ProfileCodec;
pub use profile_fields::{ChangeRow, ProfileFields};
pub use profile_name::ProfileName;
pub use profile_state::ProfileState;
pub use secret_bytes::{SecretBytes, Zeroize};
pub use secret_key::SecretKey;
pub use secret_store::SecretStore;
pub use tunnel_address::TunnelAddress;
pub use wg_conf_parser::{ParsedConf, WgConf, WgConfParser};
