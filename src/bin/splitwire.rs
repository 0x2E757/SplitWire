//! `SplitWire.exe`: the service and the command line. So far the command line runs the engine in
//! the foreground (`run`, `run --dry-run`); the service and the pipe commands come next.

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    std::process::exit(splitwire::cli::App::run(&args));
}
