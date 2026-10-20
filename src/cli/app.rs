use std::io;
use std::time::SystemTime;

use super::text::Times;
use super::{AdminCommands, Command, Console, ExitCode, PipeCommands, RunArgs, ServiceCommand};
use crate::application::{DataFolder, EngineConfig, EngineHost, HostConfig, StopSignal};
use crate::platform::console::ConsoleInterrupt;
use crate::platform::elevation::Elevation;
use crate::platform::library::AppLibrary;
use crate::platform::local_clock::LocalClock;
use crate::platform::pipe::PipeClient;

/// `SplitWire.exe <command>`: arguments to an action to an exit code.
pub struct App;

impl App {
    pub fn run(args: &[String]) -> i32 {
        // No later implicit DLL load may look in the current or PATH folders.
        let _ = AppLibrary::restrict_default_search();
        match Command::parse(args) {
            Ok(Command::Help) => {
                println!("{}", Command::USAGE);
                ExitCode::OK
            }
            Ok(Command::Run(run)) => Self::run_engine(run),
            Ok(Command::Pipe { pipe, action }) => {
                let now = SystemTime::now();
                let times = Times {
                    now,
                    offset_secs: LocalClock::offset_secs(now),
                };
                let (mut out, mut err, mut input) = (io::stdout(), io::stderr(), io::stdin());
                let console = Console {
                    out: &mut out,
                    err: &mut err,
                    input: &mut input,
                };
                PipeCommands::new(PipeClient::new(pipe), console, times).run(action)
            }
            Ok(Command::Service) => ServiceCommand::run(),
            Ok(Command::Install { restore }) => AdminCommands::install(restore),
            Ok(Command::Uninstall { purge }) => AdminCommands::uninstall(purge),
            Ok(Command::Cleanup) => AdminCommands::cleanup(),
            Err(problem) => {
                eprintln!("SplitWire: {problem}\n{}", Command::USAGE);
                ExitCode::USAGE
            }
        }
    }

    fn run_engine(run: RunArgs) -> i32 {
        if !run.dry_run && !Elevation::is_elevated().unwrap_or(false) {
            eprintln!("SplitWire: run needs an administrator console; --dry-run does not");
            return ExitCode::ADMIN_REQUIRED;
        }
        let stop = StopSignal::default();
        let interrupt = stop.clone();
        if let Err(e) = ConsoleInterrupt::on_interrupt(move || interrupt.stop("Ctrl+C")) {
            eprintln!("SplitWire: {e}");
            return ExitCode::ERROR;
        }
        let config = HostConfig {
            pipe: run.pipe_name(),
            engine: EngineConfig {
                folder: DataFolder::new(run.dir.unwrap_or_else(DataFolder::default_root)),
                listen: run.listen,
                connect: run.connect,
                restore_all: false,
            },
            dry_run: run.dry_run,
            limit: run.limit,
            echo: true,
        };
        match EngineHost::run(config, &stop, |_| {}) {
            Ok(()) => ExitCode::OK,
            Err(e) => {
                eprintln!("SplitWire: {e}");
                ExitCode::ERROR
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(line: &str) -> i32 {
        let args: Vec<String> = line.split_whitespace().map(String::from).collect();
        App::run(&args)
    }

    #[test]
    fn usage_errors_exit_with_2() {
        assert_eq!(run(""), ExitCode::USAGE);
        assert_eq!(run("start"), ExitCode::USAGE);
        assert_eq!(run("run --for soon"), ExitCode::USAGE);
        assert_eq!(run("connect"), ExitCode::USAGE);
        assert_eq!(run("help"), ExitCode::OK);
    }

    #[test]
    fn a_pipe_command_without_an_engine_exits_with_3() {
        let pipe = format!("SplitWire.Test.{}.absent", std::process::id());
        assert_eq!(run(&format!("list --pipe {pipe}")), ExitCode::NOT_REACHABLE);
    }
}
