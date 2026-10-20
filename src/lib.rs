//! SplitWire: domain-based split tunnelling over wireguard-nt. The domain modules are pure logic;
//! everything that touches the machine or the network is a trait implemented in `platform`, so the
//! logic is tested with fakes.

pub mod alerts;
pub mod application;
pub mod cli;
pub mod clock;
pub mod config;
pub mod dns;
pub mod ipc;
pub mod list;
pub mod logs;
pub mod net;
pub mod platform;
pub mod presentation;
pub mod profile;
pub mod routes;
pub mod stats;
pub mod tunnel;

#[cfg(not(all(target_os = "windows", target_arch = "x86_64", target_env = "msvc")))]
compile_error!("SplitWire requires x86_64-pc-windows-msvc.");
