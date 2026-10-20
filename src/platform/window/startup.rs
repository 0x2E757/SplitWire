//! "Start SplitWire at sign-in": the HKCU Run value `SplitWire` =
//! `"<exe>" --tray`. Present is on; no elevation is needed.

use std::io;
use std::path::Path;

use crate::platform::registry::{RegistryKey, RegistryRoot};

pub struct StartAtSignIn {
    /// The key under HKCU that holds the value: the Run key, or a scratch key in the tests.
    key: String,
}

impl StartAtSignIn {
    const RUN: &'static str = r"Software\Microsoft\Windows\CurrentVersion\Run";
    const VALUE: &'static str = "SplitWire";
    /// What the value starts the window with: the tray, and the window only when chosen.
    pub const ARGUMENT: &'static str = "--tray";

    /// The signed-in user's Run key.
    pub fn user() -> Self {
        Self::under(Self::RUN)
    }

    pub fn under(key: &str) -> Self {
        Self { key: key.into() }
    }

    /// `"<exe>" --tray`, quoted for a path with spaces.
    pub fn command(exe: &Path) -> String {
        format!("\"{}\" {}", exe.display(), Self::ARGUMENT)
    }

    /// Whether the value is there.
    pub fn is_set(&self) -> bool {
        RegistryKey::open(RegistryRoot::CurrentUser, &self.key, false)
            .ok()
            .flatten()
            .and_then(|key| key.string(Self::VALUE).ok().flatten())
            .is_some()
    }

    /// Writes the value starting `exe`, or deletes it.
    pub fn set(&self, on: bool, exe: &Path) -> io::Result<()> {
        if on {
            RegistryKey::create(RegistryRoot::CurrentUser, &self.key)?
                .set_string(Self::VALUE, &Self::command(exe))
        } else {
            match RegistryKey::open(RegistryRoot::CurrentUser, &self.key, true)? {
                Some(key) => key.delete_value(Self::VALUE),
                None => Ok(()),
            }
        }
    }

    #[cfg(test)]
    fn value(&self) -> Option<String> {
        RegistryKey::open(RegistryRoot::CurrentUser, &self.key, false)
            .unwrap()
            .and_then(|key| key.string(Self::VALUE).unwrap())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::platform::registry::tests::ScratchKey;

    #[test]
    fn the_value_starts_the_tray_and_goes_away() {
        let scratch = ScratchKey::new("run");
        let run = StartAtSignIn::under(&format!(r"{}\Run", scratch.path));
        let exe = Path::new(r"C:\Program Files\SplitWire\SplitWire.Window.exe");
        assert!(!run.is_set());
        run.set(false, exe).unwrap();
        run.set(true, exe).unwrap();
        assert!(run.is_set());
        assert_eq!(
            run.value().as_deref(),
            Some(r#""C:\Program Files\SplitWire\SplitWire.Window.exe" --tray"#)
        );
        run.set(false, exe).unwrap();
        assert!(!run.is_set());
        assert_eq!(StartAtSignIn::user().key, StartAtSignIn::RUN);
    }
}
