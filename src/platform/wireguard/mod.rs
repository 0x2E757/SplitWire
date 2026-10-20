//! wireguard-nt through `wireguard.dll`: the function table (`Api`), the `wireguard.h` structures
//! (`layout`), the configuration bytes (`ConfigBuffer`), the driver (`WireGuardNt`, the real
//! `WireGuardDriver`) and its logger (`DriverLog`). Tests load the DLL from a scratch folder and
//! resolve its symbols; nothing in it is called outside a live run.

mod api;
mod config_buffer;
mod driver_log;
pub mod layout;
mod nt;

pub use api::{AdapterHandle, Api, LoggerCallback};
pub use config_buffer::ConfigBuffer;
pub use driver_log::DriverLog;
pub use nt::WireGuardNt;
