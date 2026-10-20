//! The command line of `SplitWire.exe`: `run` (the engine in the foreground), the pipe
//! commands that talk to a running engine (`list`, `status`, `import`, `connect`, …), the
//! administrator's `install`, `uninstall` and `cleanup`, and `service`, the SCM's entry.

mod admin_commands;
mod app;
mod command;
mod duration_arg;
mod exit_code;
mod pipe_commands;
mod run_args;
mod service_command;
mod text;

#[cfg(test)]
mod tests;

pub use admin_commands::{AdminCommands, InstallLocation};
pub use app::App;
pub use command::{Command, PipeAction};
pub use duration_arg::DurationArg;
pub use exit_code::ExitCode;
pub use pipe_commands::{Console, EngineLink, PipeCommands};
pub use run_args::RunArgs;
pub use service_command::ServiceCommand;
pub use text::{PreviewText, ReplyText, StatusText, Times};
