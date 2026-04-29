use clap::Parser;

/// An early-stage shell implementation in Rust.
#[derive(Parser, Debug)]
#[command(name = "ferrish", version, about)]
pub struct Cli {}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::{error::ErrorKind, CommandFactory};

    #[test]
    fn cli_debug_assert() {
        Cli::command().debug_assert();
    }

    #[test]
    fn bare_invocation_succeeds() {
        assert!(Cli::try_parse_from(["ferrish"]).is_ok());
    }

    #[test]
    fn help_long() {
        let err = Cli::try_parse_from(["ferrish", "--help"]).unwrap_err();
        assert_eq!(err.kind(), ErrorKind::DisplayHelp);
        let rendered = err.render().to_string();
        assert!(rendered.contains("Usage:"), "missing Usage: in help");
        assert!(rendered.contains("--help"), "missing --help in help");
        assert!(rendered.contains("--version"), "missing --version in help");
    }

    #[test]
    fn help_short() {
        let err = Cli::try_parse_from(["ferrish", "-h"]).unwrap_err();
        assert_eq!(err.kind(), ErrorKind::DisplayHelp);
    }

    #[test]
    fn version_long() {
        let err = Cli::try_parse_from(["ferrish", "--version"]).unwrap_err();
        assert_eq!(err.kind(), ErrorKind::DisplayVersion);
        let rendered = err.render().to_string();
        assert!(
            rendered.contains(env!("CARGO_PKG_VERSION")),
            "missing version string in output"
        );
    }

    #[test]
    fn version_short() {
        let err = Cli::try_parse_from(["ferrish", "-V"]).unwrap_err();
        assert_eq!(err.kind(), ErrorKind::DisplayVersion);
    }

    #[test]
    fn unknown_flag_exits_nonzero() {
        let err = Cli::try_parse_from(["ferrish", "--bogus"]).unwrap_err();
        assert_eq!(err.exit_code(), 2);
    }
}
