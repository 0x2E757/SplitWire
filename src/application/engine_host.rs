use std::io;
use std::sync::Arc;
use std::time::Duration;

use super::{Control, Engine, EngineConfig, SchedulerWaker, SettingsFiles, StopSignal};
use crate::clock::Clock;
use crate::config::Settings;
use crate::logs::{Log, LogFile};
use crate::platform::composition::PlatformSet;
use crate::platform::engine_mutex::EngineMutex;
use crate::platform::local_clock::LocalClock;
use crate::platform::pipe::{PipeListener, PipeName};

/// How a host runs its engine: a foreground `run` or the service.
pub struct HostConfig {
    pub engine: EngineConfig,
    pub dry_run: bool,
    /// `--for`: stop after this long.
    pub limit: Option<Duration>,
    /// The pipe it serves: `SplitWire`, `SplitWire.DryRun` or `--pipe`.
    pub pipe: PipeName,
    /// Echo the log to the console (a foreground run).
    pub echo: bool,
}

/// Runs an engine from start to clean stop and serves the pipe meanwhile: Ctrl+C or the SCM
/// (through `stop`) or the `--for` time ends it. The pipe's first instance is taken before
/// anything else, so a second engine stops at once; the pipe closes before the engine stops, so
/// no request reaches a stopping engine. A failed start is also written to
/// `logs\splitwire.error.log`.
pub struct EngineHost;

impl EngineHost {
    /// `started` gets the scheduler's waker once the engine runs (resume from sleep).
    pub fn run(
        config: HostConfig,
        stop: &StopSignal,
        started: impl FnOnce(SchedulerWaker),
    ) -> io::Result<()> {
        let logs = config.engine.folder.logs();
        let result = Self::serve(config, stop, started);
        if let Err(e) = &result {
            let _ = LogFile::write_start_error(&logs, &format!("SplitWire could not run: {e}"));
        }
        result
    }

    fn serve(
        mut config: HostConfig,
        stop: &StopSignal,
        started: impl FnOnce(SchedulerWaker),
    ) -> io::Result<()> {
        let clock: Arc<dyn Clock> = Arc::new(LocalClock);
        let log = Log::new(Log::DEFAULT_BYTES, clock.clone(), config.echo);
        let listen = match config.engine.listen {
            Some(listen) => listen,
            None => SettingsFiles::new(config.engine.folder.clone(), config.dry_run)
                .global()?
                .map_or(Settings::default().listen, |s| s.listen),
        };
        config.engine.listen = Some(listen);
        let _single = if config.dry_run {
            None
        } else {
            Some(EngineMutex::acquire()?)
        };
        let listener = PipeListener::bind(config.pipe.clone())?;
        let platform = if config.dry_run {
            PlatformSet::dry_run(&log, clock.clone())
        } else {
            PlatformSet::live(listen, &log)?
        };
        let since = clock.now();
        let engine = Engine::start(config.engine, platform, clock, log.clone())?;
        let control = Control::new(engine.coordinator().clone(), since, log.clone());
        let pipe = match listener.serve(Arc::new(move |body| control.serve(body)), log.clone()) {
            Ok(pipe) => pipe,
            Err(e) => {
                engine.stop();
                return Err(e);
            }
        };
        started(engine.waker());
        let reason = stop.wait(config.limit);
        log.info(format!("{reason}: stopping"));
        pipe.stop();
        engine.stop();
        Ok(())
    }
}
