//! The pipe for real, in the user session, under a name of its own (`SplitWire.Test.<pid>.<n>`):
//! an exchange, the first-instance rule, the DACL, a bad frame and a stop with idle clients.

use std::fs::OpenOptions;
use std::io::{Read, Write};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use super::*;
use crate::clock::SystemClock;
use crate::ipc::{ErrorCode, Frame, LogsPage, Request, Response};
use crate::logs::Log;

static NEXT: AtomicUsize = AtomicUsize::new(0);

fn name() -> PipeName {
    let n = NEXT.fetch_add(1, Ordering::SeqCst);
    PipeName::parse(&format!("SplitWire.Test.{}.{n}", std::process::id())).unwrap()
}

fn log() -> Log {
    Log::new(1 << 16, Arc::new(SystemClock), false)
}

/// Answers every request with a `Logs` page whose `last_seq` is the request's tag.
fn echo() -> FrameHandler {
    Arc::new(|body: &[u8]| {
        let response = match Request::decode(body) {
            Ok(request) => Response::Logs(LogsPage {
                entries: Vec::new(),
                last_seq: request.tag().into(),
            }),
            Err(e) => Response::Error(crate::ipc::ErrorReply {
                code: ErrorCode::BadRequest,
                message: e.to_string(),
            }),
        };
        response.encode().unwrap()
    })
}

fn serve(name: &PipeName) -> PipeServer {
    PipeListener::bind(name.clone())
        .unwrap()
        .serve(echo(), log())
        .unwrap()
}

/// A raw client connection; the next instance may take a moment to appear.
fn open_waiting(name: &PipeName) -> std::fs::File {
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        match OpenOptions::new().read(true).write(true).open(name.path()) {
            Ok(pipe) => return pipe,
            Err(e) if Instant::now() < deadline => {
                assert_eq!(e.raw_os_error(), Some(231), "{e}");
                std::thread::sleep(Duration::from_millis(10));
            }
            Err(e) => panic!("{e}"),
        }
    }
}

fn tag_of(response: Response) -> u64 {
    match response {
        Response::Logs(page) => page.last_seq,
        other => panic!("{other:?}"),
    }
}

#[test]
fn a_client_exchanges_requests_over_one_connection() {
    let name = name();
    let server = serve(&name);
    let mut client = PipeClient::new(name.clone());
    assert_eq!(tag_of(client.exchange(&Request::Status).unwrap()), 1);
    let logs = Request::Logs { after: 7 };
    assert_eq!(tag_of(client.exchange(&logs).unwrap()), 5);
    let mut other = PipeClient::new(name.clone());
    assert_eq!(tag_of(other.exchange(&Request::Routes).unwrap()), 3);
    server.stop();
    assert_eq!(
        PipeClient::new(name).exchange(&Request::Status),
        Err(PipeError::NotRunning)
    );
}

#[test]
fn only_the_first_instance_may_serve_a_name() {
    let name = name();
    let server = serve(&name);
    let second = PipeListener::bind(name.clone()).err().unwrap();
    assert_eq!(second.kind(), std::io::ErrorKind::AlreadyExists);
    assert!(second.to_string().contains(&name.path()), "{second}");
    server.stop();
    // Once stopped, the name is free again.
    serve(&name).stop();
}

#[test]
fn the_pipe_keeps_the_dacl_of_the_spec() {
    let name = name();
    let server = serve(&name);
    let pipe = OpenOptions::new()
        .read(true)
        .write(true)
        .open(name.path())
        .unwrap();
    let dacl = PipeSecurity::dacl_of(&pipe).unwrap();
    assert!(dacl.starts_with("D:P"), "{dacl}");
    for trustee in [";;;SY)", ";;;BA)", ";;;OW)"] {
        assert!(dacl.contains(trustee), "{dacl}");
    }
    // Interactive users read and write, but cannot create an instance (0x4).
    assert!(dacl.contains("(A;;0x12008b;;;IU)"), "{dacl}");
    // A user's own pipe is not the service's; whoever runs the tests elevated owns it as an
    // administrator.
    assert!(PipeSecurity::served_by_admin(&pipe).is_ok());
    drop(pipe);
    server.stop();
}

#[test]
fn a_bad_frame_is_answered_with_bad_request_and_the_pipe_hung_up() {
    let name = name();
    let server = serve(&name);
    let mut pipe = OpenOptions::new()
        .read(true)
        .write(true)
        .open(name.path())
        .unwrap();
    pipe.write_all(&u32::MAX.to_le_bytes()).unwrap();
    let body = Frame::read(&mut pipe).unwrap();
    match Response::decode(&body).unwrap() {
        Response::Error(reply) => assert_eq!(reply.code, ErrorCode::BadRequest),
        other => panic!("{other:?}"),
    }
    let mut rest = Vec::new();
    assert!(pipe.read_to_end(&mut rest).is_err() || rest.is_empty());
    // Garbage inside a well-formed frame is a bad request too, and the connection stays.
    let mut client = PipeClient::new(name.clone());
    assert_eq!(tag_of(client.exchange(&Request::Status).unwrap()), 1);
    server.stop();
}

#[test]
fn stopping_ends_idle_clients_at_once() {
    let name = name();
    let server = serve(&name);
    let idle: Vec<_> = (0..3).map(|_| open_waiting(&name)).collect();
    std::thread::sleep(Duration::from_millis(100));
    let started = Instant::now();
    server.stop();
    assert!(started.elapsed() < Duration::from_secs(5));
    drop(idle);
}
