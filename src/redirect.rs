use std::path::PathBuf;

/// How the target file is opened.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RedirectMode {
    /// Truncate and overwrite the file (`>`/`1>`/`2>`).
    Overwrite,
    /// Append to the file, creating it if absent (`>>`/`1>>`/`2>>`).
    Append,
}

/// A stdout redirection (`>`, `1>`, `>>`, `1>>`).
///
/// The parameter type encodes the target fd — passing a `StdoutRedirection`
/// to `execute()` always redirects fd 1.  There is no redundant fd field to
/// keep in sync.
///
/// The parser applies "last redirect wins" semantics per fd: if `>` / `1>` /
/// `>>` / `1>>` appear more than once, only the last operator is recorded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StdoutRedirection {
    /// Whether to overwrite or append to the target file.
    pub mode: RedirectMode,
    /// Path to the target file.
    pub target: PathBuf,
}

impl StdoutRedirection {
    /// Create a new stdout redirection.
    pub fn new(mode: RedirectMode, target: impl Into<PathBuf>) -> Self {
        Self {
            mode,
            target: target.into(),
        }
    }
}

/// A stderr redirection (`2>`, `2>>`).
///
/// The parameter type encodes the target fd — passing a `StderrRedirection`
/// to `execute()` always redirects fd 2.  There is no redundant fd field to
/// keep in sync.
///
/// The parser applies "last redirect wins" semantics per fd: if `2>` / `2>>`
/// appear more than once, only the last operator is recorded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StderrRedirection {
    /// Whether to overwrite or append to the target file.
    pub mode: RedirectMode,
    /// Path to the target file.
    pub target: PathBuf,
}

impl StderrRedirection {
    /// Create a new stderr redirection.
    pub fn new(mode: RedirectMode, target: impl Into<PathBuf>) -> Self {
        Self {
            mode,
            target: target.into(),
        }
    }
}
