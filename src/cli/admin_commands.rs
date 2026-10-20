use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use super::ExitCode;
use crate::application::DataFolder;
use crate::clock::SystemClock;
use crate::config::Settings;
use crate::dns::{DnsCacheFlusher, NrptSync};
use crate::logs::Log;
use crate::platform::dns_flush::DnsCacheFlush;
use crate::platform::elevation::Elevation;
use crate::platform::folder_security::FolderSecurity;
use crate::platform::library::AppLibrary;
use crate::platform::nrpt::RegistryNrpt;
use crate::platform::service::Installer;
use crate::platform::wireguard::WireGuardNt;
use crate::tunnel::WireGuardDriver;

/// Where `install` accepts the executable: `%ProgramFiles%\SplitWire` only, because a service
/// binary or DLL a user can write would be a path to SYSTEM.
pub struct InstallLocation;

impl InstallLocation {
    pub fn check(exe: &Path, program_files: &Path) -> Result<(), String> {
        let expected = program_files.join("SplitWire");
        let folder = exe.parent().unwrap_or(Path::new(""));
        let same = folder
            .to_string_lossy()
            .eq_ignore_ascii_case(&expected.to_string_lossy());
        if same {
            Ok(())
        } else {
            Err(format!(
                "install runs only from {}; this is {}",
                expected.display(),
                folder.display()
            ))
        }
    }

    pub fn program_files() -> PathBuf {
        std::env::var_os("ProgramFiles")
            .map_or_else(|| PathBuf::from(r"C:\Program Files"), PathBuf::from)
    }
}

/// `install`, `uninstall` and `cleanup`: an administrator's, from an elevated
/// console, so each says so with exit code 4 otherwise. Agents never run them.
pub struct AdminCommands;

impl AdminCommands {
    /// Creates and secures the data folder, registers or updates the service and starts it;
    /// `--restore` (an update) starts it restoring `connected=` regardless of `connect_at_boot`.
    pub fn install(restore: bool) -> i32 {
        Self::admin("install", || {
            let exe = std::env::current_exe()?;
            InstallLocation::check(&exe, &InstallLocation::program_files())
                .map_err(io::Error::other)?;
            let folder = DataFolder::new(DataFolder::default_root());
            fs::create_dir_all(folder.logs())?;
            FolderSecurity::secure_data_folder(folder.root())?;
            Installer::register(&exe)?;
            Installer::start(if restore { &["--restore"] } else { &[] })?;
            println!("The SplitWire service is installed and started.");
            Ok(())
        })
    }

    /// Stops the service (which disconnects everything) and deletes it, and removes the driver
    /// when no WireGuard adapter is left. The data folder stays for a later install; `--purge`
    /// deletes it.
    pub fn uninstall(purge: bool) -> i32 {
        Self::admin("uninstall", || {
            Installer::remove()?;
            println!("The SplitWire service is stopped and deleted.");
            let log = Log::new(1 << 16, Arc::new(SystemClock), true);
            let driver = WireGuardNt::new(AppLibrary::beside_exe()?, log);
            match driver.delete_driver() {
                Ok(()) => println!("The wireguard-nt driver is removed."),
                Err(e) => println!("The wireguard-nt driver stays ({e}): another adapter uses it."),
            }
            if !purge {
                return Ok(());
            }
            let root = DataFolder::default_root();
            match fs::remove_dir_all(&root) {
                Ok(()) => println!("Deleted {}.", root.display()),
                Err(e) if e.kind() == io::ErrorKind::NotFound => {}
                Err(e) => return Err(e),
            }
            Ok(())
        })
    }

    /// The start cleanup without an engine: every SplitWire NRPT rule removed, the Windows DNS
    /// cache flushed. Adapters and routes cannot outlive the engine's process.
    pub fn cleanup() -> i32 {
        Self::admin("cleanup", || {
            let log = Log::new(1 << 16, Arc::new(SystemClock), true);
            let rules = RegistryNrpt::live(*Settings::default().listen.ip());
            NrptSync::new(Arc::new(rules), log).remove_all()?;
            DnsCacheFlush.flush()?;
            println!("Every SplitWire NRPT rule is removed and the DNS cache flushed.");
            Ok(())
        })
    }

    fn admin(command: &str, work: impl FnOnce() -> io::Result<()>) -> i32 {
        if !Elevation::is_elevated().unwrap_or(false) {
            eprintln!("SplitWire: {command} needs an administrator console");
            return ExitCode::ADMIN_REQUIRED;
        }
        match work() {
            Ok(()) => ExitCode::OK,
            Err(e) => {
                eprintln!("SplitWire: {command} failed: {e}");
                ExitCode::ERROR
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn install_accepts_only_the_program_files_folder() {
        let program_files = Path::new(r"C:\Program Files");
        let ok = Path::new(r"C:\Program Files\SplitWire\SplitWire.exe");
        assert!(InstallLocation::check(ok, program_files).is_ok());
        let case = Path::new(r"c:\program files\splitwire\SplitWire.exe");
        assert!(InstallLocation::check(case, program_files).is_ok());
        for bad in [
            r"D:\Downloads\SplitWire\SplitWire.exe",
            r"C:\Program Files\SplitWire\bin\SplitWire.exe",
            r"C:\Program Files\SplitWire2\SplitWire.exe",
            "SplitWire.exe",
        ] {
            assert!(
                InstallLocation::check(Path::new(bad), program_files).is_err(),
                "{bad}"
            );
        }
    }
}
