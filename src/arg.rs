use std::path::PathBuf;

use miette::SourceSpan;

use crate::input::InputSource;

/// A list of shell command arguments.
pub type Args = Vec<Arg>;

/// The quoting context in which a shell argument was parsed.
///
/// Tracks whether the argument originated from a single-quoted string, a
/// double-quoted string, an entirely unquoted token, or a mixed-quoting
/// context.  This is used by future features: globbing is suppressed for all
/// quoted args; variable expansion is suppressed for single-quoted args;
/// operator detection (e.g. redirects) is restricted to [`QuoteStyle::None`]-style tokens.
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

/// Represents a shell command argument produced by the parser.
///
/// Every `Arg` carries the byte offset and source of the token as it appeared
/// in the original input line, so executor errors can embed self-contained
/// diagnostic labels without any additional context threading.
#[derive(Debug, Clone)]
pub enum Arg {
    /// An unquoted token.
    Literal {
        /// The parsed byte content.
        bytes: Vec<u8>,
        /// Byte offset and length of this token in the raw input line.
        span: SourceSpan,
        /// The raw input line this token was parsed from.
        src: InputSource,
    },
    /// A token whose quoting origin is preserved.
    Quoted {
        /// The parsed byte content (quotes stripped, escapes resolved).
        bytes: Vec<u8>,
        /// The quoting context in which this argument was parsed.
        style: QuoteStyle,
        /// Byte offset and length of this token in the raw input line.
        span: SourceSpan,
        /// The raw input line this token was parsed from.
        src: InputSource,
    },
}

impl Arg {
    /// Construct a synthetic argument with no source location.
    ///
    /// Use only in tests or for arguments that are not derived from user input.
    pub fn synthetic(bytes: &[u8]) -> Self {
        Arg::Literal {
            bytes: bytes.to_vec(),
            span: SourceSpan::from((0, 0)),
            src: InputSource::synthetic(),
        }
    }

    /// The parsed byte content of this argument.
    pub fn as_bytes(&self) -> &[u8] {
        match self {
            Arg::Literal { bytes, .. } | Arg::Quoted { bytes, .. } => bytes,
        }
    }

    /// Byte offset and length of this token in the raw input line.
    pub fn span(&self) -> SourceSpan {
        match self {
            Arg::Literal { span, .. } | Arg::Quoted { span, .. } => *span,
        }
    }

    /// The raw input source this token was parsed from.
    pub fn src(&self) -> InputSource {
        match self {
            Arg::Literal { src, .. } | Arg::Quoted { src, .. } => src.clone(),
        }
    }
}

impl PartialEq for Arg {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Arg::Literal { bytes: a, .. }, Arg::Literal { bytes: b, .. }) => a == b,
            (
                Arg::Quoted {
                    bytes: a,
                    style: sa,
                    ..
                },
                Arg::Quoted {
                    bytes: b,
                    style: sb,
                    ..
                },
            ) => a == b && sa == sb,
            _ => false,
        }
    }
}

impl Eq for Arg {}

impl std::fmt::Display for Arg {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", String::from_utf8_lossy(self.as_bytes()))
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
    fn synthetic_as_bytes() {
        let arg = Arg::synthetic(b"hello");
        assert_eq!(arg.as_bytes(), b"hello");
        assert!(matches!(arg, Arg::Literal { .. }));
    }

    #[test]
    fn display_renders_bytes() {
        let arg = Arg::synthetic(b"display_test");
        assert_eq!(arg.to_string(), "display_test");
    }

    #[test]
    fn equality_ignores_span_and_src() {
        let arg1 = Arg::synthetic(b"same");
        let arg2 = Arg::synthetic(b"same");
        assert_eq!(arg1, arg2);
    }

    #[test]
    fn to_pathbuf() {
        let arg = Arg::synthetic(b"/path/to/file");
        let pathbuf = PathBuf::from(&arg);
        assert_eq!(pathbuf, PathBuf::from("/path/to/file"));
    }

    #[test]
    fn quoted_as_bytes() {
        let arg = Arg::Quoted {
            bytes: b"hello world".to_vec(),
            style: QuoteStyle::Single,
            span: SourceSpan::from((0, 13)),
            src: InputSource::synthetic(),
        };
        assert_eq!(arg.as_bytes(), b"hello world");
        assert_eq!(arg.to_string(), "hello world");
    }
}
