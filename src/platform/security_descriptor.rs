use std::io;
use std::ptr::null_mut;

use super::abi::{
    Bool, ConvertStringSecurityDescriptorToSecurityDescriptorW, GetSecurityDescriptorDacl,
    GetSecurityDescriptorOwner, LocalFree, Raw, SDDL_REVISION_1, check_bool, wide,
};

/// A self-relative security descriptor allocated by Windows, freed on drop: the data folder's ACL
/// and the pipe's DACL are built from SDDL through it.
pub struct SecurityDescriptor(Raw);

// The descriptor is plain memory owned by this value; Windows only reads it.
unsafe impl Send for SecurityDescriptor {}
unsafe impl Sync for SecurityDescriptor {}

impl SecurityDescriptor {
    pub fn parse(sddl: &str) -> io::Result<Self> {
        let text = wide(sddl);
        let mut descriptor: Raw = null_mut();
        check_bool(unsafe {
            ConvertStringSecurityDescriptorToSecurityDescriptorW(
                text.as_ptr(),
                SDDL_REVISION_1,
                &mut descriptor,
                null_mut(),
            )
        })?;
        Ok(Self(descriptor))
    }

    /// Takes ownership of a descriptor Windows allocated with `LocalAlloc`.
    pub fn from_raw(raw: Raw) -> Self {
        Self(raw)
    }

    pub fn raw(&self) -> Raw {
        self.0
    }

    /// The owner SID inside the descriptor, or null when it names none.
    pub fn owner(&self) -> io::Result<Raw> {
        let mut owner: Raw = null_mut();
        let mut defaulted: Bool = 0;
        check_bool(unsafe { GetSecurityDescriptorOwner(self.0, &mut owner, &mut defaulted) })?;
        Ok(owner)
    }

    pub fn dacl(&self) -> io::Result<Raw> {
        let mut present: Bool = 0;
        let mut dacl: Raw = null_mut();
        let mut defaulted: Bool = 0;
        check_bool(unsafe {
            GetSecurityDescriptorDacl(self.0, &mut present, &mut dacl, &mut defaulted)
        })?;
        if present == 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "the SDDL has no DACL",
            ));
        }
        Ok(dacl)
    }
}

impl Drop for SecurityDescriptor {
    fn drop(&mut self) {
        unsafe { LocalFree(self.0) };
    }
}
