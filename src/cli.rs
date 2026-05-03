use std::path::PathBuf;

use clap::Parser;

/// An early-stage shell implementation in Rust.
#[derive(Parser, Debug)]
#[command(name = "ferrish", version, about)]
pub struct Cli {
    /// Script file to execute non-interactively.
    #[arg(value_name = "SCRIPT")]
    pub script: Option<PathBuf>,

    /// Execute a command string then exit.
    #[arg(short = 'c', value_name = "COMMAND", conflicts_with = "script")]
    pub command: Option<String>,
}

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
    fn script_arg_parses() {
        let cli = Cli::try_parse_from(["ferrish", "script.sh"]).unwrap();
        assert_eq!(cli.script, Some(PathBuf::from("script.sh")));
        assert!(cli.command.is_none());
    }

    #[test]
    fn command_flag_parses() {
        let cli = Cli::try_parse_from(["ferrish", "-c", "echo hello"]).unwrap();
        assert_eq!(cli.command.as_deref(), Some("echo hello"));
        assert!(cli.script.is_none());
    }

    #[test]
    fn script_and_command_flag_conflict() {
        let err = Cli::try_parse_from(["ferrish", "script.sh", "-c", "echo hi"]).unwrap_err();
        assert_eq!(err.kind(), ErrorKind::ArgumentConflict);
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
