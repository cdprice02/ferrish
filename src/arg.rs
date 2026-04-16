use std::path::PathBuf;

/// A list of shell command arguments.
pub type Args = Vec<Arg>;

/// The quoting context in which a shell argument was parsed.
///
/// Tracks whether the argument originated from a single-quoted string, a
/// double-quoted string, an entirely unquoted token, or a mixed-quoting
/// context.  This is used by future features: globbing is suppressed for all
/// quoted args; variable expansion is suppressed for single-quoted args;
/// operator detection (e.g. redirects) is restricted to `None`-style tokens.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QuoteStyle {
    /// Argument was entirely unquoted — no quote characters were present.
    None,
    /// Argument combined quoted and unquoted segments (e.g. `pre"mid"post`, `1'>'`).
    Mixed,
    /// Argument was entirely enclosed in single quotes; all characters are literal.
    Single,
    /// Argument was entirely enclosed in double quotes; backslash escaping applies.
    Double,
}

/// Represents a shell command argument.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Arg {
    /// A raw byte sequence that originated from entirely unquoted input.
    Literal(Vec<u8>),
    /// A byte sequence with its quoting origin preserved.
    Quoted {
        /// The raw byte content of the argument.
        bytes: Vec<u8>,
        /// The quoting context in which this argument was parsed.
        style: QuoteStyle,
    },
}

impl Arg {
    /// Returns the raw bytes of this argument, regardless of quoting.
    pub fn as_bytes(&self) -> &[u8] {
        match self {
            Arg::Literal(bytes) | Arg::Quoted { bytes, .. } => bytes,
        }
    }
}

impl std::fmt::Display for Arg {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", String::from_utf8_lossy(self.as_bytes()))
    }
}

impl From<&str> for Arg {
    fn from(val: &str) -> Self {
        Arg::from(val.as_bytes())
    }
}

impl From<&[u8]> for Arg {
    fn from(val: &[u8]) -> Self {
        Arg::from(val.to_vec())
    }
}

impl From<Vec<u8>> for Arg {
    fn from(val: Vec<u8>) -> Self {
        Arg::Literal(val)
    }
}

impl From<&Arg> for PathBuf {
    fn from(val: &Arg) -> Self {
        PathBuf::from(val.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_arg_from_str() {
        let arg = Arg::from("hello");
        assert_eq!(arg.as_bytes(), b"hello");
        assert!(matches!(arg, Arg::Literal(_)));
    }

    #[test]
    fn test_arg_display() {
        let arg = Arg::from("display_test");
        assert_eq!(arg.to_string(), "display_test");
    }

    #[test]
    fn test_arg_equality() {
        let arg1 = Arg::from("same");
        let arg2 = Arg::from("same");
        assert_eq!(arg1, arg2);
    }

    #[test]
    fn test_arg_to_pathbuf() {
        let arg = Arg::from("/path/to/file");
        let pathbuf = PathBuf::from(&arg);
        assert_eq!(pathbuf, PathBuf::from("/path/to/file"));
    }

    #[test]
    fn test_quoted_single_as_bytes() {
        let arg = Arg::Quoted {
            bytes: b"hello".to_vec(),
            style: QuoteStyle::Single,
        };
        assert_eq!(arg.as_bytes(), b"hello");
        assert_eq!(arg.to_string(), "hello");
    }

    #[test]
    fn test_quoted_double_as_bytes() {
        let arg = Arg::Quoted {
            bytes: b"world".to_vec(),
            style: QuoteStyle::Double,
        };
        assert_eq!(arg.as_bytes(), b"world");
    }
}
