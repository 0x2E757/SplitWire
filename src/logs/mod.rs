//! The in-memory log: a ring buffer bounded by size, and the handle every component writes through;
//! warnings and errors also go to `logs\splitwire.log`.

mod log;
mod log_file;
mod log_line_text;
mod ring_buffer;

pub use log::Log;
pub use log_file::LogFile;
pub use log_line_text::{LogFileLine, LogLineText};
pub use ring_buffer::{Level, LogBuffer, LogEntry};
