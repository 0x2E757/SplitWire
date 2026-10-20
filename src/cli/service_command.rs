use std::sync::{Arc, Mutex};

use super::ExitCode;
use crate::application::{
    DataFolder, EngineConfig, EngineHost, HostConfig, SchedulerWaker, StopSignal,
};
use crate::platform::pipe::PipeName;
use crate::platform::service::{ControlHandler, Dispatcher, ServiceEvents};

/// `SplitWire.exe service`: the SCM's entry. The engine runs on `%ProgramData%\SplitWire` and
/// serves `\\.\pipe\SplitWire` until the SCM stops it; a resume from sleep wakes the periodic
/// jobs. Started any other way, it fails at once.
pub struct ServiceCommand;

impl ServiceCommand {
    pub fn run() -> i32 {
        match Dispatcher::run(|args| Self::serve(&args)) {
            Ok(()) => ExitCode::OK,
            Err(e) => {
                eprintln!("SplitWire: 'service' is for the service control manager ({e})");
                ExitCode::ERROR
            }
        }
    }

    fn serve(args: &[String]) {
        let stop = StopSignal::default();
        let waker: Arc<Mutex<Option<SchedulerWaker>>> = Arc::default();
        let events = ServiceEvents {
            on_stop: {
                let stop = stop.clone();
                Box::new(move || stop.stop("the service control manager asked to stop"))
            },
            on_resume: {
                let waker = waker.clone();
                Box::new(move || {
                    if let Some(waker) = waker.lock().unwrap().as_ref() {
                        waker.wake();
                    }
                })
            },
        };
        let Ok(handler) = ControlHandler::register(events) else {
            return;
        };
        let _ = handler.starting();
        let config = HostConfig {
            engine: EngineConfig {
                folder: DataFolder::new(DataFolder::default_root()),
                listen: None,
                connect: Vec::new(),
                restore_all: args.iter().any(|a| a == "--restore"),
            },
            dry_run: false,
            limit: None,
            pipe: PipeName::service(),
            echo: false,
        };
        let result = EngineHost::run(config, &stop, |started| {
            *waker.lock().unwrap() = Some(started);
            let _ = handler.running();
        });
        let _ = handler.stopping();
        let _ = handler.stopped(result.err().map(|_| ExitCode::ERROR as u32));
    }
}
