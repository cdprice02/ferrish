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
