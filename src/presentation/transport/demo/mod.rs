//! The demo transport: a stateful stand-in for the service that serves the demo scenarios as
//! IPC message values (`SplitWire.Window.exe --demo <scenario>`, `--verify-pages`, the tests).

mod alerts;
mod engine;
mod lists;
mod logs;
mod model;
mod profiles;
mod reports;
mod scenario;
mod scenarios;
mod traffic;

pub use engine::DemoEngine;
pub use scenario::Scenario;

use super::{EngineTransport, TransportError};
use crate::ipc::{Frame, Request, Response};

pub struct DemoTransport {
    engine: DemoEngine,
}

impl DemoTransport {
    pub fn new(scenario: Scenario) -> Self {
        Self {
            engine: DemoEngine::new(scenario),
        }
    }

    /// The demo's `logs\splitwire.log`.
    pub fn log_file(scenario: Scenario) -> String {
        logs::DemoLogs::file(scenario)
    }
}

impl EngineTransport for DemoTransport {
    fn exchange(&mut self, request: &Request) -> Result<Response, TransportError> {
        if !self.engine.service_running() {
            return Err(TransportError::NotRunning);
        }
        // Encoding both ways keeps the demo honest: every message crosses the codec.
        let failed = |e: crate::ipc::WireError| TransportError::Failed(e.to_string());
        let frame = request.encode().map_err(failed)?;
        let request = Request::decode(Frame::body(&frame).map_err(failed)?).map_err(failed)?;
        let frame = self.engine.handle(request).encode().map_err(failed)?;
        Response::decode(Frame::body(&frame).map_err(failed)?).map_err(failed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_scenario_answers_through_the_codec() {
        for scenario in Scenario::ALL {
            let mut transport = DemoTransport::new(scenario);
            let status = transport.exchange(&Request::Status);
            match scenario {
                Scenario::NoService => assert_eq!(status, Err(TransportError::NotRunning)),
                _ => assert!(matches!(status, Ok(Response::Status(_)))),
            }
        }
    }
}
