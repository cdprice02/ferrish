use std::path::PathBuf;

use bytes::Bytes;
use miette::SourceSpan;

use crate::input::InputSource;
use crate::lexer::{Token, TokenKind};

/// A list of shell command arguments.
pub type Args = Vec<Arg>;

/// Represents a shell command argument produced by the parser.
///
/// Every `Arg` carries the source tokens, combined byte content, absolute span,
/// and input source, enabling precise diagnostics without any extra context threading.
#[derive(Debug, Clone)]
pub struct Arg {
    /// The constituent lexer tokens that make up this argument.
    pub tokens: Vec<Token>,
    /// Combined byte content of all tokens (quotes stripped, escapes resolved).
    pub bytes: Bytes,
    /// Byte offset and length spanning from the first to the last token in the raw input.
    pub span: SourceSpan,
    /// The raw input line this argument was parsed from.
    pub src: InputSource,
}

impl Arg {
    /// Construct a synthetic argument with no real source location.
    ///
    /// Use only in tests or for arguments not derived from user input.
    pub fn synthetic(bytes: &[u8]) -> Self {
        let b = Bytes::copy_from_slice(bytes);
        // Zero-length span: synthetic source has no bytes, so any non-zero span would be
        // out-of-bounds if miette ever tries to render it.
        let span = SourceSpan::from((0_usize, 0_usize));
        let src = InputSource::synthetic();
        Arg {
            tokens: vec![Token {
                kind: TokenKind::Word(b.clone()),
                span,
                src: src.clone(),
            }],
            bytes: b,
            span,
            src,
        }
    }

    /// The combined byte content of this argument.
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// Byte offset and length of this argument in the raw input.
    pub fn span(&self) -> SourceSpan {
        self.span
    }

    /// The raw input source this argument was parsed from.
    pub fn src(&self) -> InputSource {
        self.src.clone()
    }
}

impl PartialEq for Arg {
    fn eq(&self, other: &Self) -> bool {
        self.bytes == other.bytes
    }
}

impl Eq for Arg {}

impl std::fmt::Display for Arg {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", String::from_utf8_lossy(&self.bytes))
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
        assert_eq!(arg.tokens.len(), 1);
        assert!(matches!(arg.tokens[0].kind, TokenKind::Word(_)));
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
}
