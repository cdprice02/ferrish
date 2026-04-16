/// Stdout redirection target extracted from a command line.
///
/// When the parser encounters `>` or `1>` followed by a filename, it records
/// the target here and removes the operator tokens from the argument list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Redirect {
    /// Path to the file that should receive the command's standard output.
    pub target: std::path::PathBuf,
}

impl Redirect {
    /// Create a new stdout redirect targeting `target`.
    pub fn new(target: impl Into<std::path::PathBuf>) -> Self {
        Self {
            target: target.into(),
        }
    }
}

/// Stdout append-redirection target extracted from a command line.
///
/// When the parser encounters `>>` followed by a filename, it records the
/// target here and removes the operator tokens from the argument list.
/// Unlike [`Redirect`], existing file content is preserved — new output is
/// appended at the end.  If the file does not exist it is created.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RedirectAppend {
    /// Path to the file that should receive appended standard output.
    pub target: std::path::PathBuf,
}

impl RedirectAppend {
    /// Create a new stdout append redirect targeting `target`.
    pub fn new(target: impl Into<std::path::PathBuf>) -> Self {
        Self {
            target: target.into(),
        }
    }
}

/// Stderr redirection target extracted from a command line.
///
/// When the parser encounters `2>` followed by a filename, it records the
/// target here and removes the operator tokens from the argument list.
/// Standard output is unaffected and continues to go to the terminal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StderrRedirect {
    /// Path to the file that should receive the command's standard error.
    pub target: std::path::PathBuf,
}

impl StderrRedirect {
    /// Create a new stderr redirect targeting `target`.
    pub fn new(target: impl Into<std::path::PathBuf>) -> Self {
        Self {
            target: target.into(),
        }
    }
}
