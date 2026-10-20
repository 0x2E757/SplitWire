use std::fmt;

use super::{Reader, WireError, Writer};
use crate::profile::SecretBytes;

/// The text of a `.conf` on its way from a client to the service (`PreviewImport`, `Import`): it
/// holds a private key, so it is zeroed on drop and redacted in `Debug`. No response carries one.
pub struct ConfText(SecretBytes);

impl ConfText {
    pub fn new(text: String) -> Self {
        Self(SecretBytes::from_vec(text.into_bytes()))
    }

    pub fn expose(&self) -> &str {
        std::str::from_utf8(self.0.expose()).expect("built from UTF-8 only")
    }

    pub fn write(&self, w: &mut Writer) {
        w.str(self.expose());
    }

    pub fn read(r: &mut Reader) -> Result<Self, WireError> {
        let text = r.str()?;
        let mut bytes = SecretBytes::with_capacity(text.len());
        bytes.extend(text.as_bytes());
        Ok(Self(bytes))
    }
}

impl fmt::Debug for ConfText {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ConfText({} bytes, redacted)", self.0.len())
    }
}

impl PartialEq for ConfText {
    fn eq(&self, other: &Self) -> bool {
        self.0.expose() == other.0.expose()
    }
}

impl Eq for ConfText {}
