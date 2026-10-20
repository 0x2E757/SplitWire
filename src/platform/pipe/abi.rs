//! The named-pipe ABI: the server's instances, the client's checks of who serves the pipe, and
//! cancelling a client thread's blocking read when the server stops.

use crate::platform::abi::{Bool, Raw};

pub const PIPE_ACCESS_DUPLEX: u32 = 0x3;
pub const FILE_FLAG_FIRST_PIPE_INSTANCE: u32 = 0x0008_0000;
pub const PIPE_TYPE_BYTE: u32 = 0x0;
pub const PIPE_READMODE_BYTE: u32 = 0x0;
pub const PIPE_WAIT: u32 = 0x0;
pub const PIPE_REJECT_REMOTE_CLIENTS: u32 = 0x8;
pub const PIPE_UNLIMITED_INSTANCES: u32 = 255;
pub const ERROR_ACCESS_DENIED: i32 = 5;
pub const ERROR_PIPE_BUSY: i32 = 231;
pub const ERROR_PIPE_CONNECTED: i32 = 535;
pub const SE_KERNEL_OBJECT: u32 = 6;
pub const WIN_LOCAL_SYSTEM_SID: u32 = 22;
pub const WIN_BUILTIN_ADMINISTRATORS_SID: u32 = 26;

/// C `SECURITY_ATTRIBUTES`.
#[repr(C)]
pub struct SecurityAttributes {
    pub length: u32,
    pub descriptor: Raw,
    pub inherit: Bool,
}

#[link(name = "kernel32")]
unsafe extern "system" {
    pub fn CreateNamedPipeW(
        name: *const u16,
        open_mode: u32,
        pipe_mode: u32,
        max_instances: u32,
        out_size: u32,
        in_size: u32,
        default_timeout: u32,
        security: *const SecurityAttributes,
    ) -> Raw;
    pub fn ConnectNamedPipe(pipe: Raw, overlapped: Raw) -> Bool;
    pub fn DisconnectNamedPipe(pipe: Raw) -> Bool;
    pub fn FlushFileBuffers(file: Raw) -> Bool;
    pub fn CancelSynchronousIo(thread: Raw) -> Bool;
}

#[link(name = "advapi32")]
unsafe extern "system" {
    pub fn GetSecurityInfo(
        handle: Raw,
        object: u32,
        information: u32,
        owner: *mut Raw,
        group: *mut Raw,
        dacl: *mut Raw,
        sacl: *mut Raw,
        descriptor: *mut Raw,
    ) -> u32;
    pub fn IsWellKnownSid(sid: Raw, kind: u32) -> Bool;
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::mem::{offset_of, size_of};

    #[test]
    fn layouts_match_the_sdk() {
        assert_eq!(size_of::<SecurityAttributes>(), 24); // SECURITY_ATTRIBUTES
        assert_eq!(offset_of!(SecurityAttributes, descriptor), 8); // SECURITY_ATTRIBUTES.lpSecurityDescriptor
        assert_eq!(offset_of!(SecurityAttributes, inherit), 16); // SECURITY_ATTRIBUTES.bInheritHandle
    }
}
