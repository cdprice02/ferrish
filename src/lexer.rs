use std::fmt;

use bytes::Bytes;
use miette::SourceSpan;

use crate::error::ShellError;
use crate::input::{Input, InputSource};

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

/// Observable state of the lexer at any point during iteration.
///
/// Carries all flags that drive the state machine so callers can query
/// completeness without re-running a full lex pass. New state fields (nesting
/// depth, heredoc, etc.) are added here as the shell grammar grows.
#[derive(Clone, Debug, Default)]
pub struct LexerState {
    /// The quote that is currently open, if any.
    pub open_quote: Option<QuoteKind>,
    /// Set when `scan_bytes` ends with a `\` inside a double-quoted string.
    /// On the next call the first byte is checked: if it is `"` or `\`, both
    /// bytes form an escape and the byte is skipped; otherwise the `\` was
    /// literal and the byte is processed normally.
    pending_dq_backslash: bool,
}

impl LexerState {
    /// Returns `true` when the lexer is in a terminal state — no unclosed
    /// quotes or other incomplete constructs. Used by the REPL input collector
    /// to decide whether to prompt for more input.
    pub fn is_complete(&self) -> bool {
        self.open_quote.is_none()
    }

    /// Returns `true` when accumulated input is incomplete and more is needed.
    pub fn needs_continuation(&self) -> bool {
        self.open_quote.is_some()
    }

    /// Update quote state by scanning `bytes` without building tokens.
    ///
    /// Processes each byte and updates `open_quote` to reflect quote opens and
    /// closes. May be called repeatedly on successive chunks; callers are
    /// responsible for passing only new bytes on each call rather than the full
    /// accumulated buffer.
    ///
    /// Escape and quote semantics mirror the `Lexer` iterator in `lexer.rs`.
    /// Keep both in sync when extending the grammar.
    pub fn scan_bytes(&mut self, bytes: &[u8]) {
        let mut i = 0;
        while i < bytes.len() {
            match self.open_quote {
                Some(QuoteKind::Single) => {
                    if bytes[i] == b'\'' {
                        self.open_quote = None;
                    }
                    i += 1;
                }
                Some(QuoteKind::Double) => {
                    // Consume a backslash deferred from the previous chunk boundary.
                    if self.pending_dq_backslash {
                        self.pending_dq_backslash = false;
                        if matches!(bytes[i], b'"' | b'\\') {
                            i += 1; // skip the escaped byte
                            continue;
                        }
                        // \ was not an escape — fall through to process bytes[i] normally.
                    }
                    if bytes[i] == b'\\' {
                        if i + 1 < bytes.len() && matches!(bytes[i + 1], b'"' | b'\\') {
                            i += 2;
                            continue;
                        } else if i + 1 == bytes.len() {
                            // Chunk ends on \; defer the escape check to the next call.
                            self.pending_dq_backslash = true;
                            i += 1;
                            continue;
                        }
                    }
                    if bytes[i] == b'"' {
                        self.open_quote = None;
                    }
                    i += 1;
                }
                None => {
                    match bytes[i] {
                        b'\'' => self.open_quote = Some(QuoteKind::Single),
                        b'"' => self.open_quote = Some(QuoteKind::Double),
                        b'\\' if i + 1 < bytes.len() => i += 1,
                        _ => {}
                    }
                    i += 1;
                }
            }
        }
    }
}

/// The syntactic kind of a lexed token.
#[derive(Debug, Clone)]
pub enum TokenKind {
    /// An unquoted word segment. May include redirect operator spellings such as
    /// `>` or `2>>`; the parser classifies those.
    Word(Bytes),
    /// Content of a single-quoted string (`'...'`). The span includes both quote
    /// characters; the content bytes have the quotes stripped.
    SingleQuoted(Bytes),
    /// Content of a double-quoted string (`"..."`), with `\\` and `\"` escapes
    /// resolved. The span includes both quote characters.
    DoubleQuoted(Bytes),
    /// An unquoted `|` character — always a pipeline separator.
    Pipe,
}

/// A single token produced by the lexer.
#[derive(Debug, Clone)]
pub struct Token {
    /// The syntactic content and kind of this token.
    pub kind: TokenKind,
    /// Byte offset and length of this token in the raw input.
    pub span: SourceSpan,
    /// The raw input this token was lexed from.
    pub src: InputSource,
}

/// Lazy token iterator produced by [`lex`].
///
/// Yields [`Token`] values one at a time as the parser pulls them. An unclosed
/// quote is reported as a final `Err` item; after that the iterator returns
/// `None`. The lexer's current state is always accessible via [`Lexer::state`].
///
/// Unescaped single-quoted strings and words/double-quoted strings without
/// escape sequences are emitted as zero-copy slices of the original input
/// buffer. Only tokens with processed escape sequences require a heap allocation.
pub struct Lexer<'a> {
    input: &'a Input,
    buffer: &'a [u8],
    abs_start: usize,
    src: InputSource,
    i: usize,
    state: LexerState,

    // Unquoted word accumulation.
    word_start_abs: Option<usize>,
    word_buf: Vec<u8>,
    word_has_escape: bool,

    // Double-quoted string accumulation.
    dq_buf: Vec<u8>,
    dq_has_escape: bool,

    // Buffer index of the opening quote character.
    quote_start_i: usize,

    done: bool,
}

impl<'a> Lexer<'a> {
    /// The lexer's current state — readable at any point during iteration.
    pub fn state(&self) -> &LexerState {
        &self.state
    }

    /// Emit and clear any pending unquoted word token.
    ///
    /// `end_abs` is the absolute byte offset of the first byte past the word.
    /// Returns `None` when no word is in progress.
    fn flush_word(&mut self, end_abs: usize) -> Option<Token> {
        let start = self.word_start_abs.take()?;
        let bytes = if self.word_has_escape {
            self.word_has_escape = false;
            Bytes::from(std::mem::take(&mut self.word_buf))
        } else {
            self.word_buf.clear();
            self.input.raw_slice(start..end_abs)
        };
        let span = SourceSpan::from((start, end_abs - start));
        Some(Token {
            kind: TokenKind::Word(bytes),
            span,
            src: self.src.clone(),
        })
    }
}

impl<'a> Iterator for Lexer<'a> {
    type Item = Result<Token, ShellError>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.done {
            return None;
        }

        loop {
            if self.i >= self.buffer.len() {
                self.done = true;

                if let Some(ref kind) = self.state.open_quote.clone() {
                    return Some(Err(ShellError::UnclosedQuote {
                        style: kind.clone(),
                        span: SourceSpan::from((self.abs_start + self.quote_start_i, 1)),
                        src: self.src.clone(),
                    }));
                }

                return self.flush_word(self.abs_start + self.buffer.len()).map(Ok);
            }

            let byte = self.buffer[self.i];

            // --- Inside single quote ---
            if self.state.open_quote == Some(QuoteKind::Single) {
                if byte == b'\'' {
                    let content_start = self.abs_start + self.quote_start_i + 1;
                    let content_end = self.abs_start + self.i;
                    let bytes = self.input.raw_slice(content_start..content_end);
                    let span = SourceSpan::from((
                        self.abs_start + self.quote_start_i,
                        self.i - self.quote_start_i + 1,
                    ));
                    self.state.open_quote = None;
                    self.i += 1;
                    return Some(Ok(Token {
                        kind: TokenKind::SingleQuoted(bytes),
                        span,
                        src: self.src.clone(),
                    }));
                }
                self.i += 1;
                continue;
            }

            // --- Inside double quote ---
            if self.state.open_quote == Some(QuoteKind::Double) {
                if byte == b'"' {
                    let bytes = if self.dq_has_escape {
                        self.dq_has_escape = false;
                        Bytes::from(std::mem::take(&mut self.dq_buf))
                    } else {
                        self.dq_buf.clear();
                        let content_start = self.abs_start + self.quote_start_i + 1;
                        let content_end = self.abs_start + self.i;
                        self.input.raw_slice(content_start..content_end)
                    };
                    let span = SourceSpan::from((
                        self.abs_start + self.quote_start_i,
                        self.i - self.quote_start_i + 1,
                    ));
                    self.state.open_quote = None;
                    self.i += 1;
                    return Some(Ok(Token {
                        kind: TokenKind::DoubleQuoted(bytes),
                        span,
                        src: self.src.clone(),
                    }));
                }

                if byte == b'\\' && self.i + 1 < self.buffer.len() {
                    let next = self.buffer[self.i + 1];
                    if next == b'"' || next == b'\\' {
                        if !self.dq_has_escape {
                            // Retroactively populate dq_buf with content scanned so far.
                            let content_start = self.quote_start_i + 1;
                            self.dq_buf
                                .extend_from_slice(&self.buffer[content_start..self.i]);
                            self.dq_has_escape = true;
                        }
                        self.dq_buf.push(next);
                        self.i += 2;
                        continue;
                    }
                }

                if self.dq_has_escape {
                    self.dq_buf.push(byte);
                }
                self.i += 1;
                continue;
            }

            // --- Outside any quote ---
            match byte {
                b'\'' => {
                    if let Some(token) = self.flush_word(self.abs_start + self.i) {
                        // Return the pending word; the opening quote is recorded and i
                        // advances past it so the next call resumes inside the quote.
                        self.quote_start_i = self.i;
                        self.state.open_quote = Some(QuoteKind::Single);
                        self.i += 1;
                        return Some(Ok(token));
                    }
                    self.quote_start_i = self.i;
                    self.state.open_quote = Some(QuoteKind::Single);
                    self.i += 1;
                }

                b'"' => {
                    if let Some(token) = self.flush_word(self.abs_start + self.i) {
                        self.quote_start_i = self.i;
                        self.state.open_quote = Some(QuoteKind::Double);
                        self.dq_has_escape = false;
                        self.dq_buf.clear();
                        self.i += 1;
                        return Some(Ok(token));
                    }
                    self.quote_start_i = self.i;
                    self.state.open_quote = Some(QuoteKind::Double);
                    self.dq_has_escape = false;
                    self.dq_buf.clear();
                    self.i += 1;
                }

                b'\\' => {
                    if self.word_start_abs.is_none() {
                        self.word_start_abs = Some(self.abs_start + self.i);
                    }
                    if self.i + 1 < self.buffer.len() {
                        if !self.word_has_escape {
                            // Retroactively populate word_buf with bytes scanned so far.
                            let word_rel_start = self.word_start_abs.unwrap() - self.abs_start;
                            self.word_buf
                                .extend_from_slice(&self.buffer[word_rel_start..self.i]);
                            self.word_has_escape = true;
                        }
                        self.word_buf.push(self.buffer[self.i + 1]);
                        self.i += 2;
                    } else {
                        // Trailing backslash — consumed silently. Switch to buffered mode
                        // so the raw_slice path does not include the backslash.
                        if !self.word_has_escape {
                            let word_rel_start = self.word_start_abs.unwrap() - self.abs_start;
                            self.word_buf
                                .extend_from_slice(&self.buffer[word_rel_start..self.i]);
                            self.word_has_escape = true;
                        }
                        self.i += 1;
                    }
                }

                b if b.is_ascii_whitespace() => {
                    if let Some(token) = self.flush_word(self.abs_start + self.i) {
                        self.i += 1;
                        return Some(Ok(token));
                    }
                    self.i += 1;
                }

                b'|' => {
                    if let Some(token) = self.flush_word(self.abs_start + self.i) {
                        // Return the pending word; i still points at '|' so the pipe
                        // token is emitted on the next call.
                        return Some(Ok(token));
                    }
                    let span = SourceSpan::from((self.abs_start + self.i, 1));
                    self.i += 1;
                    return Some(Ok(Token {
                        kind: TokenKind::Pipe,
                        span,
                        src: self.src.clone(),
                    }));
                }

                _ => {
                    if self.word_start_abs.is_none() {
                        self.word_start_abs = Some(self.abs_start + self.i);
                    }
                    if self.word_has_escape {
                        self.word_buf.push(byte);
                    }
                    self.i += 1;
                }
            }
        }
    }
}

/// Lex `input` into a lazy token iterator.
///
/// Tokens are produced on demand as the caller pulls from the iterator.
/// The final state (e.g. an unclosed quote) is exposed via [`Lexer::state`]
/// after the iterator is exhausted.
pub fn lex(input: &Input) -> Lexer<'_> {
    Lexer {
        input,
        buffer: input.trimmed_bytes(),
        abs_start: input.leading_offset(),
        src: input.as_source(),
        i: 0,
        state: LexerState::default(),
        word_start_abs: None,
        word_buf: Vec::new(),
        word_has_escape: false,
        dq_buf: Vec::new(),
        dq_has_escape: false,
        quote_start_i: 0,
        done: false,
    }
}

/// Returns `true` when `bytes` ends in an incomplete construct (e.g. an
/// unclosed quote) that requires more input before a complete command can
/// be parsed.
pub fn needs_continuation(bytes: &[u8]) -> bool {
    let mut state = LexerState::default();
    state.scan_bytes(bytes);
    state.needs_continuation()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lex_raw(raw: &[u8]) -> Vec<Token> {
        lex(&Input::new(raw))
            .collect::<Result<Vec<_>, _>>()
            .unwrap()
    }

    fn word_bytes(token: &Token) -> &[u8] {
        match &token.kind {
            TokenKind::Word(b) | TokenKind::SingleQuoted(b) | TokenKind::DoubleQuoted(b) => b,
            TokenKind::Pipe => b"|",
        }
    }

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
    fn adjacent_quoted_segments_are_separate_tokens() {
        let tokens = lex_raw(b"pre'mid'post");
        assert_eq!(tokens.len(), 3);
        assert!(matches!(&tokens[0].kind, TokenKind::Word(b) if b.as_ref() == b"pre"));
        assert!(matches!(&tokens[1].kind, TokenKind::SingleQuoted(b) if b.as_ref() == b"mid"));
        assert!(matches!(&tokens[2].kind, TokenKind::Word(b) if b.as_ref() == b"post"));
        let t0_end = tokens[0].span.offset() + tokens[0].span.len();
        assert_eq!(t0_end, tokens[1].span.offset());
        let t1_end = tokens[1].span.offset() + tokens[1].span.len();
        assert_eq!(t1_end, tokens[2].span.offset());
    }

    #[test]
    fn single_quote_preserves_spaces() {
        let tokens = lex_raw(b"'hello    world'");
        assert_eq!(tokens.len(), 1);
        assert!(
            matches!(&tokens[0].kind, TokenKind::SingleQuoted(b) if b.as_ref() == b"hello    world")
        );
    }

    #[test]
    fn double_quote_processes_backslash_escapes() {
        let tokens = lex_raw(b"\"A \\\" inside\"");
        assert_eq!(tokens.len(), 1);
        assert!(
            matches!(&tokens[0].kind, TokenKind::DoubleQuoted(b) if b.as_ref() == b"A \" inside")
        );
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
        assert!(matches!(&tokens[0].kind, TokenKind::SingleQuoted(_)));
    }

    #[test]
    fn unclosed_single_quote_returns_error() {
        let err = lex(&Input::new(b"echo 'hello"))
            .collect::<Result<Vec<_>, _>>()
            .unwrap_err();
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
        let err = lex(&Input::new(b"echo \"hello"))
            .collect::<Result<Vec<_>, _>>()
            .unwrap_err();
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
        let err = lex(&Input::new(b"echo 'hello"))
            .collect::<Result<Vec<_>, _>>()
            .unwrap_err();
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
    fn scan_bytes_dq_escape_at_chunk_boundary() {
        // "hello\" split across two chunks — the \" escape must be honoured even
        // though \ and " arrive in separate scan_bytes calls.
        let mut state = LexerState::default();
        state.scan_bytes(b"\"hello\\"); // opens double quote, ends mid-escape
        assert!(state.needs_continuation());
        state.scan_bytes(b"\"world\""); // \" consumed as escape, final " closes
        assert!(!state.needs_continuation());
    }

    #[test]
    fn scan_bytes_dq_non_escape_at_chunk_boundary() {
        // \ followed by a non-escapable char (x) across a boundary — \ is literal.
        let mut state = LexerState::default();
        state.scan_bytes(b"\"hello\\");
        assert!(state.needs_continuation());
        state.scan_bytes(b"xworld\""); // x is not escapable; " closes the quote
        assert!(!state.needs_continuation());
    }

    #[test]
    fn scan_bytes_incremental_detects_multiline_quote() {
        let mut state = LexerState::default();
        state.scan_bytes(b"echo 'hello\n");
        assert!(state.needs_continuation());
        state.scan_bytes(b"world'\n");
        assert!(!state.needs_continuation());
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

    #[test]
    fn single_quoted_content_is_zero_copy_slice() {
        let raw = b"'hello world'";
        let input = Input::new(raw);
        let tokens = lex(&input).collect::<Result<Vec<_>, _>>().unwrap();
        assert_eq!(tokens.len(), 1);
        if let TokenKind::SingleQuoted(b) = &tokens[0].kind {
            assert_eq!(b.as_ref(), b"hello world");
        } else {
            panic!("expected SingleQuoted");
        }
    }

    #[test]
    fn unescaped_word_is_zero_copy_slice() {
        let raw = b"hello";
        let input = Input::new(raw);
        let tokens = lex(&input).collect::<Result<Vec<_>, _>>().unwrap();
        assert_eq!(tokens.len(), 1);
        if let TokenKind::Word(b) = &tokens[0].kind {
            assert_eq!(b.as_ref(), b"hello");
        } else {
            panic!("expected Word");
        }
    }
}
