use std::io;
use std::net::{SocketAddr, SocketAddrV4, TcpListener, UdpSocket};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::JoinHandle;

use super::{DnsService, TcpServer, UdpServer};
use crate::logs::Log;

/// The DNS proxy on `listen` (loopback only), UDP and TCP. It runs while at least one profile
/// is connected: started by the first Connect, stopped after the last Disconnect.
pub struct DnsProxy {
    address: SocketAddr,
    stop: Arc<AtomicBool>,
    threads: Vec<JoinHandle<()>>,
    log: Log,
}

impl DnsProxy {
    /// Binds both sockets first, so a busy port fails before any thread starts.
    pub fn start(listen: SocketAddrV4, service: Arc<DnsService>, log: Log) -> io::Result<Self> {
        if !listen.ip().is_loopback() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("{listen}: the proxy binds loopback only"),
            ));
        }
        let udp = UdpSocket::bind(listen)
            .map_err(|e| io::Error::new(e.kind(), format!("DNS UDP {listen}: {e}")))?;
        let tcp = TcpListener::bind(listen)
            .map_err(|e| io::Error::new(e.kind(), format!("DNS TCP {listen}: {e}")))?;
        let address = udp.local_addr()?;
        let log = log.with_source("dns");
        let stop = Arc::new(AtomicBool::new(false));
        let mut threads = UdpServer::spawn(udp, service.clone(), stop.clone(), log.clone())?;
        threads.push(TcpServer::spawn(tcp, service, stop.clone(), log.clone())?);
        log.info(format!("the proxy listens on {address} (UDP and TCP)"));
        Ok(Self {
            address,
            stop,
            threads,
            log,
        })
    }

    pub fn address(&self) -> SocketAddr {
        self.address
    }

    /// False when a thread ended without being stopped (the NRPT watchdog acts on it).
    pub fn alive(&self) -> bool {
        self.threads.iter().all(|thread| !thread.is_finished())
    }

    /// Stops the threads and closes the sockets.
    pub fn stop(self) {
        self.stop.store(true, Ordering::SeqCst);
        for thread in self.threads {
            let _ = thread.join();
        }
        self.log
            .info(format!("the proxy on {} stopped", self.address));
    }
}
