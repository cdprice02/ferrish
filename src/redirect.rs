/// Stdout redirection target extracted from a command line.
///
/// When the parser encounters `>` or `1>` followed by a filename, it records
/// the target here and removes the operator tokens from the argument list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Redirect {
    /// Path to the file that should receive the command's standard output.
    pub target: String,
}

impl Redirect {
    /// Create a new stdout redirect targeting `target`.
    pub fn new(target: impl Into<String>) -> Self {
        Self {
            target: target.into(),
        }
    }
}
