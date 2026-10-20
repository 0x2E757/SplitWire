use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

/// `logs\splitwire.log`: the warnings and errors, kept across restarts. Past `max_bytes` the file
/// becomes `splitwire.log.1` (replacing the older one) and a new file starts, so the two together
/// stay under twice the limit. Interactive users may read it; it never holds secrets.
pub struct LogFile {
    path: PathBuf,
    max_bytes: u64,
    file: Option<File>,
    size: u64,
}

impl LogFile {
    pub const NAME: &'static str = "splitwire.log";
    /// The last fatal start error, written by itself.
    pub const ERROR_NAME: &'static str = "splitwire.error.log";
    pub const MAX_BYTES: u64 = 1 << 20;

    /// `splitwire.log` in `logs`.
    pub fn in_folder(logs: &Path) -> Self {
        Self::new(logs.join(Self::NAME), Self::MAX_BYTES)
    }

    pub fn new(path: PathBuf, max_bytes: u64) -> Self {
        Self {
            path,
            max_bytes,
            file: None,
            size: 0,
        }
    }

    /// The file after one rollover.
    pub fn rolled_path(&self) -> PathBuf {
        let mut name = self.path.as_os_str().to_owned();
        name.push(".1");
        PathBuf::from(name)
    }

    /// Appends `line` and a line break, rolling the file over first when it would pass the limit.
    pub fn append(&mut self, line: &str) -> io::Result<()> {
        let bytes = line.len() as u64 + 2;
        if self.file.is_none() {
            self.open()?;
        }
        if self.size > 0 && self.size + bytes > self.max_bytes {
            self.file = None;
            fs::rename(&self.path, self.rolled_path())?;
            self.open()?;
        }
        let file = self.file.as_mut().expect("opened above");
        file.write_all(format!("{line}\r\n").as_bytes())?;
        self.size += bytes;
        Ok(())
    }

    /// Replaces `logs\splitwire.error.log` with `text`: why the last start failed.
    pub fn write_start_error(logs: &Path, text: &str) -> io::Result<()> {
        fs::create_dir_all(logs)?;
        fs::write(logs.join(Self::ERROR_NAME), format!("{text}\r\n"))
    }

    fn open(&mut self) -> io::Result<()> {
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)?;
        self.size = file.metadata()?.len();
        self.file = Some(file);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rolls_over_once_past_the_limit() {
        let folder = std::env::temp_dir().join(format!("splitwire-logfile-{}", std::process::id()));
        let _ = fs::remove_dir_all(&folder);
        fs::create_dir_all(&folder).unwrap();
        let mut log = LogFile::new(folder.join(LogFile::NAME), 64);
        for n in 0..5 {
            log.append(&format!("line {n} {}", "x".repeat(20))).unwrap();
        }
        let current = fs::read_to_string(folder.join(LogFile::NAME)).unwrap();
        let rolled = fs::read_to_string(log.rolled_path()).unwrap();
        assert!(current.starts_with("line 4"), "{current}");
        assert!(
            rolled.starts_with("line 2") && rolled.contains("line 3"),
            "{rolled}"
        );
        assert!(current.len() as u64 <= 64 && rolled.len() as u64 <= 64);
        // A new handle appends to what is there.
        let mut again = LogFile::new(folder.join(LogFile::NAME), 64);
        again.append("line 5").unwrap();
        let current = fs::read_to_string(folder.join(LogFile::NAME)).unwrap();
        assert!(current.ends_with("line 5\r\n"), "{current}");
        LogFile::write_start_error(&folder, "listen 127.0.0.53:53: in use").unwrap();
        let error = fs::read_to_string(folder.join(LogFile::ERROR_NAME)).unwrap();
        assert_eq!(error, "listen 127.0.0.53:53: in use\r\n");
        fs::remove_dir_all(&folder).unwrap();
    }
}
