//! The files the window reads and writes itself: its own settings, the `.conf` files picked for an
//! import, and the service's log file (interactive users may read `logs\`).

use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::ipc::ConfText;
use crate::presentation::{PickedFile, WindowSettings};

/// `%LOCALAPPDATA%\SplitWire\splitwire.window`.
pub struct WindowStore {
    path: PathBuf,
}

impl WindowStore {
    pub fn new(folder: &Path) -> Self {
        Self {
            path: folder.join(WindowSettings::FILE),
        }
    }

    pub fn load(&self) -> WindowSettings {
        std::fs::read_to_string(&self.path)
            .map(|text| WindowSettings::parse(&text))
            .unwrap_or_default()
    }

    pub fn save(&self, settings: &WindowSettings) -> std::io::Result<()> {
        if let Some(folder) = self.path.parent() {
            std::fs::create_dir_all(folder)?;
        }
        crate::platform::atomic_file::AtomicFile::write(&self.path, settings.text().as_bytes())
    }
}

/// Reads picked or dropped `.conf` files into redacted texts.
pub struct ConfFiles;

impl ConfFiles {
    /// A `.conf` is a few hundred bytes; anything this large is not one.
    pub const MAX_BYTES: u64 = 64 * 1024;

    pub fn read(paths: &[PathBuf]) -> Vec<PickedFile> {
        paths.iter().map(|path| Self::one(path)).collect()
    }

    fn one(path: &Path) -> PickedFile {
        let file_name = path.file_name().map_or_else(
            || path.display().to_string(),
            |n| n.to_string_lossy().into_owned(),
        );
        let text = match std::fs::metadata(path) {
            Err(error) => Err(format!("cannot be read: {error}")),
            Ok(meta) if meta.is_dir() => Err("is a folder, not a .conf file".into()),
            Ok(meta) if meta.len() > Self::MAX_BYTES => {
                Err("larger than 64 KB: not a WireGuard .conf".into())
            }
            Ok(_) => match std::fs::read(path) {
                Err(error) => Err(format!("cannot be read: {error}")),
                Ok(bytes) => String::from_utf8(bytes)
                    .map(|text| {
                        Arc::new(ConfText::new(
                            text.trim_start_matches('\u{feff}').to_owned(),
                        ))
                    })
                    .map_err(|_| "is not UTF-8 text".into()),
            },
        };
        PickedFile { file_name, text }
    }
}

/// `logs\splitwire.log` and its rollover `.1`, older first.
pub struct LogFiles;

impl LogFiles {
    pub fn read(folder: &Path) -> String {
        ["splitwire.log.1", "splitwire.log"]
            .iter()
            .filter_map(|name| std::fs::read(folder.join(name)).ok())
            .map(|bytes| String::from_utf8_lossy(&bytes).into_owned())
            .collect::<Vec<_>>()
            .join("")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let folder =
            std::env::temp_dir().join(format!("splitwire-window-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&folder);
        std::fs::create_dir_all(&folder).unwrap();
        folder
    }

    #[test]
    fn window_settings_survive_a_round_trip() {
        let folder = scratch("store");
        let store = WindowStore::new(&folder.join("nested"));
        assert_eq!(store.load(), WindowSettings::default());
        let settings = WindowSettings {
            open_window_at_sign_in: true,
            ..WindowSettings::default()
        };
        store.save(&settings).unwrap();
        assert_eq!(store.load(), settings);
        let _ = std::fs::remove_dir_all(folder);
    }

    #[test]
    fn conf_files_are_read_with_their_problems() {
        let folder = scratch("conf");
        std::fs::write(folder.join("a.conf"), "\u{feff}[Interface]\n").unwrap();
        std::fs::write(folder.join("b.conf"), [0xff, 0xfe, 0x00]).unwrap();
        let picked = ConfFiles::read(&[
            folder.join("a.conf"),
            folder.join("b.conf"),
            folder.join("c.conf"),
        ]);
        assert_eq!(picked[0].file_name, "a.conf");
        assert_eq!(picked[0].text.as_ref().unwrap().expose(), "[Interface]\n");
        assert_eq!(picked[1].text.as_ref().unwrap_err(), "is not UTF-8 text");
        assert!(
            picked[2]
                .text
                .as_ref()
                .unwrap_err()
                .starts_with("cannot be read")
        );
        std::fs::write(folder.join("splitwire.log.1"), "old\n").unwrap();
        std::fs::write(folder.join("splitwire.log"), "new\n").unwrap();
        assert_eq!(LogFiles::read(&folder), "old\nnew\n");
        let _ = std::fs::remove_dir_all(folder);
    }
}
