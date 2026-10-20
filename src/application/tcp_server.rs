use std::io::{self, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use super::DnsService;
use crate::logs::Log;

/// The proxy over TCP (RFC 1035 length-prefixed messages), for answers too big for UDP: one
/// thread per connection, at most 16.
pub struct TcpServer;

impl TcpServer {
    pub const MAX_CONNECTIONS: usize = 16;
    const IDLE: Duration = Duration::from_secs(10);
    const POLL: Duration = Duration::from_millis(50);

    /// Starts the accepting thread; it ends when `stop` is set.
    pub fn spawn(
        listener: TcpListener,
        service: Arc<DnsService>,
        stop: Arc<AtomicBool>,
        log: Log,
    ) -> io::Result<JoinHandle<()>> {
        listener.set_nonblocking(true)?;
        thread::Builder::new()
            .name("dns-tcp".into())
            .spawn(move || Self::accept(&listener, &service, &stop, &log))
    }

    fn accept(listener: &TcpListener, service: &Arc<DnsService>, stop: &AtomicBool, log: &Log) {
        let open = Arc::new(AtomicUsize::new(0));
        while !stop.load(Ordering::SeqCst) {
            match listener.accept() {
                Ok((stream, _)) if open.load(Ordering::SeqCst) < Self::MAX_CONNECTIONS => {
                    open.fetch_add(1, Ordering::SeqCst);
                    let (service, open) = (service.clone(), open.clone());
                    let spawned =
                        thread::Builder::new()
                            .name("dns-tcp-conn".into())
                            .spawn(move || {
                                let _ = Self::serve(stream, &service);
                                open.fetch_sub(1, Ordering::SeqCst);
                            });
                    if spawned.is_err() {
                        log.error("DNS TCP: no thread for a connection");
                    }
                }
                // Too many connections: this one is closed at once.
                Ok(_) => {}
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => thread::sleep(Self::POLL),
                Err(e) => {
                    log.error(format!("DNS TCP accept failed: {e}"));
                    thread::sleep(Self::POLL);
                }
            }
        }
    }

    fn serve(mut stream: TcpStream, service: &DnsService) -> io::Result<()> {
        stream.set_nonblocking(false)?;
        stream.set_read_timeout(Some(Self::IDLE))?;
        loop {
            let mut len = [0u8; 2];
            stream.read_exact(&mut len)?;
            let mut query = vec![0u8; usize::from(u16::from_be_bytes(len))];
            stream.read_exact(&mut query)?;
            let Some(outcome) = service.answer(&query) else {
                return Ok(());
            };
            let len = u16::try_from(outcome.response.len()).map_err(io::Error::other)?;
            let mut framed = len.to_be_bytes().to_vec();
            framed.extend_from_slice(&outcome.response);
            stream.write_all(&framed)?;
        }
    }
}
