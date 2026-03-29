use std::process::ExitStatus;

use crate::arg::Arg;
use thiserror::Error;

/// Convenience result type for shell execution results
pub type ShellResult<T> = anyhow::Result<T, ShellError>;

/// Errors that can occur during command execution
///
/// These are domain errors that the shell can handle gracefully.
/// They're displayed to the user but don't crash the shell.
#[derive(Error, Debug)]
pub enum ShellError {
    // --- Command-level ---
    #[error("command not found")]
    CommandNotFound,

    #[error("missing operand")]
    MissingOperand,

    // --- File system ---
    #[error("no such file or directory: {arg}")]
    FileNotFound { arg: Arg },

    #[error("is a directory: {arg}")]
    IsADirectory { arg: Arg },

    #[error("not a directory: {arg}")]
    NotADirectory { arg: Arg },

    // --- Process execution ---
    #[error("exited with status {0}")]
    NonZeroExit(ExitStatus),

    #[error("failed to spawn: {0}")]
    SpawnFailed(#[source] std::io::Error),

    #[error("failed to wait: {0}")]
    WaitFailed(#[source] std::io::Error),

    // --- I/O ---
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

impl PartialEq for ShellError {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::FileNotFound { arg: l_arg }, Self::FileNotFound { arg: r_arg }) => {
                l_arg == r_arg
            }
            (Self::IsADirectory { arg: l_arg }, Self::IsADirectory { arg: r_arg }) => {
                l_arg == r_arg
            }
            (Self::NotADirectory { arg: l_arg }, Self::NotADirectory { arg: r_arg }) => {
                l_arg == r_arg
            }
            (Self::NonZeroExit(l0), Self::NonZeroExit(r0)) => l0 == r0,
            (Self::SpawnFailed(l0), Self::SpawnFailed(r0)) => {
                l0.raw_os_error() == r0.raw_os_error()
            }
            (Self::WaitFailed(l0), Self::WaitFailed(r0)) => l0.raw_os_error() == r0.raw_os_error(),
            (Self::Io(l0), Self::Io(r0)) => l0.raw_os_error() == r0.raw_os_error(),
            _ => core::mem::discriminant(self) == core::mem::discriminant(other),
        }
    }
}

impl ShellError {
    /// Check if this error is fatal (should stop execution)
    pub fn is_fatal(&self) -> bool {
        matches!(self, ShellError::SpawnFailed(_) | ShellError::WaitFailed(_))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_display_command_not_found() {
        let err = ShellError::CommandNotFound;
        assert_eq!(err.to_string(), "command not found");
    }

    #[test]
    fn test_display_missing_operand() {
        let err = ShellError::MissingOperand;
        assert_eq!(err.to_string(), "missing operand");
    }

    #[test]
    fn test_display_file_not_found() {
        let err = ShellError::FileNotFound {
            arg: Arg::from("/path/to/file"),
        };
        assert_eq!(err.to_string(), "no such file or directory: /path/to/file");
    }

    #[test]
    fn test_display_is_a_directory() {
        let err = ShellError::IsADirectory {
            arg: Arg::from("/some/dir"),
        };
        assert_eq!(err.to_string(), "is a directory: /some/dir");
    }

    #[test]
    fn test_display_not_a_directory() {
        let err = ShellError::NotADirectory {
            arg: Arg::from("/some/file"),
        };
        assert_eq!(err.to_string(), "not a directory: /some/file");
    }

    #[test]
    fn test_is_fatal_spawn_failed() {
        let err = ShellError::SpawnFailed(std::io::Error::last_os_error());
        assert!(err.is_fatal());
    }

    #[test]
    fn test_is_fatal_wait_failed() {
        let err = ShellError::WaitFailed(std::io::Error::last_os_error());
        assert!(err.is_fatal());
    }

    #[test]
    fn test_is_fatal_command_not_found() {
        let err = ShellError::CommandNotFound;
        assert!(!err.is_fatal());
    }

    #[test]
    fn test_is_fatal_file_not_found() {
        let err = ShellError::FileNotFound {
            arg: Arg::from("test"),
        };
        assert!(!err.is_fatal());
    }

    #[test]
    #[ignore]
    fn test_equality_file_not_found() {
        let err1 = ShellError::FileNotFound {
            arg: Arg::from("file1"),
        };
        let err2 = ShellError::FileNotFound {
            arg: Arg::from("file1"),
        };
        assert_eq!(err1, err2);
    }

    #[test]
    #[ignore]
    fn test_inequality_file_not_found() {
        let err1 = ShellError::FileNotFound {
            arg: Arg::from("file1"),
        };
        let err2 = ShellError::FileNotFound {
            arg: Arg::from("file2"),
        };
        assert_ne!(err1, err2);
    }

    #[test]
    #[ignore]
    fn test_equality_different_variant() {
        let err1 = ShellError::CommandNotFound;
        let err2 = ShellError::MissingOperand;
        assert_ne!(err1, err2);
    }

    #[test]
    #[ignore]
    fn test_equality_nonzero_exit() {
        use std::process::Command as StdCommand;

        let status1 = StdCommand::new("sh").arg("-c").arg("exit 42").status().unwrap();
        let status2 = StdCommand::new("sh").arg("-c").arg("exit 42").status().unwrap();

        let e1 = ShellError::NonZeroExit(status1);
        let e2 = ShellError::NonZeroExit(status2);
        assert_eq!(e1, e2);
    }

    #[test]
    #[ignore]
    fn test_equality_spawn_wait_io() {
        // Use synthetic OS error codes to compare raw_os_error equality
        let s1 = ShellError::SpawnFailed(std::io::Error::from_raw_os_error(2));
        let s2 = ShellError::SpawnFailed(std::io::Error::from_raw_os_error(2));
        assert_eq!(s1, s2);

        let w1 = ShellError::WaitFailed(std::io::Error::from_raw_os_error(3));
        let w2 = ShellError::WaitFailed(std::io::Error::from_raw_os_error(3));
        assert_eq!(w1, w2);

        let i1 = ShellError::Io(std::io::Error::from_raw_os_error(4));
        let i2 = ShellError::Io(std::io::Error::from_raw_os_error(4));
        assert_eq!(i1, i2);
    }
}
