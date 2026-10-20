//! The pipe between the service and its clients: `\\.\pipe\SplitWire` in byte mode,
//! created as the first instance, rejecting remote clients, with a DACL that lets interactive
//! users talk to the service but never serve the pipe themselves.

mod abi;
mod pipe_client;
mod pipe_name;
mod pipe_security;
mod pipe_server;
mod pipe_transport;

#[cfg(test)]
mod tests;

pub use pipe_client::{PipeClient, PipeError};
pub use pipe_name::PipeName;
pub use pipe_security::PipeSecurity;
pub use pipe_server::{FrameHandler, PipeListener, PipeServer};
