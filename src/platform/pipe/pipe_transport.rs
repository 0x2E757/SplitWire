//! The pipe client as the window's way to the engine: its errors map onto the transport's one to
//! one, so the window shows "service not running" exactly when the CLI exits with 3.

use super::{PipeClient, PipeError};
use crate::ipc::{Request, Response};
use crate::presentation::transport::{EngineTransport, TransportError};

impl From<PipeError> for TransportError {
    fn from(error: PipeError) -> Self {
        match error {
            PipeError::NotRunning => TransportError::NotRunning,
            PipeError::Failed(why) => TransportError::Failed(why),
        }
    }
}

impl EngineTransport for PipeClient {
    fn exchange(&mut self, request: &Request) -> Result<Response, TransportError> {
        // The inherent method, not this one: the names are the same.
        PipeClient::exchange(self, request).map_err(TransportError::from)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::platform::pipe::PipeName;

    #[test]
    fn an_absent_pipe_is_a_service_not_running() {
        let name = format!("SplitWire.Test.{}.absent", std::process::id());
        let mut client = PipeClient::new(PipeName::parse(&name).unwrap());
        let transport: &mut dyn EngineTransport = &mut client;
        assert_eq!(
            transport.exchange(&Request::Status),
            Err(TransportError::NotRunning)
        );
        assert_eq!(
            TransportError::from(PipeError::Failed("broke".into())),
            TransportError::Failed("broke".into())
        );
    }
}
