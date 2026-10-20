//! The window's one worker thread for the engine: requests go to the transport in
//! order, answers come back to the window's frame loop, which never waits on the pipe.

use std::sync::mpsc::{self, Receiver, Sender};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use crate::ipc::{Request, Response};
use crate::presentation::transport::{EngineTransport, TransportError};

type Answer = (u64, Result<Response, TransportError>);

pub struct EngineLink {
    requests: Option<Sender<(u64, Request)>>,
    answers: Receiver<Answer>,
    worker: Option<JoinHandle<()>>,
    /// Requests sent and not answered yet.
    in_flight: usize,
}

impl EngineLink {
    pub fn start(mut transport: Box<dyn EngineTransport>) -> Self {
        let (request_tx, request_rx) = mpsc::channel::<(u64, Request)>();
        let (answer_tx, answer_rx) = mpsc::channel::<Answer>();
        let worker = std::thread::Builder::new()
            .name("engine-link".into())
            .spawn(move || {
                for (id, request) in request_rx {
                    let answer = transport.exchange(&request);
                    if answer_tx.send((id, answer)).is_err() {
                        break;
                    }
                }
            })
            .ok();
        Self {
            requests: Some(request_tx),
            answers: answer_rx,
            worker,
            in_flight: 0,
        }
    }

    pub fn send(&mut self, id: u64, request: Request) {
        if let Some(requests) = &self.requests
            && requests.send((id, request)).is_ok()
        {
            self.in_flight += 1;
        }
    }

    /// The answers that arrived since the last call.
    pub fn answers(&mut self) -> Vec<Answer> {
        let answers: Vec<Answer> = self.answers.try_iter().collect();
        self.in_flight = self.in_flight.saturating_sub(answers.len());
        answers
    }

    /// Waits until every request sent is answered, for at most `limit` (the tray's menu waits
    /// for a fresh status this way); the answers that came.
    pub fn settle(&mut self, limit: Duration) -> Vec<Answer> {
        let deadline = Instant::now() + limit;
        let mut answers = Vec::new();
        while self.in_flight > 0 {
            let left = deadline.saturating_duration_since(Instant::now());
            match self.answers.recv_timeout(left) {
                Ok(answer) => {
                    self.in_flight -= 1;
                    answers.push(answer);
                }
                Err(_) => break,
            }
        }
        answers
    }

    /// Waits for `count` answers.
    #[cfg(test)]
    pub fn wait(&self, count: usize) -> Vec<Answer> {
        (0..count)
            .filter_map(|_| self.answers.recv().ok())
            .collect()
    }
}

impl Drop for EngineLink {
    fn drop(&mut self) {
        self.requests = None;
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::presentation::transport::demo::{DemoTransport, Scenario};

    #[test]
    fn answers_come_back_in_order() {
        let mut link = EngineLink::start(Box::new(DemoTransport::new(Scenario::Normal)));
        link.send(1, Request::Status);
        link.send(2, Request::Routes);
        let answers = link.wait(2);
        assert_eq!(answers[0].0, 1);
        assert!(matches!(answers[1].1, Ok(Response::Routes(_))));
    }

    #[test]
    fn settling_waits_for_every_answer_and_no_longer() {
        let mut link = EngineLink::start(Box::new(DemoTransport::new(Scenario::Normal)));
        link.send(1, Request::Status);
        link.send(2, Request::AlertHistory);
        let answers = link.settle(Duration::from_secs(5));
        assert_eq!(answers.iter().map(|a| a.0).collect::<Vec<_>>(), [1, 2]);
        let start = Instant::now();
        assert!(link.settle(Duration::from_secs(5)).is_empty());
        assert!(
            start.elapsed() < Duration::from_secs(1),
            "nothing was in flight"
        );
    }
}
