use std::collections::VecDeque;
use std::fmt;

use bytes::Bytes;
use miette::SourceSpan;

use crate::error::ShellError;

/// The kind of quote that was opened but never closed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QuoteKind {
    /// A single-quote (`'`) was opened but never closed.
    Single,
    /// A double-quote (`"`) was opened but never closed.
    Double,
}

impl fmt::Display for QuoteKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            QuoteKind::Single => write!(f, "single"),
            QuoteKind::Double => write!(f, "double"),
        }
    }
}

/// The syntactic kind of a token.
///
/// Adjacent quoted and unquoted segments (e.g. `pre'mid'post`) are accumulated
/// into a single `Word` token by the scanner. Quote characters and backslash
/// escapes are stripped from the content bytes but included in the span, so
/// `span.len() == bytes.len()` iff the word is a pure unquoted, unescaped
/// sequence — the parser uses this to distinguish quoted redirect operators.
#[derive(Debug, Clone)]
pub enum TokenKind {
    /// A word: any whitespace-delimited token that is not a pipe. The span
    /// covers the full raw extent (quotes included); `bytes` holds the resolved
    /// content (quotes and escape chars stripped).
    Word(Bytes),
    /// An unquoted `|` — always a pipeline separator.
    Pipe,
}

/// A single token produced by the tokenizer.
#[derive(Debug, Clone)]
pub struct Token {
    /// The syntactic content and kind of this token.
    pub kind: TokenKind,
    /// Byte offset and length of this token in the raw input.
    pub span: SourceSpan,
}

/// Intermediate descriptor emitted by the scanner during [`push`] and resolved
/// to [`Token`] values inside [`Scanner::finalize`] once the buffer is frozen.
///
/// [`push`]: crate::scanner::Scanner::push
/// [`Scanner::finalize`]: crate::scanner::Scanner::finalize
pub(crate) enum TokenDesc {
    Word {
        start: usize,
        end: usize,
        /// `None` for pure unquoted/unescaped words whose content equals
        /// `raw[start..end]`. `Some(v)` when any quote or escape was processed.
        content: Option<Vec<u8>>,
    },
    Pipe {
        pos: usize,
    },
    Error(ShellError),
}

/// Finalized, consumable token stream produced by [`Scanner::finalize`].
///
/// Implements [`Iterator`] to yield [`Token`] values one at a time. Unescaped
/// word tokens are zero-copy slices of the frozen raw buffer; escaped/quoted
/// tokens own their resolved content bytes.
///
/// [`Scanner::finalize`]: crate::scanner::Scanner::finalize
pub struct Tokenizer {
    raw: Bytes,
    tokens: VecDeque<Result<Token, ShellError>>,
}

impl Tokenizer {
    pub(crate) fn new(raw: Bytes, tokens: VecDeque<Result<Token, ShellError>>) -> Self {
        Self { raw, tokens }
    }

    /// The raw bytes as a slice, exactly as pushed to the [`Scanner`].
    pub fn raw_bytes(&self) -> &[u8] {
        &self.raw
    }

    /// A zero-copy [`Bytes`] clone of the raw input (ref-count increment only).
    ///
    /// Use this in the error path of [`Shell::step`] so the pipeline can
    /// consume the `Tokenizer` while the error path retains access to the
    /// source bytes without an extra allocation.
    ///
    /// [`Shell::step`]: crate::shell::Shell
    pub fn raw_bytes_shared(&self) -> Bytes {
        self.raw.clone()
    }
}

impl Iterator for Tokenizer {
    type Item = Result<Token, ShellError>;

    fn next(&mut self) -> Option<Self::Item> {
        self.tokens.pop_front()
    }
}
