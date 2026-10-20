//! Windows adapters: thin wrappers over the hand-written ABI, without policy. Unsafe ABI
//! operations stay in this module. `composition::PlatformSet` is the only place naming them.

pub mod abi;
pub mod atomic_file;
pub mod com;
pub mod composition;
pub mod console;
pub mod dns_exchange;
pub mod dns_flush;
pub mod dpapi;
pub mod dry_run;
pub mod elevation;
pub mod engine_mutex;
pub mod executables;
pub mod folder_security;
pub mod inet;
pub mod ip_helper;
pub mod library;
pub mod local_clock;
pub mod nrpt;
pub mod pipe;
pub mod registry;
pub mod security_descriptor;
pub mod service;
pub mod sha256;
pub mod window;
pub mod wireguard;
