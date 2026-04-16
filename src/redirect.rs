use std::path::PathBuf;

/// Which standard file descriptor a redirection targets.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StdFd {
    /// Standard output (fd 1).
    Stdout,
    /// Standard error (fd 2).
    Stderr,
}

/// How the target file is opened.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RedirectMode {
    /// Truncate and overwrite the file (`>`/`1>`/`2>`).
    Overwrite,
    /// Append to the file, creating it if absent (`>>`/`2>>`).
    Append,
}

/// A single redirection extracted from a command line.
///
/// Covers all four operators: `>`/`1>` (stdout overwrite), `>>` (stdout
/// append), `2>` (stderr overwrite), and `2>>` (stderr append).  The parser
/// applies "last redirect wins" semantics — if the same fd appears more than
/// once in a command line, only the last operator is recorded here.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Redirection {
    /// Which file descriptor is being redirected.
    pub fd: StdFd,
    /// Whether to overwrite or append to the target file.
    pub mode: RedirectMode,
    /// Path to the target file.
    pub target: PathBuf,
}

impl Redirection {
    /// Create a new redirection.
    pub fn new(
        fd: StdFd,
        mode: RedirectMode,
        target: impl Into<PathBuf>,
    ) -> Self {
        Self {
            fd,
            mode,
            target: target.into(),
        }
    }
}
