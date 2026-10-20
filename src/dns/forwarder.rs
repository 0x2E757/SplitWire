use std::io;
use std::net::SocketAddr;
use std::sync::{Arc, RwLock};

use super::DnsExchange;

/// A list of DNS servers that can be replaced while in use (the fallback follows the network).
#[derive(Default)]
pub struct ServerSet {
    servers: RwLock<Vec<SocketAddr>>,
}

impl ServerSet {
    pub fn new(servers: Vec<SocketAddr>) -> Self {
        Self {
            servers: RwLock::new(servers),
        }
    }

    pub fn get(&self) -> Vec<SocketAddr> {
        self.servers.read().unwrap().clone()
    }

    pub fn replace(&self, servers: Vec<SocketAddr>) {
        *self.servers.write().unwrap() = servers;
    }
}

/// Forwards a query to the servers of a set in order; the first reply wins.
#[derive(Clone)]
pub struct Forwarder {
    exchange: Arc<dyn DnsExchange>,
    servers: Arc<ServerSet>,
}

impl Forwarder {
    pub fn new(exchange: Arc<dyn DnsExchange>, servers: Arc<ServerSet>) -> Self {
        Self { exchange, servers }
    }

    pub fn forward(&self, query: &[u8]) -> io::Result<Vec<u8>> {
        let mut last = io::Error::other("no DNS servers configured");
        for server in self.servers.get() {
            match self.exchange.exchange(server, query) {
                Ok(reply) => return Ok(reply),
                Err(e) => last = io::Error::new(e.kind(), format!("{server}: {e}")),
            }
        }
        Err(last)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    /// Answers from the servers it knows, times out for the rest, and records every call.
    struct FakeExchange {
        answering: Vec<SocketAddr>,
        calls: Mutex<Vec<SocketAddr>>,
    }

    impl DnsExchange for FakeExchange {
        fn exchange(&self, server: SocketAddr, query: &[u8]) -> io::Result<Vec<u8>> {
            self.calls.lock().unwrap().push(server);
            if self.answering.contains(&server) {
                Ok(query.to_vec())
            } else {
                Err(io::ErrorKind::TimedOut.into())
            }
        }
    }

    fn addr(text: &str) -> SocketAddr {
        text.parse().unwrap()
    }

    fn forwarder(answering: &[&str], servers: &[&str]) -> (Arc<FakeExchange>, Forwarder) {
        let exchange = Arc::new(FakeExchange {
            answering: answering.iter().map(|s| addr(s)).collect(),
            calls: Mutex::new(Vec::new()),
        });
        let set = ServerSet::new(servers.iter().map(|s| addr(s)).collect());
        let forwarder = Forwarder::new(exchange.clone(), Arc::new(set));
        (exchange, forwarder)
    }

    #[test]
    fn the_first_server_that_answers_wins() {
        let (exchange, forwarder) = forwarder(&["8.8.8.8:53"], &["1.1.1.1:53", "8.8.8.8:53"]);
        assert_eq!(forwarder.forward(b"q").unwrap(), b"q");
        assert_eq!(
            *exchange.calls.lock().unwrap(),
            [addr("1.1.1.1:53"), addr("8.8.8.8:53")]
        );
    }

    #[test]
    fn the_last_failure_names_its_server() {
        let (_, forwarder) = forwarder(&[], &["1.1.1.1:53", "9.9.9.9:53"]);
        let error = forwarder.forward(b"q").unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::TimedOut);
        assert!(error.to_string().starts_with("9.9.9.9:53: "));
    }

    #[test]
    fn an_empty_set_fails_and_a_replaced_set_is_used_at_once() {
        let (_, forwarder) = forwarder(&["8.8.8.8:53"], &[]);
        assert!(
            forwarder
                .forward(b"q")
                .unwrap_err()
                .to_string()
                .contains("no DNS servers")
        );
        forwarder.servers.replace(vec![addr("8.8.8.8:53")]);
        assert!(forwarder.forward(b"q").is_ok());
    }
}
