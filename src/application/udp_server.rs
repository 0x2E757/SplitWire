use std::io;
use std::net::{SocketAddr, UdpSocket};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use super::DnsService;
use crate::logs::Log;

type Datagram = (Vec<u8>, SocketAddr);

/// The proxy over UDP: one thread receives, four workers answer, each upstream exchange
/// blocking for up to 2 s.
pub struct UdpServer;

impl UdpServer {
    pub const WORKERS: usize = 4;
    /// How often a blocked receive looks at the stop flag.
    const POLL: Duration = Duration::from_millis(200);

    /// Starts the threads; they end when `stop` is set.
    pub fn spawn(
        socket: UdpSocket,
        service: Arc<DnsService>,
        stop: Arc<AtomicBool>,
        log: Log,
    ) -> io::Result<Vec<JoinHandle<()>>> {
        socket.set_read_timeout(Some(Self::POLL))?;
        let socket = Arc::new(socket);
        let (sender, receiver) = mpsc::channel::<Datagram>();
        let receiver = Arc::new(Mutex::new(receiver));
        let mut threads = Vec::with_capacity(Self::WORKERS + 1);
        for worker in 0..Self::WORKERS {
            let (socket, service, receiver) = (socket.clone(), service.clone(), receiver.clone());
            threads.push(
                thread::Builder::new()
                    .name(format!("dns-udp-{worker}"))
                    .spawn(move || Self::work(&socket, &service, &receiver))?,
            );
        }
        threads.push(
            thread::Builder::new()
                .name("dns-udp".into())
                .spawn(move || Self::receive(&socket, &sender, &stop, &log))?,
        );
        Ok(threads)
    }

    /// Dropping the sender at the end lets the workers finish.
    fn receive(socket: &UdpSocket, sender: &Sender<Datagram>, stop: &AtomicBool, log: &Log) {
        let mut buffer = vec![0u8; 65_535];
        while !stop.load(Ordering::SeqCst) {
            match socket.recv_from(&mut buffer) {
                Ok((len, peer)) => {
                    let _ = sender.send((buffer[..len].to_vec(), peer));
                }
                Err(e)
                    if matches!(
                        e.kind(),
                        io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
                    ) => {}
                // Windows reports an ICMP "port unreachable" for an earlier reply this way.
                Err(e) if e.kind() == io::ErrorKind::ConnectionReset => {}
                Err(e) => {
                    log.error(format!("DNS UDP receive failed: {e}"));
                    thread::sleep(Self::POLL);
                }
            }
        }
    }

    fn work(socket: &UdpSocket, service: &DnsService, receiver: &Mutex<Receiver<Datagram>>) {
        loop {
            let next = receiver.lock().unwrap().recv();
            let Ok((query, peer)) = next else {
                return;
            };
            if let Some(outcome) = service.answer(&query) {
                let _ = socket.send_to(&outcome.udp_payload(), peer);
            }
        }
    }
}
