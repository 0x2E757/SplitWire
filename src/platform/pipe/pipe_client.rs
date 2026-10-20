use std::fmt;
use std::fs::{File, OpenOptions};
use std::io;
use std::os::windows::fs::OpenOptionsExt;
use std::thread;
use std::time::{Duration, Instant};

use super::abi::ERROR_PIPE_BUSY;
use super::{PipeName, PipeSecurity};
use crate::ipc::{Frame, Request, Response};
use crate::profile::SecretBytes;

/// Why a request got no response.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PipeError {
    /// No engine serves the pipe (the CLI's exit code 3, the window's "service not running").
    NotRunning,
    /// The exchange broke off, or the pipe is not the service's; the next request tries again.
    Failed(String),
}

impl fmt::Display for PipeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PipeError::NotRunning => f.write_str("the SplitWire service is not running"),
            PipeError::Failed(why) => f.write_str(why),
        }
    }
}

/// A client of the pipe: one request, one response, over a connection kept between requests and
/// opened again when the service restarted. Before the service's pipe gets anything, its owner
/// must be SYSTEM or the Administrators, so a user's process squatting the name never receives a
/// `.conf`. The frame of a request is zeroed once sent.
pub struct PipeClient {
    name: PipeName,
    pipe: Option<File>,
}

impl PipeClient {
    /// How long a busy pipe (every instance taken) is waited for.
    const BUSY_WAIT: Duration = Duration::from_secs(2);
    const SECURITY_SQOS_PRESENT: u32 = 0x0010_0000;
    /// The service may identify the client but never act as it.
    const SECURITY_IDENTIFICATION: u32 = 0x0001_0000;
    /// `GENERIC_READ | FILE_WRITE_DATA`: what the interactive users' ACE grants. `GENERIC_WRITE`
    /// would also ask for `FILE_CREATE_PIPE_INSTANCE` and the attributes, which only the
    /// service's own accounts have, so an unelevated client would be denied.
    pub const ACCESS: u32 = 0x8000_0000 | 0x0000_0002;

    pub fn new(name: PipeName) -> Self {
        Self { name, pipe: None }
    }

    pub fn name(&self) -> &PipeName {
        &self.name
    }

    pub fn exchange(&mut self, request: &Request) -> Result<Response, PipeError> {
        let frame = SecretBytes::from_vec(
            request
                .encode()
                .map_err(|e| PipeError::Failed(e.to_string()))?,
        );
        // A kept connection may have gone with a restarted service: a failed send is tried once
        // more on a new one. A request that was sent is never sent twice.
        let reused = self.pipe.is_some();
        let mut pipe = self.take_or_open()?;
        let sent = match Frame::write(&mut pipe, frame.expose()) {
            Err(_) if reused => {
                pipe = self.open()?;
                Frame::write(&mut pipe, frame.expose())
            }
            result => result,
        };
        drop(frame);
        sent.map_err(|e| PipeError::Failed(format!("sending to {} failed: {e}", self.name)))?;
        let body = Frame::read(&mut pipe)
            .map_err(|e| PipeError::Failed(format!("reading from {} failed: {e}", self.name)))?;
        let response =
            Response::decode(&body).map_err(|e| PipeError::Failed(format!("bad response: {e}")))?;
        self.pipe = Some(pipe);
        Ok(response)
    }

    fn take_or_open(&mut self) -> Result<File, PipeError> {
        match self.pipe.take() {
            Some(pipe) => Ok(pipe),
            None => self.open(),
        }
    }

    fn open(&self) -> Result<File, PipeError> {
        let deadline = Instant::now() + Self::BUSY_WAIT;
        let pipe = loop {
            let opened = OpenOptions::new()
                .access_mode(Self::ACCESS)
                .security_qos_flags(Self::SECURITY_SQOS_PRESENT | Self::SECURITY_IDENTIFICATION)
                .open(self.name.path());
            match opened {
                Ok(pipe) => break pipe,
                Err(e) if e.kind() == io::ErrorKind::NotFound => {
                    return Err(PipeError::NotRunning);
                }
                Err(e)
                    if e.raw_os_error() == Some(ERROR_PIPE_BUSY) && Instant::now() < deadline =>
                {
                    thread::sleep(Duration::from_millis(50));
                }
                Err(e) => {
                    return Err(PipeError::Failed(format!(
                        "opening {} failed: {e}",
                        self.name.path()
                    )));
                }
            }
        };
        self.check_server(&pipe)?;
        Ok(pipe)
    }

    fn check_server(&self, pipe: &File) -> Result<(), PipeError> {
        if !self.name.is_service() {
            return Ok(());
        }
        match PipeSecurity::served_by_admin(pipe) {
            Ok(true) => Ok(()),
            Ok(false) => Err(PipeError::Failed(format!(
                "{} is not served by the SplitWire service; nothing was sent",
                self.name.path()
            ))),
            Err(e) => Err(PipeError::Failed(format!(
                "checking who serves {} failed: {e}",
                self.name.path()
            ))),
        }
    }
}
