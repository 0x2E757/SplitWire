use std::fmt;

/// The name of a SplitWire pipe under `\\.\pipe\`: the service's `SplitWire`, a dry run's
/// `SplitWire.DryRun`, or another one given by `--pipe` (tests use `SplitWire.Test.<pid>`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PipeName(String);

impl PipeName {
    pub const SERVICE: &'static str = "SplitWire";
    pub const DRY_RUN: &'static str = "SplitWire.DryRun";
    const MAX_LEN: usize = 64;

    /// Letters, digits, `.`, `_` and `-` only, so a name cannot leave `\\.\pipe\`.
    pub fn parse(name: &str) -> Result<Self, String> {
        let valid = !name.is_empty()
            && name.len() <= Self::MAX_LEN
            && name
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'));
        if valid {
            Ok(Self(name.to_owned()))
        } else {
            Err(format!(
                "'{name}' is not a pipe name (letters, digits, '.', '_' and '-', at most 64)"
            ))
        }
    }

    pub fn service() -> Self {
        Self(Self::SERVICE.into())
    }

    pub fn dry_run() -> Self {
        Self(Self::DRY_RUN.into())
    }

    /// The service's pipe: a client checks that SYSTEM or an administrator serves it.
    pub fn is_service(&self) -> bool {
        self.0.eq_ignore_ascii_case(Self::SERVICE)
    }

    pub fn path(&self) -> String {
        format!(r"\\.\pipe\{}", self.0)
    }
}

impl fmt::Display for PipeName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_stay_under_the_pipe_folder() {
        assert_eq!(PipeName::service().path(), r"\\.\pipe\SplitWire");
        assert_eq!(PipeName::dry_run().path(), r"\\.\pipe\SplitWire.DryRun");
        assert!(PipeName::parse("splitwire").unwrap().is_service());
        assert!(!PipeName::dry_run().is_service());
        for bad in ["", r"..\x", "a/b", "a b", &"x".repeat(65)] {
            assert!(PipeName::parse(bad).is_err(), "{bad}");
        }
    }
}
