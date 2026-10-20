use std::path::PathBuf;

use super::RunArgs;
use crate::ipc::ProfileTarget;
use crate::platform::pipe::PipeName;

/// What a pipe command asks the engine.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PipeAction {
    List,
    Status {
        name: Option<String>,
    },
    Import {
        files: Vec<PathBuf>,
        name: Option<String>,
        preview: bool,
        replace: bool,
    },
    Connect {
        name: String,
    },
    Disconnect {
        target: ProfileTarget,
    },
    Remove {
        name: String,
    },
    /// `--save` reads the new combined text from stdin.
    Lists {
        save: bool,
    },
    Alerts {
        dismiss: Option<String>,
        restore: bool,
    },
}

/// One command line of `SplitWire.exe`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Command {
    Help,
    Service,
    Install { restore: bool },
    Uninstall { purge: bool },
    Cleanup,
    Run(RunArgs),
    Pipe { pipe: PipeName, action: PipeAction },
}

impl Command {
    pub const USAGE: &'static str = "\
usage: SplitWire.exe <command> [options]
  run [--dry-run] [--for <duration>] [--dir <path>] [--listen <ip:port>] [--pipe <name>]
      [--connect <name>]...      the engine in the foreground until Ctrl+C; --for stops it
                                 after <duration> (a time limit for tests)
  list                           the profiles in order, with state and handshake
  status [<name>]                the engine's status, or one profile's with its last attempt
  import <file>... [--name <n>] [--preview] [--replace]
                                 import .conf files (the name defaults to the file name)
  connect <name>                 connect until disconnected
  disconnect <name> | --all
  remove <name>                  disconnect and delete a profile
  lists [--save]                 print every list; --save reads a new text from stdin
  alerts [--dismiss <id> | --restore]
  install [--restore] | uninstall [--purge] | cleanup | service      (administrator)
  help
The pipe commands talk to \\\\.\\pipe\\SplitWire; --pipe SplitWire.DryRun talks to a dry run.
A dry run (--dry-run) changes nothing on the machine and needs no administrator; its --listen
takes any loopback port, such as 127.0.0.1:15353.
Exit codes: 0 ok, 1 error, 2 usage, 3 engine not reachable, 4 administrator required,
5 refused, 6 unknown profile.";

    pub fn parse(args: &[String]) -> Result<Self, String> {
        let Some((command, rest)) = args.split_first() else {
            return Err("no command".into());
        };
        match command.as_str() {
            "help" | "--help" | "-h" | "/?" => Ok(Self::Help),
            "service" => Self::no_options(rest).map(|()| Self::Service),
            "install" => {
                Self::flag_only(rest, "--restore").map(|restore| Self::Install { restore })
            }
            "uninstall" => Self::flag_only(rest, "--purge").map(|purge| Self::Uninstall { purge }),
            "cleanup" => Self::no_options(rest).map(|()| Self::Cleanup),
            "run" => RunArgs::parse(rest).map(Self::Run),
            other => Self::pipe_command(other, rest),
        }
    }

    fn pipe_command(command: &str, rest: &[String]) -> Result<Self, String> {
        let mut options = Options::split(rest)?;
        let action = match command {
            "list" => PipeAction::List,
            "status" => PipeAction::Status {
                name: options.at_most_one("a profile name")?,
            },
            "import" => {
                let files: Vec<PathBuf> =
                    options.take_words().into_iter().map(Into::into).collect();
                if files.is_empty() {
                    return Err("import needs at least one .conf file".into());
                }
                let name = options.value("--name")?;
                if name.is_some() && files.len() > 1 {
                    return Err("--name names one file only".into());
                }
                PipeAction::Import {
                    files,
                    name,
                    preview: options.flag("--preview"),
                    replace: options.flag("--replace"),
                }
            }
            "connect" => PipeAction::Connect {
                name: options.exactly_one("a profile name")?,
            },
            "disconnect" => {
                let all = options.flag("--all");
                let name = options.at_most_one("a profile name")?;
                let target = match (all, name) {
                    (true, None) => ProfileTarget::All,
                    (false, Some(name)) => ProfileTarget::One(name),
                    _ => return Err("disconnect takes a profile name or --all".into()),
                };
                PipeAction::Disconnect { target }
            }
            "remove" => PipeAction::Remove {
                name: options.exactly_one("a profile name")?,
            },
            "lists" => PipeAction::Lists {
                save: options.flag("--save"),
            },
            "alerts" => {
                let dismiss = options.value("--dismiss")?;
                let restore = options.flag("--restore");
                if dismiss.is_some() && restore {
                    return Err("alerts takes --dismiss <id> or --restore, not both".into());
                }
                PipeAction::Alerts { dismiss, restore }
            }
            other => return Err(format!("unknown command '{other}'")),
        };
        let pipe = options.value("--pipe")?;
        options.finish()?;
        let pipe = match pipe {
            Some(name) => PipeName::parse(&name)?,
            None => PipeName::service(),
        };
        Ok(Self::Pipe { pipe, action })
    }

    fn no_options(rest: &[String]) -> Result<(), String> {
        match rest.first() {
            None => Ok(()),
            Some(extra) => Err(format!("unexpected '{extra}'")),
        }
    }

    fn flag_only(rest: &[String], flag: &str) -> Result<bool, String> {
        match rest {
            [] => Ok(false),
            [only] if only == flag => Ok(true),
            [extra, ..] => Err(format!("unexpected '{extra}'")),
        }
    }
}

/// The words and `--options` after a pipe command; every one must be used.
struct Options {
    words: Vec<String>,
    flags: Vec<String>,
    values: Vec<(String, String)>,
}

impl Options {
    const WITH_VALUE: [&'static str; 3] = ["--name", "--dismiss", "--pipe"];

    fn split(args: &[String]) -> Result<Self, String> {
        let mut options = Self {
            words: Vec::new(),
            flags: Vec::new(),
            values: Vec::new(),
        };
        let mut args = args.iter();
        while let Some(arg) = args.next() {
            if Self::WITH_VALUE.contains(&arg.as_str()) {
                let value = args.next().ok_or_else(|| format!("{arg} needs a value"))?;
                options.values.push((arg.clone(), value.clone()));
            } else if arg.starts_with("--") {
                options.flags.push(arg.clone());
            } else {
                options.words.push(arg.clone());
            }
        }
        Ok(options)
    }

    fn flag(&mut self, name: &str) -> bool {
        let before = self.flags.len();
        self.flags.retain(|f| f != name);
        before != self.flags.len()
    }

    fn value(&mut self, name: &str) -> Result<Option<String>, String> {
        let mut found: Vec<String> = Vec::new();
        self.values.retain(|(option, value)| {
            let hit = option == name;
            if hit {
                found.push(value.clone());
            }
            !hit
        });
        match found.len() {
            0 => Ok(None),
            1 => Ok(found.pop()),
            _ => Err(format!("{name} is given twice")),
        }
    }

    fn take_words(&mut self) -> Vec<String> {
        std::mem::take(&mut self.words)
    }

    fn at_most_one(&mut self, what: &str) -> Result<Option<String>, String> {
        let mut words = self.take_words();
        match words.len() {
            0 | 1 => Ok(words.pop()),
            _ => Err(format!("expected one {what}, got '{}'", words.join(" "))),
        }
    }

    fn exactly_one(&mut self, what: &str) -> Result<String, String> {
        self.at_most_one(what)?
            .ok_or_else(|| format!("expected {what}"))
    }

    fn finish(self) -> Result<(), String> {
        let unused = self
            .words
            .iter()
            .chain(&self.flags)
            .chain(self.values.iter().map(|(option, _)| option))
            .next();
        match unused {
            None => Ok(()),
            Some(extra) => Err(format!("unexpected '{extra}'")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(line: &str) -> Result<Command, String> {
        let args: Vec<String> = line.split_whitespace().map(String::from).collect();
        Command::parse(&args)
    }

    fn action(line: &str) -> PipeAction {
        match parse(line).unwrap() {
            Command::Pipe { action, .. } => action,
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn parses_every_command() {
        assert_eq!(parse("help").unwrap(), Command::Help);
        assert_eq!(parse("service").unwrap(), Command::Service);
        assert_eq!(
            parse("install --restore").unwrap(),
            Command::Install { restore: true }
        );
        assert_eq!(
            parse("uninstall").unwrap(),
            Command::Uninstall { purge: false }
        );
        assert_eq!(parse("cleanup").unwrap(), Command::Cleanup);
        assert!(matches!(parse("run --dry-run").unwrap(), Command::Run(_)));
        assert_eq!(action("list"), PipeAction::List);
        assert_eq!(
            action("status AcmeCorp"),
            PipeAction::Status {
                name: Some("AcmeCorp".into())
            }
        );
        assert_eq!(
            action("import a.conf --preview --name Acme"),
            PipeAction::Import {
                files: vec!["a.conf".into()],
                name: Some("Acme".into()),
                preview: true,
                replace: false,
            }
        );
        assert_eq!(
            action("connect Acme"),
            PipeAction::Connect {
                name: "Acme".into()
            }
        );
        assert_eq!(
            action("disconnect --all"),
            PipeAction::Disconnect {
                target: ProfileTarget::All
            }
        );
        assert_eq!(
            action("alerts --dismiss gp_nrpt::"),
            PipeAction::Alerts {
                dismiss: Some("gp_nrpt::".into()),
                restore: false
            }
        );
        assert_eq!(action("lists --save"), PipeAction::Lists { save: true });
        match parse("list --pipe SplitWire.DryRun").unwrap() {
            Command::Pipe { pipe, .. } => assert_eq!(pipe, PipeName::dry_run()),
            other => panic!("{other:?}"),
        }
        match parse("remove Acme").unwrap() {
            Command::Pipe { pipe, action } => {
                assert!(pipe.is_service());
                assert_eq!(
                    action,
                    PipeAction::Remove {
                        name: "Acme".into()
                    }
                );
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn rejects_what_does_not_fit() {
        for bad in [
            "",
            "start",
            "list extra",
            "status A B",
            "import",
            "import a.conf b.conf --name X",
            "connect",
            "connect A --for soon",
            "disconnect",
            "disconnect A --all",
            "remove",
            "alerts --dismiss x --restore",
            "lists --verbose",
            "install --purge",
            "service now",
            "list --pipe",
            "list --pipe a/b",
            "status --pipe A --pipe B",
            "connect Acme --for 1h",
        ] {
            assert!(parse(bad).is_err(), "{bad}");
        }
    }
}
