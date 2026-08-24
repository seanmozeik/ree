//! Command-line entry point for `ree`.

use std::process::ExitCode;

#[derive(Debug, usage::Cli)]
#[usage(
    bin = "ree",
    version,
    about = env!("CARGO_PKG_DESCRIPTION"),
    license = env!("CARGO_PKG_LICENSE"),
    unknown_flags = "error"
)]
struct Cli {}

fn main() -> ExitCode {
    let _cli = Cli::parse();

    match ree::reset() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) if error.requires_silent_exit() => ExitCode::FAILURE,
        Err(error) => {
            eprintln!("ree: {error}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use std::ffi::OsStr;

    use super::*;

    #[test]
    fn accepts_an_empty_command_line() {
        assert!(Cli::parse_from(&[]).is_ok());
    }

    #[test]
    fn rejects_unknown_flags() {
        assert!(Cli::parse_from(&[OsStr::new("--unknown")]).is_err());
    }

    #[test]
    fn exports_a_usage_spec() {
        let spec = Cli::to_kdl();
        assert!(spec.contains("name ree"));
        assert!(spec.contains("version \"0.1.0\""));
    }
}
