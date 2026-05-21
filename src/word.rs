use std::path::PathBuf;

use bytes::Bytes;
use miette::SourceSpan;

/// A shell word produced by the parser.
///
/// Carries the combined byte content and the raw span, enabling precise
/// diagnostics without threading source context through every call site.
/// Quotes and backslash escapes are stripped from `bytes`; the `span`
/// covers the full raw extent (including quote characters). Consequently
/// `span.len() == bytes.len()` iff the word is a pure unquoted, unescaped
/// sequence — the parser uses this to distinguish redirect operators from
/// quoted look-alikes.
#[derive(Debug, Clone)]
pub struct Word {
    /// Combined byte content (quotes stripped, escapes resolved).
    pub bytes: Bytes,
    /// Byte offset and length of this word in the raw input.
    pub span: SourceSpan,
}

impl Word {
    /// Construct a synthetic word with no real source location.
    ///
    /// Use only in tests or for words not derived from user input.
    pub fn synthetic(bytes: &[u8]) -> Self {
        Word {
            bytes: Bytes::copy_from_slice(bytes),
            span: SourceSpan::from((0_usize, 0_usize)),
        }
    }

    /// The combined byte content of this word.
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// Byte offset and length of this word in the raw input.
    pub fn span(&self) -> SourceSpan {
        self.span
    }
}

impl PartialEq for Word {
    fn eq(&self, other: &Self) -> bool {
        self.bytes == other.bytes
    }
}

impl Eq for Word {}

impl std::fmt::Display for Word {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", String::from_utf8_lossy(&self.bytes))
    }
}

impl From<&Word> for PathBuf {
    fn from(val: &Word) -> Self {
        PathBuf::from(val.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn synthetic_as_bytes() {
        let w = Word::synthetic(b"hello");
        assert_eq!(w.as_bytes(), b"hello");
    }

    #[test]
    fn display_renders_bytes() {
        let w = Word::synthetic(b"display_test");
        assert_eq!(w.to_string(), "display_test");
    }

    #[test]
    fn equality_ignores_span() {
        let w1 = Word::synthetic(b"same");
        let w2 = Word::synthetic(b"same");
        assert_eq!(w1, w2);
    }

    #[test]
    fn to_pathbuf() {
        let w = Word::synthetic(b"/path/to/file");
        let p = PathBuf::from(&w);
        assert_eq!(p, PathBuf::from("/path/to/file"));
    }
}
