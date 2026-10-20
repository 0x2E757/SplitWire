//! The Windows service: the SCM entry, the control handler for stop, shutdown and
//! power events, and the installer. Install and uninstall are run by a person, from an elevated
//! console; agents and tests never register, start or stop a service.

mod abi;
mod control_handler;
mod dispatcher;
mod installer;

pub use control_handler::{ControlHandler, ServiceEvents};
pub use dispatcher::Dispatcher;
pub use installer::Installer;
