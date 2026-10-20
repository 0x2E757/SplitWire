//! `SplitWire.Window.exe`'s arguments.

use crate::platform::pipe::PipeName;
use crate::presentation::transport::demo::Scenario;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Arguments {
    /// `--demo <scenario>`: a demo scenario, served by the demo transport.
    pub demo: Option<Scenario>,
    /// `--theme light|dark`: overrides the theme (screenshots of `--demo`).
    pub dark: Option<bool>,
    /// `--verify-pages`: the page check, without a window.
    pub verify_pages: bool,
    /// `--render-pages <folder>`: a picture of every page, without a window.
    pub render_pages: Option<std::path::PathBuf>,
    /// `--tray`: started at sign-in: the tray icon, and the window only when the user's settings
    /// open it at sign-in.
    pub tray: bool,
    /// `--pipe <name>`: the service's pipe (`SplitWire.DryRun` for a dry run).
    pub pipe: Option<PipeName>,
    pub help: bool,
    /// What was not understood.
    pub problem: Option<String>,
}

impl Arguments {
    pub fn parse(args: impl IntoIterator<Item = String>) -> Self {
        let mut parsed = Self::default();
        let mut args = args.into_iter();
        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--demo" => match args.next().as_deref().map(Scenario::parse) {
                    Some(Some(scenario)) => parsed.demo = Some(scenario),
                    _ => {
                        parsed.problem = Some(format!(
                            "--demo takes one of: {}",
                            Scenario::ALL.map(Scenario::key).join(", ")
                        ))
                    }
                },
                "--theme" => match args.next().as_deref() {
                    Some("dark") => parsed.dark = Some(true),
                    Some("light") => parsed.dark = Some(false),
                    _ => parsed.problem = Some("--theme takes light or dark".into()),
                },
                "--verify-pages" => parsed.verify_pages = true,
                "--render-pages" => match args.next() {
                    Some(folder) => parsed.render_pages = Some(folder.into()),
                    None => parsed.problem = Some("--render-pages takes a folder".into()),
                },
                "--tray" => parsed.tray = true,
                "--pipe" => match args.next().as_deref().map(PipeName::parse) {
                    Some(Ok(name)) => parsed.pipe = Some(name),
                    Some(Err(problem)) => parsed.problem = Some(problem),
                    None => parsed.problem = Some("--pipe takes a pipe name".into()),
                },
                "--help" | "-h" | "/?" => parsed.help = true,
                other => parsed.problem = Some(format!("unknown argument {other}")),
            }
        }
        parsed
    }

    pub const USAGE: &'static str = "SplitWire.Window.exe [--tray] [--pipe <name>]\n\
        SplitWire.Window.exe --demo normal|firstrun|noservice|connectfail|settingsinvalid [--theme light|dark]\n\
        SplitWire.Window.exe --verify-pages
        SplitWire.Window.exe --render-pages <folder>";
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(text: &str) -> Arguments {
        Arguments::parse(text.split_whitespace().map(str::to_owned))
    }

    #[test]
    fn demo_theme_and_checks() {
        let args = parse("--demo connectfail --theme dark");
        assert_eq!(args.demo, Some(Scenario::ConnectFail));
        assert_eq!(args.dark, Some(true));
        assert!(parse("--verify-pages").verify_pages);
        assert_eq!(
            parse("--render-pages shots").render_pages,
            Some("shots".into())
        );
        assert!(parse("--render-pages").problem.is_some());
        assert!(
            parse("--demo nothing")
                .problem
                .unwrap()
                .contains("normal, firstrun")
        );
        let args = parse("--pipe SplitWire.DryRun --tray");
        assert_eq!(args.pipe, Some(PipeName::dry_run()));
        assert!(args.tray);
        assert!(parse("--pipe a/b").problem.is_some());
        assert!(parse("--pipe").problem.is_some());
        assert!(parse("--what").problem.is_some());
    }
}
