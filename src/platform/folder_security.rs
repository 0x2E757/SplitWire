use std::io;
use std::path::Path;
use std::ptr::null_mut;

use super::abi::{
    ConvertSecurityDescriptorToStringSecurityDescriptorW, DACL_SECURITY_INFORMATION,
    GetNamedSecurityInfoW, LocalFree, OWNER_SECURITY_INFORMATION,
    PROTECTED_DACL_SECURITY_INFORMATION, Raw, SDDL_REVISION_1, SE_FILE_OBJECT,
    SetNamedSecurityInfoW, check_bool, wide_os,
};
use super::security_descriptor::SecurityDescriptor;
use crate::application::FolderAccess;

/// The owner and the protected DACL of the data folder and its `logs\`. Setting them at every
/// start also takes back a folder that a non-administrator created first.
pub struct FolderSecurity;

impl FolderAccess for FolderSecurity {
    fn restrict(&self, root: &Path) -> io::Result<()> {
        Self::secure_data_folder(root)
    }
}

impl FolderSecurity {
    /// `%ProgramData%\SplitWire`: owned by Administrators, full control for SYSTEM and
    /// Administrators only, inherited by everything inside.
    pub const DATA_FOLDER: &'static str = "O:BAD:P(A;OICI;FA;;;SY)(A;OICI;FA;;;BA)";
    /// `logs\` adds read and execute for interactive users; logs never hold secrets.
    pub const LOGS_FOLDER: &'static str =
        "O:BAD:P(A;OICI;FA;;;SY)(A;OICI;FA;;;BA)(A;OICI;0x1200a9;;;IU)";

    /// Secures the data folder and its `logs\` (which must exist).
    pub fn secure_data_folder(root: &Path) -> io::Result<()> {
        Self::apply(root, Self::DATA_FOLDER)?;
        Self::apply(&root.join("logs"), Self::LOGS_FOLDER)
    }

    /// Sets the owner (when `sddl` names one) and the protected DACL of `sddl` on `path`, and lets
    /// the DACL flow down to everything inside.
    pub fn apply(path: &Path, sddl: &str) -> io::Result<()> {
        let descriptor = SecurityDescriptor::parse(sddl)?;
        let owner = descriptor.owner()?;
        let dacl = descriptor.dacl()?;
        let mut information = DACL_SECURITY_INFORMATION | PROTECTED_DACL_SECURITY_INFORMATION;
        if !owner.is_null() {
            information |= OWNER_SECURITY_INFORMATION;
        }
        let path = wide_os(path.as_os_str());
        let status = unsafe {
            SetNamedSecurityInfoW(
                path.as_ptr(),
                SE_FILE_OBJECT,
                information,
                owner,
                null_mut(),
                dacl,
                null_mut(),
            )
        };
        Self::status(status)
    }

    /// The owner and DACL of `path` as SDDL, such as `O:BAD:P(A;OICI;FA;;;SY)…`.
    pub fn read(path: &Path) -> io::Result<String> {
        let path = wide_os(path.as_os_str());
        let mut descriptor: Raw = null_mut();
        let information = OWNER_SECURITY_INFORMATION | DACL_SECURITY_INFORMATION;
        let status = unsafe {
            GetNamedSecurityInfoW(
                path.as_ptr(),
                SE_FILE_OBJECT,
                information,
                null_mut(),
                null_mut(),
                null_mut(),
                null_mut(),
                &mut descriptor,
            )
        };
        Self::status(status)?;
        let descriptor = SecurityDescriptor::from_raw(descriptor);
        let mut text: *mut u16 = null_mut();
        let mut length = 0u32;
        check_bool(unsafe {
            ConvertSecurityDescriptorToStringSecurityDescriptorW(
                descriptor.raw(),
                SDDL_REVISION_1,
                information,
                &mut text,
                &mut length,
            )
        })?;
        // `length` counts the terminating NUL.
        let units = unsafe { std::slice::from_raw_parts(text, length as usize) };
        let sddl = String::from_utf16_lossy(units)
            .trim_end_matches('\0')
            .to_owned();
        unsafe { LocalFree(text.cast()) };
        Ok(sddl)
    }

    fn status(code: u32) -> io::Result<()> {
        match code {
            0 => Ok(()),
            code => Err(io::Error::from_raw_os_error(code as i32)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn the_folder_policies_are_valid_sddl() {
        for sddl in [FolderSecurity::DATA_FOLDER, FolderSecurity::LOGS_FOLDER] {
            let descriptor = SecurityDescriptor::parse(sddl).unwrap();
            assert!(!descriptor.owner().unwrap().is_null());
            assert!(!descriptor.dacl().unwrap().is_null());
        }
        assert!(SecurityDescriptor::parse("D:P(nonsense)").is_err());
        assert!(SecurityDescriptor::parse("O:BA").unwrap().dacl().is_err());
    }

    /// Changes an ACL for real, so it runs only on a scratch folder named by
    /// `SPLITWIRE_ACL_SCRATCH`, and it keeps the current owner's full control.
    #[test]
    fn applies_a_protected_dacl_to_a_scratch_folder() {
        let Some(scratch) = std::env::var_os("SPLITWIRE_ACL_SCRATCH") else {
            eprintln!("skipped: set SPLITWIRE_ACL_SCRATCH to a scratch folder to run it");
            return;
        };
        let folder = PathBuf::from(scratch).join(format!("acl-{}", std::process::id()));
        std::fs::create_dir_all(&folder).unwrap();
        let before = FolderSecurity::read(&folder).unwrap();
        let owner = before
            .strip_prefix("O:")
            .and_then(|rest| rest.split("D:").next())
            .unwrap()
            .to_owned();
        let sddl = format!(
            "D:P(A;OICI;FA;;;SY)(A;OICI;FA;;;BA)(A;OICI;FA;;;{owner})(A;OICI;0x1200a9;;;IU)"
        );
        FolderSecurity::apply(&folder, &sddl).unwrap();
        let after = FolderSecurity::read(&folder).unwrap();
        assert!(after.starts_with(&format!("O:{owner}D:P")), "{after}");
        assert!(after.contains(";;;IU)"), "{after}");
        assert!(
            after.contains(";;;SY)") && after.contains(";;;BA)"),
            "{after}"
        );
        let file = folder.join("inherits.txt");
        std::fs::write(&file, b"x").unwrap();
        let inherited = FolderSecurity::read(&file).unwrap();
        assert!(
            inherited.contains("ID;") && inherited.contains(";;;IU)"),
            "{inherited}"
        );
        std::fs::remove_dir_all(&folder).unwrap();
    }
}
