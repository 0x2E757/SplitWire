//! The window's boundary to the engine: one request, one response. The pipe client
//! (`platform::pipe::PipeClient`) implements it against the service; the demo implementation
//! serves the demo scenarios from IPC message values, so the window and its checks run without a
//! service.

pub mod demo;

use crate::ipc::{Request, Response};

/// Why a request got no response.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TransportError {
    /// No service answers: the pipe is absent (the window's "service not running" state).
    NotRunning,
    /// The exchange broke off; the next request tries again.
    Failed(String),
}

/// The window's only way to the engine. Requests are sent from one worker thread. The pipe
/// client's `exchange(&Request)` and its `NotRunning` / `Failed` errors map onto it one to one.
pub trait EngineTransport: Send {
    fn exchange(&mut self, request: &Request) -> Result<Response, TransportError>;
}
