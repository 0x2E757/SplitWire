//! File names of the program's executables in a package. Cargo builds them under names of its
//! own, which may hold no dots; packaging copies each to its name here (tools\package-names.ps1
//! keeps the same list for the scripts, and a test compares them).

pub struct Executables;

impl Executables {
    /// The service and the command line.
    pub const SERVICE: &'static str = "SplitWire.exe";
    /// The tray icon and the window.
    pub const WINDOW: &'static str = "SplitWire.Window.exe";
    /// Cargo's name of each packaged file, then the name it is packaged under.
    pub const PACKAGED: [(&'static str, &'static str); 2] = [
        ("splitwire.exe", Self::SERVICE),
        ("splitwire-window.exe", Self::WINDOW),
    ];

    /// The packaged name of the file Cargo builds as `built`.
    pub fn packaged(built: &str) -> Option<&'static str> {
        Self::PACKAGED
            .iter()
            .find(|(cargo, _)| *cargo == built)
            .map(|(_, packaged)| *packaged)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_scripts_package_the_same_names() {
        let script = include_str!("../../tools/package-names.ps1");
        let pairs: Vec<_> = script
            .lines()
            .filter(|line| line.trim_start().starts_with('\''))
            .filter_map(|line| {
                let (cargo, packaged) = line.trim().split_once(" = ")?;
                Some((cargo.trim_matches('\''), packaged.trim_matches('\'')))
            })
            .collect();
        assert_eq!(pairs, Executables::PACKAGED);
        assert_eq!(
            Executables::packaged("splitwire-window.exe"),
            Some(Executables::WINDOW)
        );
        assert_eq!(Executables::packaged("other.exe"), None);
    }
}
