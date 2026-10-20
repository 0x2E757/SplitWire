use std::fs::File;
use std::io;
use std::os::windows::io::AsRawHandle;
use std::ptr::null_mut;

use super::abi::{
    GetSecurityInfo, IsWellKnownSid, SE_KERNEL_OBJECT, SecurityAttributes,
    WIN_BUILTIN_ADMINISTRATORS_SID, WIN_LOCAL_SYSTEM_SID,
};
use crate::platform::abi::{
    ConvertSecurityDescriptorToStringSecurityDescriptorW, DACL_SECURITY_INFORMATION, LocalFree,
    OWNER_SECURITY_INFORMATION, Raw, SDDL_REVISION_1, check_bool,
};
use crate::platform::security_descriptor::SecurityDescriptor;

/// Who may use the pipe: SYSTEM, Administrators and the pipe's owner fully;
/// interactive users read and write but cannot create an instance, so none of them can serve it.
/// A client checks the other way round that the service's pipe is served by SYSTEM or an
/// administrator before it sends anything, a `.conf` in particular.
pub struct PipeSecurity {
    descriptor: SecurityDescriptor,
}

impl PipeSecurity {
    pub const SDDL: &'static str = "D:P(A;;GA;;;SY)(A;;GA;;;BA)(A;;GA;;;OW)(A;;0x12008b;;;IU)";

    pub fn new() -> io::Result<Self> {
        Ok(Self {
            descriptor: SecurityDescriptor::parse(Self::SDDL)?,
        })
    }

    /// The attributes `CreateNamedPipeW` takes; valid while `self` lives.
    pub fn attributes(&self) -> SecurityAttributes {
        SecurityAttributes {
            length: size_of::<SecurityAttributes>() as u32,
            descriptor: self.descriptor.raw(),
            inherit: 0,
        }
    }

    /// Whether the pipe behind `pipe` belongs to LocalSystem or the Administrators, which only an
    /// elevated process can arrange: a user's process squatting the name is refused.
    pub fn served_by_admin(pipe: &File) -> io::Result<bool> {
        let mut owner: Raw = null_mut();
        let descriptor = Self::descriptor(pipe, OWNER_SECURITY_INFORMATION, &mut owner)?;
        let trusted = [WIN_LOCAL_SYSTEM_SID, WIN_BUILTIN_ADMINISTRATORS_SID]
            .iter()
            .any(|kind| unsafe { IsWellKnownSid(owner, *kind) } != 0);
        drop(descriptor);
        Ok(trusted)
    }

    /// The DACL of the pipe behind `pipe`, as SDDL.
    pub fn dacl_of(pipe: &File) -> io::Result<String> {
        let mut unused: Raw = null_mut();
        let descriptor = Self::descriptor(pipe, DACL_SECURITY_INFORMATION, &mut unused)?;
        let mut text: *mut u16 = null_mut();
        let mut length = 0u32;
        check_bool(unsafe {
            ConvertSecurityDescriptorToStringSecurityDescriptorW(
                descriptor.raw(),
                SDDL_REVISION_1,
                DACL_SECURITY_INFORMATION,
                &mut text,
                &mut length,
            )
        })?;
        let units = unsafe { std::slice::from_raw_parts(text, length as usize) };
        let sddl = String::from_utf16_lossy(units)
            .trim_end_matches('\0')
            .to_owned();
        unsafe { LocalFree(text.cast()) };
        Ok(sddl)
    }

    fn descriptor(
        pipe: &File,
        information: u32,
        owner: &mut Raw,
    ) -> io::Result<SecurityDescriptor> {
        let mut descriptor: Raw = null_mut();
        let status = unsafe {
            GetSecurityInfo(
                pipe.as_raw_handle(),
                SE_KERNEL_OBJECT,
                information,
                owner,
                null_mut(),
                null_mut(),
                null_mut(),
                &mut descriptor,
            )
        };
        if status != 0 {
            return Err(io::Error::from_raw_os_error(status as i32));
        }
        Ok(SecurityDescriptor::from_raw(descriptor))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_pipe_policy_is_valid_sddl() {
        let security = PipeSecurity::new().unwrap();
        let attributes = security.attributes();
        assert_eq!(attributes.length, 24);
        assert!(!attributes.descriptor.is_null());
    }

    #[test]
    fn an_interactive_user_is_granted_what_the_client_asks_for() {
        // The client's GENERIC_READ maps to FILE_GENERIC_READ for a pipe.
        const FILE_GENERIC_READ: u32 = 0x0012_0089;
        const FILE_WRITE_DATA: u32 = 0x0000_0002;
        let asked = FILE_GENERIC_READ | FILE_WRITE_DATA;
        assert_eq!(
            super::super::PipeClient::ACCESS,
            0x8000_0000 | FILE_WRITE_DATA
        );
        let interactive = PipeSecurity::SDDL
            .split("(A;;")
            .find_map(|ace| ace.strip_suffix(";;;IU)"))
            .and_then(|mask| u32::from_str_radix(mask.trim_start_matches("0x"), 16).ok())
            .unwrap();
        assert_eq!(
            asked & !interactive,
            0,
            "the client asks for more than {interactive:#x}"
        );
    }
}
