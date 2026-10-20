use std::net::SocketAddrV4;
use std::path::PathBuf;
use std::time::Duration;

use super::DurationArg;
use crate::config::SettingValue;
use crate::platform::pipe::PipeName;

/// `run [--dry-run] [--for <d>] [--dir <p>] [--listen <a>] [--pipe <n>] [--connect <name>]…`: the
/// engine in the foreground until Ctrl+C or the `--for` time is up, serving the pipe.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RunArgs {
    pub dry_run: bool,
    pub limit: Option<Duration>,
    pub dir: Option<PathBuf>,
    pub listen: Option<SocketAddrV4>,
    /// `None`: `SplitWire`, or `SplitWire.DryRun` for a dry run.
    pub pipe: Option<PipeName>,
    pub connect: Vec<String>,
}

impl RunArgs {
    /// The arguments after `run`.
    pub fn parse(args: &[String]) -> Result<Self, String> {
        let mut parsed = Self::default();
        let mut listen = None;
        let mut args = args.iter();
        while let Some(arg) = args.next() {
            let mut value = || {
                args.next()
                    .cloned()
                    .ok_or_else(|| format!("{arg} needs a value"))
            };
            match arg.as_str() {
                "--dry-run" => parsed.dry_run = true,
                "--for" => parsed.limit = Some(DurationArg::parse(&value()?)?),
                "--dir" => parsed.dir = Some(PathBuf::from(value()?)),
                "--listen" => listen = Some(value()?),
                "--pipe" => parsed.pipe = Some(PipeName::parse(&value()?)?),
                "--connect" => parsed.connect.push(value()?),
                other => return Err(format!("unknown option '{other}'")),
            }
        }
        if let Some(text) = listen {
            let address = SettingValue::listen(&text, parsed.dry_run)
                .map_err(|reason| format!("--listen {text}: {reason}"))?;
            parsed.listen = Some(address);
        }
        Ok(parsed)
    }

    /// The pipe this run serves.
    pub fn pipe_name(&self) -> PipeName {
        match (&self.pipe, self.dry_run) {
            (Some(pipe), _) => pipe.clone(),
            (None, true) => PipeName::dry_run(),
            (None, false) => PipeName::service(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(line: &str) -> Result<RunArgs, String> {
        let args: Vec<String> = line.split_whitespace().map(String::from).collect();
        RunArgs::parse(&args)
    }

    #[test]
    fn parses_every_option() {
        let args = parse(
            "--dry-run --for 30s --dir C:/tmp/x --listen 127.0.0.1:15353 --pipe Mine \
             --connect A --connect B",
        )
        .unwrap();
        assert_eq!(
            args,
            RunArgs {
                dry_run: true,
                limit: Some(Duration::from_secs(30)),
                dir: Some(PathBuf::from("C:/tmp/x")),
                listen: Some("127.0.0.1:15353".parse().unwrap()),
                pipe: Some(PipeName::parse("Mine").unwrap()),
                connect: vec!["A".into(), "B".into()],
            }
        );
        assert_eq!(parse("").unwrap(), RunArgs::default());
    }

    #[test]
    fn a_dry_run_serves_its_own_pipe() {
        assert_eq!(parse("--dry-run").unwrap().pipe_name(), PipeName::dry_run());
        assert_eq!(parse("").unwrap().pipe_name(), PipeName::service());
        assert_eq!(
            parse("--dry-run --pipe Other").unwrap().pipe_name(),
            PipeName::parse("Other").unwrap()
        );
    }

    #[test]
    fn rejects_bad_input() {
        assert!(parse("--for").is_err());
        assert!(parse("--for soon").is_err());
        assert!(parse("--dry-run --listen localhost").is_err());
        assert!(parse("--verbose").is_err());
        assert!(parse("--pipe a/b").is_err());
        // Port 53 only for a live run.
        assert!(parse("--listen 127.0.0.1:15353").is_err());
        assert!(parse("--listen 127.0.0.53:53").is_ok());
    }
}
