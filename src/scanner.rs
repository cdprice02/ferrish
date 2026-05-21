use std::collections::VecDeque;

use bytes::{Bytes, BytesMut};
use miette::SourceSpan;

use crate::error::ShellError;
use crate::tokenizer::{QuoteKind, Token, TokenDesc, TokenKind, Tokenizer};

enum State {
    Neutral,
    /// Accumulating a whitespace-delimited word.
    ///
    /// `content: None` while the word is a pure unescaped, unquoted byte
    /// sequence — the content is tracked implicitly by `buf[start..pos]` and
    /// resolved zero-copy in [`Scanner::finalize`]. Any escape or quote
    /// materializes the content into `Some(v)`.
    Word {
        start: usize,
        content: Option<Vec<u8>>,
    },
    /// Inside a `'...'` string, as part of an ongoing word.
    SingleQuote {
        word_start: usize,
        quote_start: usize,
        content: Vec<u8>,
    },
    /// Inside a `"..."` string, as part of an ongoing word.
    DoubleQuote {
        word_start: usize,
        quote_start: usize,
        content: Vec<u8>,
        /// Chunk ended on an unresolved `\`; the escape is deferred to the
        /// first byte of the next `push` call.
        pending_bs: bool,
    },
}

/// Byte accumulator that tokenizes shell input in a single forward pass.
///
/// Feed raw bytes with [`push`]; query readiness with [`needs_continuation`];
/// freeze into a consumable [`Tokenizer`] with [`finalize`].
///
/// Token descriptors are queued during `push` and resolved to [`Token`] values
/// inside `finalize` once the buffer is frozen — enabling zero-copy slices for
/// unescaped words.
///
/// [`push`]: Scanner::push
/// [`needs_continuation`]: Scanner::needs_continuation
/// [`finalize`]: Scanner::finalize
pub struct Scanner {
    buf: BytesMut,
    pos: usize,
    state: State,
    descs: VecDeque<TokenDesc>,
}

impl Default for Scanner {
    fn default() -> Self {
        Self::new()
    }
}

impl Scanner {
    /// Create a new, empty scanner.
    pub fn new() -> Self {
        Self {
            buf: BytesMut::new(),
            pos: 0,
            state: State::Neutral,
            descs: VecDeque::new(),
        }
    }

    /// Append `bytes` to the internal buffer and process them immediately.
    ///
    /// Complete token descriptors are queued. In-progress tokens (mid-word or
    /// inside an unclosed quote) remain in state until more bytes arrive or
    /// [`finalize`] is called.
    ///
    /// [`finalize`]: Scanner::finalize
    pub fn push(&mut self, bytes: &[u8]) {
        self.buf.extend_from_slice(bytes);
        self.scan();
    }

    /// The raw bytes accumulated so far, as a slice.
    pub fn raw_bytes(&self) -> &[u8] {
        &self.buf
    }

    /// Returns `true` when more input is required before a complete command
    /// can be produced: inside an unclosed quote, or after a trailing unquoted
    /// `\` (backslash-newline line continuation).
    pub fn needs_continuation(&self) -> bool {
        if matches!(
            self.state,
            State::SingleQuote { .. } | State::DoubleQuote { .. }
        ) {
            return true;
        }
        self.pos < self.buf.len() && self.buf[self.pos] == b'\\'
    }

    /// Consume the scanner and return a finalized, iterable [`Tokenizer`].
    ///
    /// Flushes any trailing word token or emits a [`ShellError::UnclosedQuote`]
    /// error if the input ends inside a quote. The internal buffer is frozen
    /// into a ref-counted [`Bytes`]; unescaped word tokens become zero-copy
    /// slices of that buffer.
    pub fn finalize(mut self) -> Tokenizer {
        // Use self.pos, not buf.len(): scan() stops pos before a trailing `\`,
        // so buf.len() would include it in the zero-copy slice.
        let end = self.pos;

        match std::mem::replace(&mut self.state, State::Neutral) {
            State::Neutral => {}
            State::Word { start, content } => {
                self.descs.push_back(TokenDesc::Word {
                    start,
                    end,
                    content,
                });
            }
            State::SingleQuote { quote_start, .. } => {
                self.descs
                    .push_back(TokenDesc::Error(ShellError::UnclosedQuote {
                        style: QuoteKind::Single,
                        span: SourceSpan::from((quote_start, 1)),
                    }));
            }
            State::DoubleQuote { quote_start, .. } => {
                self.descs
                    .push_back(TokenDesc::Error(ShellError::UnclosedQuote {
                        style: QuoteKind::Double,
                        span: SourceSpan::from((quote_start, 1)),
                    }));
            }
        }

        let frozen = self.buf.freeze();

        let tokens = self
            .descs
            .into_iter()
            .map(|desc| match desc {
                TokenDesc::Word {
                    start,
                    end,
                    content: None,
                } => Ok(Token {
                    kind: TokenKind::Word(frozen.slice(start..end)),
                    span: SourceSpan::from((start, end - start)),
                }),
                TokenDesc::Word {
                    start,
                    end,
                    content: Some(v),
                } => Ok(Token {
                    kind: TokenKind::Word(Bytes::from(v)),
                    span: SourceSpan::from((start, end - start)),
                }),
                TokenDesc::Pipe { pos } => Ok(Token {
                    kind: TokenKind::Pipe,
                    span: SourceSpan::from((pos, 1)),
                }),
                TokenDesc::Error(e) => Err(e),
            })
            .collect();

        Tokenizer::new(frozen, tokens)
    }

    fn scan(&mut self) {
        while self.pos < self.buf.len() {
            let b = self.buf[self.pos];
            let state = std::mem::replace(&mut self.state, State::Neutral);

            match state {
                State::Neutral => match b {
                    b if b.is_ascii_whitespace() => {
                        self.state = State::Neutral;
                        self.pos += 1;
                    }
                    b'|' => {
                        self.descs.push_back(TokenDesc::Pipe { pos: self.pos });
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
                            if next == b'\n' {
                                // \<LF> in unquoted context: POSIX line continuation.
                                self.state = State::Neutral;
                                self.pos += 2;
                            } else {
                                self.state = State::Word {
                                    start: self.pos,
                                    content: Some(vec![next]),
                                };
                                self.pos += 2;
                            }
                        } else {
                            // Trailing backslash — stop; needs_continuation() detects it.
                            self.state = State::Neutral;
                            break;
                        }
                    }
                    _ => {
                        self.state = State::Word {
                            start: self.pos,
                            content: None,
                        };
                        self.pos += 1;
                    }
                },

                State::Word { start, mut content } => match b {
                    b if b.is_ascii_whitespace() => {
                        self.descs.push_back(TokenDesc::Word {
                            start,
                            end: self.pos,
                            content,
                        });
                        self.state = State::Neutral;
                        self.pos += 1;
                    }
                    b'|' => {
                        self.descs.push_back(TokenDesc::Word {
                            start,
                            end: self.pos,
                            content,
                        });
                        self.descs.push_back(TokenDesc::Pipe { pos: self.pos });
                        self.state = State::Neutral;
                        self.pos += 1;
                    }
                    b'\'' => {
                        // Materialize content before entering the quoted segment.
                        let v = content.unwrap_or_else(|| self.buf[start..self.pos].to_vec());
                        self.state = State::SingleQuote {
                            word_start: start,
                            quote_start: self.pos,
                            content: v,
                        };
                        self.pos += 1;
                    }
                    b'"' => {
                        let v = content.unwrap_or_else(|| self.buf[start..self.pos].to_vec());
                        self.state = State::DoubleQuote {
                            word_start: start,
                            quote_start: self.pos,
                            content: v,
                            pending_bs: false,
                        };
                        self.pos += 1;
                    }
                    b'\\' => {
                        if self.pos + 1 < self.buf.len() {
                            let next = self.buf[self.pos + 1];
                            if next == b'\n' {
                                // \<LF> continuation: materialize if needed, skip both chars.
                                let v =
                                    content.unwrap_or_else(|| self.buf[start..self.pos].to_vec());
                                self.state = State::Word {
                                    start,
                                    content: Some(v),
                                };
                                self.pos += 2;
                            } else {
                                // Escape: materialize if needed, push the escaped char.
                                let mut v =
                                    content.unwrap_or_else(|| self.buf[start..self.pos].to_vec());
                                v.push(next);
                                self.state = State::Word {
                                    start,
                                    content: Some(v),
                                };
                                self.pos += 2;
                            }
                        } else {
                            // Trailing backslash — wait for next push or finalize.
                            self.state = State::Word { start, content };
                            break;
                        }
                    }
                    _ => {
                        if let Some(ref mut v) = content {
                            v.push(b);
                        }
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
                        self.state = State::Word {
                            start: word_start,
                            content: Some(content),
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
                        // Resolve a backslash that arrived at the previous push boundary.
                        if matches!(b, b'"' | b'\\') {
                            content.push(b);
                        } else if b == b'\n' {
                            // \<LF> across a push boundary: POSIX line continuation — discard.
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
                            content: Some(content),
                        };
                        self.pos += 1;
                    } else if b == b'\\' {
                        if self.pos + 1 < self.buf.len() {
                            let next = self.buf[self.pos + 1];
                            if matches!(next, b'"' | b'\\') {
                                content.push(next);
                                self.pos += 2;
                            } else if next == b'\n' {
                                // \<LF> in double quotes: POSIX line continuation.
                                self.pos += 2;
                            } else {
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
                            // Backslash at end of chunk — defer to next push.
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tokenizer::TokenKind;

    fn scan_raw(raw: &[u8]) -> Vec<Token> {
        let mut sc = Scanner::new();
        sc.push(raw);
        sc.finalize().collect::<Result<Vec<_>, _>>().unwrap()
    }

    fn word_bytes(token: &Token) -> &[u8] {
        match &token.kind {
            TokenKind::Word(b) => b,
            TokenKind::Pipe => b"|",
        }
    }

    fn scan_err(raw: &[u8]) -> ShellError {
        let mut sc = Scanner::new();
        sc.push(raw);
        sc.finalize().collect::<Result<Vec<_>, _>>().unwrap_err()
    }

    // --- Basic splitting ---

    #[test]
    fn simple_words_split_on_whitespace() {
        let tokens = scan_raw(b"echo hello world");
        assert_eq!(tokens.len(), 3);
        assert_eq!(word_bytes(&tokens[0]), b"echo");
        assert_eq!(word_bytes(&tokens[1]), b"hello");
        assert_eq!(word_bytes(&tokens[2]), b"world");
    }

    #[test]
    fn pipe_emitted_as_pipe_token() {
        let tokens = scan_raw(b"echo foo | cat");
        assert_eq!(tokens.len(), 4);
        assert!(matches!(tokens[2].kind, TokenKind::Pipe));
    }

    #[test]
    fn adjacent_quoted_segments_merge_into_one_token() {
        let tokens = scan_raw(b"pre'mid'post");
        assert_eq!(tokens.len(), 1);
        assert!(matches!(&tokens[0].kind, TokenKind::Word(b) if b.as_ref() == b"premidpost"));
        assert_eq!(tokens[0].span.len(), 12);
    }

    #[test]
    fn single_quote_preserves_spaces() {
        let tokens = scan_raw(b"'hello    world'");
        assert_eq!(tokens.len(), 1);
        assert!(matches!(&tokens[0].kind, TokenKind::Word(b) if b.as_ref() == b"hello    world"));
    }

    #[test]
    fn double_quote_processes_backslash_escapes() {
        let tokens = scan_raw(b"\"A \\\" inside\"");
        assert_eq!(tokens.len(), 1);
        assert!(matches!(&tokens[0].kind, TokenKind::Word(b) if b.as_ref() == b"A \" inside"));
    }

    #[test]
    fn backslash_outside_quotes_escapes_next_char() {
        let tokens = scan_raw(b"three\\ \\ \\ spaces");
        assert_eq!(tokens.len(), 1);
        assert!(matches!(&tokens[0].kind, TokenKind::Word(b) if b.as_ref() == b"three   spaces"));
    }

    #[test]
    fn quoted_pipe_not_a_separator() {
        let tokens = scan_raw(b"'foo | bar'");
        assert_eq!(tokens.len(), 1);
        assert!(matches!(&tokens[0].kind, TokenKind::Word(_)));
    }

    #[test]
    fn unquoted_redirect_span_matches_bytes() {
        let tokens = scan_raw(b">");
        assert_eq!(tokens.len(), 1);
        if let TokenKind::Word(b) = &tokens[0].kind {
            assert_eq!(b.as_ref(), b">");
            assert_eq!(tokens[0].span.len(), b.len());
        }
    }

    #[test]
    fn quoted_redirect_span_exceeds_bytes() {
        let tokens = scan_raw(b"'>'");
        assert_eq!(tokens.len(), 1);
        if let TokenKind::Word(b) = &tokens[0].kind {
            assert_eq!(b.as_ref(), b">");
            assert!(tokens[0].span.len() > b.len());
        }
    }

    #[test]
    fn escaped_redirect_span_exceeds_bytes() {
        let tokens = scan_raw(b"\\>");
        assert_eq!(tokens.len(), 1);
        if let TokenKind::Word(b) = &tokens[0].kind {
            assert_eq!(b.as_ref(), b">");
            assert!(tokens[0].span.len() > b.len());
        }
    }

    #[test]
    fn unclosed_single_quote_returns_error() {
        let err = scan_err(b"echo 'hello");
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
        let err = scan_err(b"echo \"hello");
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
        let err = scan_err(b"echo 'hello");
        if let ShellError::UnclosedQuote { span, .. } = err {
            assert_eq!(span.offset(), 5);
        } else {
            panic!("expected UnclosedQuote");
        }
    }

    #[test]
    fn spans_are_absolute_including_leading_whitespace() {
        let tokens = scan_raw(b"  echo");
        assert_eq!(tokens.len(), 1);
        assert_eq!(tokens[0].span.offset(), 2);
    }

    #[test]
    fn dq_escape_at_chunk_boundary() {
        let mut sc = Scanner::new();
        sc.push(b"\"hello\\");
        assert!(sc.needs_continuation());
        sc.push(b"\"world\"");
        assert!(!sc.needs_continuation());
        let tokens: Vec<_> = sc.finalize().collect::<Result<_, _>>().unwrap();
        assert_eq!(tokens.len(), 1);
        if let TokenKind::Word(b) = &tokens[0].kind {
            assert_eq!(b.as_ref(), b"hello\"world");
        }
    }

    #[test]
    fn dq_non_escape_at_chunk_boundary() {
        let mut sc = Scanner::new();
        sc.push(b"\"hello\\");
        assert!(sc.needs_continuation());
        sc.push(b"xworld\"");
        assert!(!sc.needs_continuation());
        let tokens: Vec<_> = sc.finalize().collect::<Result<_, _>>().unwrap();
        assert_eq!(tokens.len(), 1);
        if let TokenKind::Word(b) = &tokens[0].kind {
            assert_eq!(b.as_ref(), b"hello\\xworld");
        }
    }

    #[test]
    fn incremental_push_detects_multiline_quote() {
        let mut sc = Scanner::new();
        sc.push(b"echo 'hello\n");
        assert!(sc.needs_continuation());
        sc.push(b"world'\n");
        assert!(!sc.needs_continuation());
    }

    #[test]
    fn needs_continuation_true_for_trailing_backslash() {
        let mut sc = Scanner::new();
        sc.push(b"echo foo\\");
        assert!(sc.needs_continuation());
    }

    #[test]
    fn needs_continuation_false_after_trailing_backslash_resolved() {
        let mut sc = Scanner::new();
        sc.push(b"echo foo\\\nbar\n");
        assert!(!sc.needs_continuation());
    }

    // --- Backslash-newline (\<LF>) line continuation ---

    #[test]
    fn backslash_newline_in_neutral_merges_words() {
        let tokens = scan_raw(b"ec\\\nho hello\n");
        assert_eq!(tokens.len(), 2);
        assert!(matches!(&tokens[0].kind, TokenKind::Word(b) if b.as_ref() == b"echo"));
    }

    #[test]
    fn backslash_newline_in_word_continues_accumulation() {
        let tokens = scan_raw(b"echo foo\\\nbar\n");
        assert_eq!(tokens.len(), 2);
        assert!(matches!(&tokens[1].kind, TokenKind::Word(b) if b.as_ref() == b"foobar"));
    }

    #[test]
    fn backslash_newline_multiple_continuations() {
        let tokens = scan_raw(b"echo a\\\nb\\\nc\n");
        assert_eq!(tokens.len(), 2);
        assert!(matches!(&tokens[1].kind, TokenKind::Word(b) if b.as_ref() == b"abc"));
    }

    #[test]
    fn backslash_newline_across_push_boundary() {
        let mut sc = Scanner::new();
        sc.push(b"echo foo\\");
        assert!(sc.needs_continuation());
        sc.push(b"\nbar\n");
        assert!(!sc.needs_continuation());
        let tokens: Vec<_> = sc.finalize().collect::<Result<_, _>>().unwrap();
        assert_eq!(tokens.len(), 2);
        assert!(matches!(&tokens[1].kind, TokenKind::Word(b) if b.as_ref() == b"foobar"));
    }

    #[test]
    fn backslash_newline_inside_single_quote_is_literal() {
        let tokens = scan_raw(b"echo 'foo\\\nbar'\n");
        assert_eq!(tokens.len(), 2);
        assert!(matches!(&tokens[1].kind, TokenKind::Word(b) if b.as_ref() == b"foo\\\nbar"));
    }

    // --- Double-quote \<LF> (POSIX line continuation inside double quotes) ---

    #[test]
    fn dq_backslash_newline_is_continuation() {
        let tokens = scan_raw(b"\"foo\\\nbar\"\n");
        assert_eq!(tokens.len(), 1);
        assert!(matches!(&tokens[0].kind, TokenKind::Word(b) if b.as_ref() == b"foobar"));
    }

    #[test]
    fn dq_backslash_newline_across_push_boundary() {
        let mut sc = Scanner::new();
        sc.push(b"\"foo\\");
        assert!(sc.needs_continuation());
        sc.push(b"\nbar\"");
        sc.finalize()
            .collect::<Result<Vec<_>, _>>()
            .map(|tokens| {
                assert_eq!(tokens.len(), 1);
                assert!(matches!(&tokens[0].kind, TokenKind::Word(b) if b.as_ref() == b"foobar"));
            })
            .unwrap();
    }

    #[test]
    fn dq_non_newline_after_pending_bs_still_literal() {
        let mut sc = Scanner::new();
        sc.push(b"\"foo\\");
        sc.push(b"xbar\"");
        let tokens: Vec<_> = sc.finalize().collect::<Result<_, _>>().unwrap();
        assert!(matches!(&tokens[0].kind, TokenKind::Word(b) if b.as_ref() == b"foo\\xbar"));
    }

    // --- Zero-copy invariant ---

    #[test]
    fn unescaped_word_is_zero_copy_slice() {
        let mut sc = Scanner::new();
        sc.push(b"hello");
        let tokenizer = sc.finalize();
        let raw = tokenizer.raw_bytes_shared();
        let tokens: Vec<_> = tokenizer.collect::<Result<_, _>>().unwrap();
        if let TokenKind::Word(b) = &tokens[0].kind {
            // Zero-copy: the word's Bytes should point into the same allocation.
            assert_eq!(b.as_ptr(), raw.slice(0..5).as_ptr());
        }
    }
}
