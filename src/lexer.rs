use std::collections::VecDeque;
use std::fmt;

use bytes::{Bytes, BytesMut};
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

/// The syntactic kind of a lexed token.
///
/// Adjacent quoted and unquoted segments (e.g. `pre'mid'post`) are accumulated
/// into a single `Word` token by the lexer. Quote characters and backslash
/// escapes are stripped from the content bytes but included in the span, so
/// `span.len() == bytes.len()` iff the word is a pure unquoted sequence — the
/// parser uses this to distinguish quoted redirect operators.
#[derive(Debug, Clone)]
pub enum TokenKind {
    /// A word: any whitespace-delimited token that is not a pipe. Includes
    /// unquoted, single-quoted, double-quoted, and mixed-quoted content. The
    /// span covers the full raw extent (quotes included); `bytes` holds the
    /// resolved content (quotes and escape chars stripped).
    Word(Bytes),
    /// An unquoted `|` — always a pipeline separator.
    Pipe,
}

/// A single token produced by the lexer.
#[derive(Debug, Clone)]
pub struct Token {
    /// The syntactic content and kind of this token.
    pub kind: TokenKind,
    /// Byte offset and length of this token in the raw input.
    pub span: SourceSpan,
}

// --- Internal state machine ---

enum State {
    /// Between words (whitespace or start of input).
    Neutral,
    /// Accumulating an unquoted/mixed word.
    Word {
        /// Absolute byte offset of the first byte of this word in `buf`.
        start: usize,
        /// Accumulated content bytes (quotes stripped, escapes resolved).
        content: Vec<u8>,
    },
    /// Inside a single-quoted string (`'...'`), as part of an ongoing word.
    SingleQuote {
        /// `buf` offset where the current word started (before the `'`).
        word_start: usize,
        /// Byte offset of the opening `'` in `buf` (for `UnclosedQuote` span).
        quote_start: usize,
        /// Content accumulated so far (including bytes before the opening `'`).
        content: Vec<u8>,
    },
    /// Inside a double-quoted string (`"..."`), as part of an ongoing word.
    DoubleQuote {
        word_start: usize,
        quote_start: usize,
        content: Vec<u8>,
        /// Chunk ended on an unresolved `\`; the escape check is deferred to
        /// the first byte of the next `push` call.
        pending_bs: bool,
    },
}

/// Streaming shell lexer.
///
/// Call [`push`] to feed raw bytes; tokens are immediately processed and
/// queued for retrieval via the [`Iterator`] impl. Call [`finalize`] once
/// all input has been pushed to flush any trailing word and emit any
/// unclosed-quote error.
///
/// [`needs_continuation`] returns `true` while the lexer is inside an
/// unclosed quote, replacing the old separate `scan_bytes` pass.
///
/// [`push`]: Lexer::push
/// [`finalize`]: Lexer::finalize
/// [`needs_continuation`]: Lexer::needs_continuation
pub struct Lexer {
    buf: BytesMut,
    pos: usize,
    state: State,
    pending: VecDeque<Result<Token, ShellError>>,
    finalized: bool,
}

impl Default for Lexer {
    fn default() -> Self {
        Self::new()
    }
}

impl Lexer {
    /// Create a new, empty lexer.
    pub fn new() -> Self {
        Self {
            buf: BytesMut::new(),
            pos: 0,
            state: State::Neutral,
            pending: VecDeque::new(),
            finalized: false,
        }
    }

    /// Append `bytes` to the internal buffer and process them immediately.
    ///
    /// Complete tokens are enqueued into the pending queue. In-progress
    /// tokens (mid-word or inside an unclosed quote) remain in state until
    /// more bytes arrive or [`finalize`] is called.
    ///
    /// [`finalize`]: Lexer::finalize
    pub fn push(&mut self, bytes: &[u8]) {
        self.buf.extend_from_slice(bytes);
        self.scan();
    }

    /// Returns `true` when the lexer is inside an unclosed quote and needs
    /// more input before a complete command can be produced.
    pub fn needs_continuation(&self) -> bool {
        matches!(
            self.state,
            State::SingleQuote { .. } | State::DoubleQuote { .. }
        )
    }

    /// The raw bytes accumulated so far.
    ///
    /// Used by the shell to attach source code to diagnostic reports after
    /// all input for a command has been collected.
    pub fn raw_bytes(&self) -> &[u8] {
        &self.buf
    }

    /// Signal end-of-input.
    ///
    /// Flushes any trailing word token or emits an [`ShellError::UnclosedQuote`]
    /// error if the input ends inside a quote. Idempotent: safe to call
    /// multiple times.
    pub fn finalize(&mut self) {
        if self.finalized {
            return;
        }
        self.finalized = true;
        let end = self.buf.len();

        match std::mem::replace(&mut self.state, State::Neutral) {
            State::Neutral => {}
            State::Word { start, content } => {
                let bytes = Bytes::copy_from_slice(&content);
                let span = SourceSpan::from((start, end - start));
                self.pending.push_back(Ok(Token {
                    kind: TokenKind::Word(bytes),
                    span,
                }));
            }
            State::SingleQuote { quote_start, .. } => {
                self.pending.push_back(Err(ShellError::UnclosedQuote {
                    style: QuoteKind::Single,
                    span: SourceSpan::from((quote_start, 1)),
                }));
            }
            State::DoubleQuote { quote_start, .. } => {
                self.pending.push_back(Err(ShellError::UnclosedQuote {
                    style: QuoteKind::Double,
                    span: SourceSpan::from((quote_start, 1)),
                }));
            }
        }
    }

    fn scan(&mut self) {
        while self.pos < self.buf.len() {
            let b = self.buf[self.pos];

            // Take state to avoid borrow conflicts during transitions.
            let state = std::mem::replace(&mut self.state, State::Neutral);

            match state {
                State::Neutral => match b {
                    b if b.is_ascii_whitespace() => {
                        self.state = State::Neutral;
                        self.pos += 1;
                    }
                    b'|' => {
                        let span = SourceSpan::from((self.pos, 1));
                        self.pending.push_back(Ok(Token {
                            kind: TokenKind::Pipe,
                            span,
                        }));
                        self.state = State::Neutral;
                        self.pos += 1;
                    }
                    b'\'' => {
                        self.state = State::SingleQuote {
                            word_start: self.pos,
                            quote_start: self.pos,
                            content: Vec::new(),
                        };
                        self.pos += 1;
                    }
                    b'"' => {
                        self.state = State::DoubleQuote {
                            word_start: self.pos,
                            quote_start: self.pos,
                            content: Vec::new(),
                            pending_bs: false,
                        };
                        self.pos += 1;
                    }
                    b'\\' => {
                        if self.pos + 1 < self.buf.len() {
                            let next = self.buf[self.pos + 1];
                            self.state = State::Word {
                                start: self.pos,
                                content: vec![next],
                            };
                            self.pos += 2;
                        } else {
                            // Trailing backslash — stop; caller will provide more
                            // bytes (backslash-newline continuation) or finalize.
                            self.state = State::Neutral;
                            break;
                        }
                    }
                    _ => {
                        self.state = State::Word {
                            start: self.pos,
                            content: vec![b],
                        };
                        self.pos += 1;
                    }
                },

                State::Word { start, mut content } => match b {
                    b if b.is_ascii_whitespace() => {
                        let bytes = Bytes::copy_from_slice(&content);
                        let span = SourceSpan::from((start, self.pos - start));
                        self.pending.push_back(Ok(Token {
                            kind: TokenKind::Word(bytes),
                            span,
                        }));
                        self.state = State::Neutral;
                        self.pos += 1;
                    }
                    b'|' => {
                        let bytes = Bytes::copy_from_slice(&content);
                        let span = SourceSpan::from((start, self.pos - start));
                        self.pending.push_back(Ok(Token {
                            kind: TokenKind::Word(bytes),
                            span,
                        }));
                        let pipe_span = SourceSpan::from((self.pos, 1));
                        self.pending.push_back(Ok(Token {
                            kind: TokenKind::Pipe,
                            span: pipe_span,
                        }));
                        self.state = State::Neutral;
                        self.pos += 1;
                    }
                    b'\'' => {
                        self.state = State::SingleQuote {
                            word_start: start,
                            quote_start: self.pos,
                            content,
                        };
                        self.pos += 1;
                    }
                    b'"' => {
                        self.state = State::DoubleQuote {
                            word_start: start,
                            quote_start: self.pos,
                            content,
                            pending_bs: false,
                        };
                        self.pos += 1;
                    }
                    b'\\' => {
                        if self.pos + 1 < self.buf.len() {
                            let next = self.buf[self.pos + 1];
                            content.push(next);
                            self.state = State::Word { start, content };
                            self.pos += 2;
                        } else {
                            // Trailing backslash — wait for next push or finalize.
                            self.state = State::Word { start, content };
                            break;
                        }
                    }
                    _ => {
                        content.push(b);
                        self.state = State::Word { start, content };
                        self.pos += 1;
                    }
                },

                State::SingleQuote {
                    word_start,
                    quote_start,
                    mut content,
                } => {
                    if b == b'\'' {
                        // Closing quote: return to word accumulation.
                        self.state = State::Word {
                            start: word_start,
                            content,
                        };
                        self.pos += 1;
                    } else {
                        content.push(b);
                        self.state = State::SingleQuote {
                            word_start,
                            quote_start,
                            content,
                        };
                        self.pos += 1;
                    }
                }

                State::DoubleQuote {
                    word_start,
                    quote_start,
                    mut content,
                    pending_bs,
                } => {
                    if pending_bs {
                        // Deferred backslash from previous push boundary.
                        if matches!(b, b'"' | b'\\') {
                            content.push(b);
                        } else {
                            content.push(b'\\');
                            content.push(b);
                        }
                        self.state = State::DoubleQuote {
                            word_start,
                            quote_start,
                            content,
                            pending_bs: false,
                        };
                        self.pos += 1;
                    } else if b == b'"' {
                        self.state = State::Word {
                            start: word_start,
                            content,
                        };
                        self.pos += 1;
                    } else if b == b'\\' {
                        if self.pos + 1 < self.buf.len() {
                            let next = self.buf[self.pos + 1];
                            if matches!(next, b'"' | b'\\') {
                                content.push(next);
                                self.pos += 2;
                            } else {
                                // Non-special backslash: literal \ + next char.
                                content.push(b'\\');
                                content.push(next);
                                self.pos += 2;
                            }
                            self.state = State::DoubleQuote {
                                word_start,
                                quote_start,
                                content,
                                pending_bs: false,
                            };
                        } else {
                            // Backslash at end of chunk — defer.
                            self.state = State::DoubleQuote {
                                word_start,
                                quote_start,
                                content,
                                pending_bs: true,
                            };
                            self.pos += 1;
                        }
                    } else {
                        content.push(b);
                        self.state = State::DoubleQuote {
                            word_start,
                            quote_start,
                            content,
                            pending_bs: false,
                        };
                        self.pos += 1;
                    }
                }
            }
        }
    }
}

impl Iterator for Lexer {
    type Item = Result<Token, ShellError>;

    fn next(&mut self) -> Option<Self::Item> {
        self.pending.pop_front()
    }
}

// --- Public helpers ---

/// Returns `true` when `bytes` ends in an incomplete construct (e.g. an
/// unclosed quote) that requires more input before a complete command can
/// be parsed.
///
/// Creates a temporary `Lexer` to perform the check — identical to calling
/// `Lexer::new()`, `push(bytes)`, `needs_continuation()`.
pub fn needs_continuation(bytes: &[u8]) -> bool {
    let mut lex = Lexer::new();
    lex.push(bytes);
    lex.needs_continuation()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lex_raw(raw: &[u8]) -> Vec<Token> {
        let mut lex = Lexer::new();
        lex.push(raw);
        lex.finalize();
        lex.collect::<Result<Vec<_>, _>>().unwrap()
    }

    fn word_bytes(token: &Token) -> &[u8] {
        match &token.kind {
            TokenKind::Word(b) => b,
            TokenKind::Pipe => b"|",
        }
    }

    fn lex_err(raw: &[u8]) -> ShellError {
        let mut lex = Lexer::new();
        lex.push(raw);
        lex.finalize();
        lex.collect::<Result<Vec<_>, _>>().unwrap_err()
    }

    // --- Basic splitting ---

    #[test]
    fn simple_words_split_on_whitespace() {
        let tokens = lex_raw(b"echo hello world");
        assert_eq!(tokens.len(), 3);
        assert_eq!(word_bytes(&tokens[0]), b"echo");
        assert_eq!(word_bytes(&tokens[1]), b"hello");
        assert_eq!(word_bytes(&tokens[2]), b"world");
    }

    #[test]
    fn pipe_emitted_as_pipe_token() {
        let tokens = lex_raw(b"echo foo | cat");
        assert_eq!(tokens.len(), 4);
        assert!(matches!(tokens[2].kind, TokenKind::Pipe));
    }

    #[test]
    fn adjacent_quoted_segments_merge_into_one_token() {
        // pre'mid'post is one whitespace-delimited unit → one Word token
        let tokens = lex_raw(b"pre'mid'post");
        assert_eq!(tokens.len(), 1);
        assert!(matches!(&tokens[0].kind, TokenKind::Word(b) if b.as_ref() == b"premidpost"));
        // Span covers the raw extent including quotes
        assert_eq!(tokens[0].span.len(), 12);
    }

    #[test]
    fn single_quote_preserves_spaces() {
        let tokens = lex_raw(b"'hello    world'");
        assert_eq!(tokens.len(), 1);
        assert!(matches!(&tokens[0].kind, TokenKind::Word(b) if b.as_ref() == b"hello    world"));
    }

    #[test]
    fn double_quote_processes_backslash_escapes() {
        let tokens = lex_raw(b"\"A \\\" inside\"");
        assert_eq!(tokens.len(), 1);
        assert!(matches!(&tokens[0].kind, TokenKind::Word(b) if b.as_ref() == b"A \" inside"));
    }

    #[test]
    fn backslash_outside_quotes_escapes_next_char() {
        let tokens = lex_raw(b"three\\ \\ \\ spaces");
        assert_eq!(tokens.len(), 1);
        assert!(matches!(&tokens[0].kind, TokenKind::Word(b) if b.as_ref() == b"three   spaces"));
    }

    #[test]
    fn quoted_pipe_not_a_separator() {
        let tokens = lex_raw(b"'foo | bar'");
        assert_eq!(tokens.len(), 1);
        assert!(matches!(&tokens[0].kind, TokenKind::Word(_)));
    }

    #[test]
    fn unquoted_redirect_span_matches_bytes() {
        // span.len() == bytes.len() → eligible as redirect operator
        let tokens = lex_raw(b">");
        assert_eq!(tokens.len(), 1);
        if let TokenKind::Word(b) = &tokens[0].kind {
            assert_eq!(b.as_ref(), b">");
            assert_eq!(tokens[0].span.len(), b.len());
        }
    }

    #[test]
    fn quoted_redirect_span_exceeds_bytes() {
        // span.len() > bytes.len() → NOT a redirect operator
        let tokens = lex_raw(b"'>'");
        assert_eq!(tokens.len(), 1);
        if let TokenKind::Word(b) = &tokens[0].kind {
            assert_eq!(b.as_ref(), b">");
            assert!(tokens[0].span.len() > b.len());
        }
    }

    #[test]
    fn escaped_redirect_span_exceeds_bytes() {
        let tokens = lex_raw(b"\\>");
        assert_eq!(tokens.len(), 1);
        if let TokenKind::Word(b) = &tokens[0].kind {
            assert_eq!(b.as_ref(), b">");
            assert!(tokens[0].span.len() > b.len());
        }
    }

    #[test]
    fn unclosed_single_quote_returns_error() {
        let err = lex_err(b"echo 'hello");
        assert!(matches!(
            err,
            ShellError::UnclosedQuote {
                style: QuoteKind::Single,
                ..
            }
        ));
    }

    #[test]
    fn unclosed_double_quote_returns_error() {
        let err = lex_err(b"echo \"hello");
        assert!(matches!(
            err,
            ShellError::UnclosedQuote {
                style: QuoteKind::Double,
                ..
            }
        ));
    }

    #[test]
    fn unclosed_single_quote_span_offset() {
        let err = lex_err(b"echo 'hello");
        if let ShellError::UnclosedQuote { span, .. } = err {
            assert_eq!(span.offset(), 5);
        } else {
            panic!("expected UnclosedQuote");
        }
    }

    #[test]
    fn spans_are_absolute_including_leading_whitespace() {
        let tokens = lex_raw(b"  echo");
        assert_eq!(tokens.len(), 1);
        assert_eq!(tokens[0].span.offset(), 2);
    }

    #[test]
    fn dq_escape_at_chunk_boundary() {
        let mut lex = Lexer::new();
        lex.push(b"\"hello\\");
        assert!(lex.needs_continuation());
        lex.push(b"\"world\"");
        assert!(!lex.needs_continuation());
        lex.finalize();
        let tokens: Vec<_> = lex.collect::<Result<_, _>>().unwrap();
        assert_eq!(tokens.len(), 1);
        if let TokenKind::Word(b) = &tokens[0].kind {
            assert_eq!(b.as_ref(), b"hello\"world");
        }
    }

    #[test]
    fn dq_non_escape_at_chunk_boundary() {
        let mut lex = Lexer::new();
        lex.push(b"\"hello\\");
        assert!(lex.needs_continuation());
        lex.push(b"xworld\"");
        assert!(!lex.needs_continuation());
        lex.finalize();
        let tokens: Vec<_> = lex.collect::<Result<_, _>>().unwrap();
        assert_eq!(tokens.len(), 1);
        if let TokenKind::Word(b) = &tokens[0].kind {
            // backslash before 'x' is literal
            assert_eq!(b.as_ref(), b"hello\\xworld");
        }
    }

    #[test]
    fn incremental_push_detects_multiline_quote() {
        let mut lex = Lexer::new();
        lex.push(b"echo 'hello\n");
        assert!(lex.needs_continuation());
        lex.push(b"world'\n");
        assert!(!lex.needs_continuation());
    }

    #[test]
    fn needs_continuation_false_for_complete_input() {
        assert!(!needs_continuation(b"echo hello\n"));
    }

    #[test]
    fn needs_continuation_true_for_unclosed_single_quote() {
        assert!(needs_continuation(b"echo 'hello\n"));
    }

    #[test]
    fn needs_continuation_true_for_unclosed_double_quote() {
        assert!(needs_continuation(b"echo \"hello\n"));
    }

    #[test]
    fn needs_continuation_false_for_closed_quotes() {
        assert!(!needs_continuation(b"echo 'hello'\n"));
        assert!(!needs_continuation(b"echo \"hello\"\n"));
    }
}
