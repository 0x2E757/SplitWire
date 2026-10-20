//! `SplitWire.Window.exe`: the tray icon and the window.
#![windows_subsystem = "windows"]

use splitwire::platform::window::{Arguments, WindowProgram};

fn main() {
    let arguments = Arguments::parse(std::env::args().skip(1));
    std::process::exit(WindowProgram::run(arguments));
}
