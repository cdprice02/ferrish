use std::process::ExitStatus;

use crate::arg::Arg;
use crate::command::builtin::BuiltInName;
use thiserror::Error;

/// Wrapper around anyhow::Result for shell execution results
pub type ShellResult = anyhow::Result<bool, ShellError>;

/// Errors that can occur during command execution
///
/// These are domain errors that the shell can handle gracefully.
/// They're displayed to the user but don't crash the shell.
#[derive(Error, Debug)]
pub enum ShellError {
    // --- Command-level ---
    #[error("command not found")]
    CommandNotFound,

    #[error("{builtin}: missing operand")]
    MissingOperand { builtin: BuiltInName },

    // --- File system ---
    #[error("{arg}: no such file or directory")]
    FileNotFound { arg: Arg },

    #[error("{arg}: not a directory")]
    NotADirectory { arg: Arg },

    #[error("{arg}: is a directory")]
    IsADirectory { arg: Arg },

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

impl ShellError {
    /// Check if this error is fatal (should stop execution)
    pub fn is_fatal(&self) -> bool {
        matches!(self, ShellError::SpawnFailed(_) | ShellError::WaitFailed(_))
    }
}
