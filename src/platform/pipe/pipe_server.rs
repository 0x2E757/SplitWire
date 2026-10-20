use std::fs::{File, OpenOptions};
use std::io;
use std::os::windows::io::{AsRawHandle, FromRawHandle};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use super::abi::{
    CancelSynchronousIo, ConnectNamedPipe, CreateNamedPipeW, DisconnectNamedPipe,
    ERROR_ACCESS_DENIED, ERROR_PIPE_CONNECTED, FILE_FLAG_FIRST_PIPE_INSTANCE, FlushFileBuffers,
    PIPE_ACCESS_DUPLEX, PIPE_READMODE_BYTE, PIPE_REJECT_REMOTE_CLIENTS, PIPE_TYPE_BYTE,
    PIPE_UNLIMITED_INSTANCES, PIPE_WAIT,
};
use super::{PipeName, PipeSecurity};
use crate::ipc::{ErrorCode, ErrorReply, Frame, Response};
use crate::logs::Log;
use crate::platform::abi::{INVALID_HANDLE_VALUE, last_error, wide};
use crate::profile::SecretBytes;

/// Answers one request frame body with a whole response frame.
pub type FrameHandler = Arc<dyn Fn(&[u8]) -> Vec<u8> + Send + Sync>;

/// The first instance of the pipe, created before the engine starts: holding it proves no other
/// process serves the name (`FILE_FLAG_FIRST_PIPE_INSTANCE`).
pub struct PipeListener {
    name: PipeName,
    security: PipeSecurity,
    first: File,
}

impl PipeListener {
    pub fn bind(name: PipeName) -> io::Result<Self> {
        let security = PipeSecurity::new()?;
        let first = Instances::create(&name, &security, true)?;
        Ok(Self {
            name,
            security,
            first,
        })
    }

    /// Serves the pipe on its own thread: one thread per client, at most
    /// `PipeServer::MAX_CLIENTS` at once.
    pub fn serve(self, handler: FrameHandler, log: Log) -> io::Result<PipeServer> {
        let log = log.with_source("pipe");
        let name = self.name.clone();
        let shared = Arc::new(Shared::default());
        let accept = {
            let shared = shared.clone();
            let log = log.clone();
            thread::Builder::new()
                .name("pipe".into())
                .spawn(move || Accept::run(self, &shared, &handler, &log))?
        };
        Ok(PipeServer {
            name,
            shared,
            accept,
            log,
        })
    }

    pub fn name(&self) -> &PipeName {
        &self.name
    }
}

/// The served pipe. `stop` ends the accept thread and every client, so no request reaches the
/// engine after it.
pub struct PipeServer {
    name: PipeName,
    shared: Arc<Shared>,
    accept: JoinHandle<()>,
    log: Log,
}

impl PipeServer {
    pub const MAX_CLIENTS: usize = 8;
    /// How long `stop` waits for a request being answered.
    const STOP_WAIT: Duration = Duration::from_secs(30);

    pub fn name(&self) -> &PipeName {
        &self.name
    }

    pub fn stop(self) {
        let Self {
            name,
            shared,
            accept,
            log,
        } = self;
        {
            // Set under the slots' lock, so a wait for a slot cannot miss the wake-up.
            let _busy = shared.slots.0.lock().unwrap();
            shared.stopping.store(true, Ordering::SeqCst);
        }
        shared.slots.1.notify_all();
        // The accept thread waits in ConnectNamedPipe: a connection of our own wakes it.
        while !accept.is_finished() {
            let _ = OpenOptions::new().read(true).write(true).open(name.path());
            thread::sleep(Duration::from_millis(20));
        }
        let _ = accept.join();
        let deadline = Instant::now() + Self::STOP_WAIT;
        let clients = std::mem::take(&mut *shared.clients.lock().unwrap());
        for client in clients {
            client.end(deadline);
        }
        log.info("pipe closed");
    }
}

#[derive(Default)]
struct Shared {
    stopping: AtomicBool,
    clients: Mutex<Vec<Client>>,
    /// The clients being served, and a wake-up when one ends or the server stops.
    slots: (Mutex<usize>, Condvar),
}

impl Shared {
    fn stopping(&self) -> bool {
        self.stopping.load(Ordering::SeqCst)
    }

    /// Waits until fewer than `MAX_CLIENTS` are served; false when the server stops.
    fn wait_for_slot(&self) -> bool {
        let mut busy = self.slots.0.lock().unwrap();
        while *busy >= PipeServer::MAX_CLIENTS && !self.stopping() {
            busy = self.slots.1.wait(busy).unwrap();
        }
        !self.stopping()
    }

    fn take_slot(&self) {
        *self.slots.0.lock().unwrap() += 1;
    }

    fn free_slot(&self) {
        *self.slots.0.lock().unwrap() -= 1;
        self.slots.1.notify_all();
    }
}

/// The accept loop: wait for a client on the free instance, hand it to a thread, make the next.
struct Accept;

impl Accept {
    fn run(listener: PipeListener, shared: &Arc<Shared>, handler: &FrameHandler, log: &Log) {
        let PipeListener {
            name,
            security,
            first,
        } = listener;
        log.info(format!("serving {}", name.path()));
        let mut instance = first;
        loop {
            let connected =
                unsafe { ConnectNamedPipe(instance.as_raw_handle(), std::ptr::null_mut()) } != 0
                    || last_error().raw_os_error() == Some(ERROR_PIPE_CONNECTED);
            if shared.stopping() {
                break;
            }
            if connected {
                Self::spawn(instance, shared, handler, log);
            } else {
                log.warn(format!("a client could not connect: {}", last_error()));
            }
            if !shared.wait_for_slot() {
                break;
            }
            instance = match Self::next(&name, &security, shared, log) {
                Some(instance) => instance,
                None => break,
            };
        }
    }

    /// The next instance; a failure is retried every second until the server stops.
    fn next(name: &PipeName, security: &PipeSecurity, shared: &Shared, log: &Log) -> Option<File> {
        loop {
            match Instances::create(name, security, false) {
                Ok(instance) => return Some(instance),
                Err(e) => log.error(format!("creating a pipe instance failed: {e}")),
            }
            thread::sleep(Duration::from_secs(1));
            if shared.stopping() {
                return None;
            }
        }
    }

    fn spawn(pipe: File, shared: &Arc<Shared>, handler: &FrameHandler, log: &Log) {
        let reading = Arc::new(AtomicBool::new(false));
        let session = Session {
            pipe,
            shared: shared.clone(),
            handler: handler.clone(),
            reading: reading.clone(),
            log: log.clone(),
        };
        shared.take_slot();
        let spawned = thread::Builder::new()
            .name("pipe client".into())
            .spawn(move || session.run());
        let mut clients = shared.clients.lock().unwrap();
        clients.retain(|client| !client.thread.is_finished());
        match spawned {
            Ok(thread) => clients.push(Client { thread, reading }),
            Err(e) => {
                shared.free_slot();
                log.error(format!("starting a client thread failed: {e}"));
            }
        }
    }
}

/// A client thread and whether it is waiting for a request (the only time `stop` cancels it).
struct Client {
    thread: JoinHandle<()>,
    reading: Arc<AtomicBool>,
}

impl Client {
    fn end(self, deadline: Instant) {
        while !self.thread.is_finished() && Instant::now() < deadline {
            if self.reading.load(Ordering::SeqCst) {
                unsafe { CancelSynchronousIo(self.thread.as_raw_handle()) };
            }
            thread::sleep(Duration::from_millis(10));
        }
        if self.thread.is_finished() {
            let _ = self.thread.join();
        }
    }
}

/// One connected client: requests answered in turn until it hangs up or the server stops.
struct Session {
    pipe: File,
    shared: Arc<Shared>,
    handler: FrameHandler,
    reading: Arc<AtomicBool>,
    log: Log,
}

impl Session {
    fn run(mut self) {
        while !self.shared.stopping() {
            self.reading.store(true, Ordering::SeqCst);
            let read = if self.shared.stopping() {
                Err(io::ErrorKind::Interrupted.into())
            } else {
                Frame::read(&mut self.pipe)
            };
            self.reading.store(false, Ordering::SeqCst);
            match read {
                Ok(body) => {
                    // A request may carry a `.conf`: its bytes are zeroed once answered.
                    let body = SecretBytes::from_vec(body);
                    let response = (self.handler)(body.expose());
                    drop(body);
                    if Frame::write(&mut self.pipe, &response).is_err() {
                        break;
                    }
                }
                Err(e) if e.kind() == io::ErrorKind::InvalidData => {
                    self.refuse(&e);
                    break;
                }
                Err(_) => break,
            }
        }
        unsafe { DisconnectNamedPipe(self.pipe.as_raw_handle()) };
        self.shared.free_slot();
    }

    /// A frame that cannot be read leaves the stream out of step: answer it, then hang up.
    fn refuse(&mut self, error: &io::Error) {
        self.log.warn(format!("a client sent a bad frame: {error}"));
        let reply = Response::Error(ErrorReply {
            code: ErrorCode::BadRequest,
            message: error.to_string(),
        });
        if let Ok(frame) = reply.encode()
            && Frame::write(&mut self.pipe, &frame).is_ok()
        {
            unsafe { FlushFileBuffers(self.pipe.as_raw_handle()) };
        }
    }
}

/// `CreateNamedPipeW` with the pipe's mode and DACL.
struct Instances;

impl Instances {
    const BUFFER: u32 = 64 << 10;

    fn create(name: &PipeName, security: &PipeSecurity, first: bool) -> io::Result<File> {
        let path = wide(&name.path());
        let attributes = security.attributes();
        let open_mode = PIPE_ACCESS_DUPLEX
            | if first {
                FILE_FLAG_FIRST_PIPE_INSTANCE
            } else {
                0
            };
        let raw = unsafe {
            CreateNamedPipeW(
                path.as_ptr(),
                open_mode,
                PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_WAIT | PIPE_REJECT_REMOTE_CLIENTS,
                PIPE_UNLIMITED_INSTANCES,
                Self::BUFFER,
                Self::BUFFER,
                0,
                &attributes,
            )
        };
        if raw == INVALID_HANDLE_VALUE {
            let error = last_error();
            if first && error.raw_os_error() == Some(ERROR_ACCESS_DENIED) {
                return Err(io::Error::new(
                    io::ErrorKind::AlreadyExists,
                    format!("another process already serves {}", name.path()),
                ));
            }
            return Err(error);
        }
        Ok(unsafe { File::from_raw_handle(raw) })
    }
}
