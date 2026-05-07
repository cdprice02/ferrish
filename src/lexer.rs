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
    /// Escape and quote semantics mirror the tokenizer. Keep both in sync when
    /// extending the grammar.
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
    /// A shell word — the result of quote stripping and escape processing on
    /// one contiguous word in the input. All quoting styles (unquoted,
    /// single-quoted, double-quoted, backslash-escaped, and any combination)
    /// produce a `Word` token; the span covers the full raw extent of the word
    /// including any quote characters.
    Word(Bytes),
    /// An unquoted `|` character — always a pipeline separator.
    Pipe,
}

/// A single token produced by the lexer.
#[derive(Debug, Clone)]
pub struct Token {
    /// The syntactic content and kind of this token.
    pub kind: TokenKind,
    /// Byte offset and length of this token in the raw input. For `Word`
    /// tokens the span covers the raw extent including quote characters, so
    /// `span.len()` may exceed the length of the processed content bytes.
    pub span: SourceSpan,
    /// The raw input this token was lexed from.
    pub src: InputSource,
}

/// Eager token iterator produced by [`lex`].
///
/// All tokens are computed up front when [`lex`] is called. An unclosed quote
/// is reported as a final `Err` item; the iterator returns `None` after that.
pub struct Lexer<'a> {
    tokens: std::vec::IntoIter<Result<Token, ShellError>>,
    _lifetime: std::marker::PhantomData<&'a ()>,
}

impl Iterator for Lexer<'_> {
    type Item = Result<Token, ShellError>;

    fn next(&mut self) -> Option<Self::Item> {
        self.tokens.next()
    }
}

/// Lex `input` into an eager token iterator.
///
/// Uses [`shlex`] for POSIX word splitting and escape processing. An unclosed
/// quote is detected up front and returned as the sole `Err` item.
pub fn lex(input: &Input) -> Lexer<'_> {
    Lexer {
        tokens: tokenize(input).into_iter(),
        _lifetime: std::marker::PhantomData,
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

/// Build the complete token list for `input`.
fn tokenize(input: &Input) -> Vec<Result<Token, ShellError>> {
    let bytes = input.trimmed_bytes();
    let abs_start = input.leading_offset();
    let src = input.as_source();

    if needs_continuation(bytes) {
        let (style, offset) = find_unclosed_quote(bytes);
        return vec![Err(ShellError::UnclosedQuote {
            style,
            span: SourceSpan::from((abs_start + offset, 1)),
            src,
        })];
    }

    let mut tokens: Vec<Result<Token, ShellError>> = Vec::new();
    let pipe_positions = find_pipe_positions(bytes);
    let mut seg_start = 0;

    for &pipe_pos in &pipe_positions {
        let segment = &bytes[seg_start..pipe_pos];
        emit_segment_tokens(segment, abs_start + seg_start, &src, &mut tokens);
        tokens.push(Ok(Token {
            kind: TokenKind::Pipe,
            span: SourceSpan::from((abs_start + pipe_pos, 1)),
            src: src.clone(),
        }));
        seg_start = pipe_pos + 1;
    }

    let segment = &bytes[seg_start..];
    emit_segment_tokens(segment, abs_start + seg_start, &src, &mut tokens);

    tokens
}

/// Tokenize one pipe-separated segment into `Word` tokens and append to `out`.
fn emit_segment_tokens(
    segment: &[u8],
    abs_seg_start: usize,
    src: &InputSource,
    out: &mut Vec<Result<Token, ShellError>>,
) {
    let spans = compute_word_spans(segment);
    // shlex errors on a lone trailing backslash (POSIX: incomplete escape).
    // Strip it before lexing to match the shell convention of consuming it silently.
    // An odd count of trailing backslashes means the last one is unescaped.
    let trailing_bs = segment.iter().rev().take_while(|&&b| b == b'\\').count();
    let shlex_input = if trailing_bs % 2 == 1 {
        &segment[..segment.len() - 1]
    } else {
        segment
    };
    let words: Vec<Vec<u8>> = shlex::bytes::Shlex::new(shlex_input).collect();
    for (word_bytes, (rel_start, raw_len)) in words.into_iter().zip(spans) {
        out.push(Ok(Token {
            kind: TokenKind::Word(Bytes::from(word_bytes)),
            span: SourceSpan::from((abs_seg_start + rel_start, raw_len)),
            src: src.clone(),
        }));
    }
}

/// Find the byte offsets of all unquoted `|` characters in `bytes`.
fn find_pipe_positions(bytes: &[u8]) -> Vec<usize> {
    let mut positions = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'\'' => {
                i += 1;
                while i < bytes.len() && bytes[i] != b'\'' {
                    i += 1;
                }
                if i < bytes.len() {
                    i += 1;
                }
            }
            b'"' => {
                i += 1;
                while i < bytes.len() {
                    if bytes[i] == b'"' {
                        i += 1;
                        break;
                    }
                    if bytes[i] == b'\\'
                        && i + 1 < bytes.len()
                        && matches!(bytes[i + 1], b'"' | b'\\')
                    {
                        i += 2;
                        continue;
                    }
                    i += 1;
                }
            }
            b'\\' if i + 1 < bytes.len() => i += 2,
            b'|' => {
                positions.push(i);
                i += 1;
            }
            _ => i += 1,
        }
    }
    positions
}

/// Find the kind and position of the first unclosed quote in `bytes`.
///
/// Called only after [`needs_continuation`] has confirmed one exists.
fn find_unclosed_quote(bytes: &[u8]) -> (QuoteKind, usize) {
    let mut i = 0;
    let mut open: Option<(QuoteKind, usize)> = None;
    while i < bytes.len() {
        match &open {
            None => match bytes[i] {
                b'\'' => {
                    open = Some((QuoteKind::Single, i));
                    i += 1;
                }
                b'"' => {
                    open = Some((QuoteKind::Double, i));
                    i += 1;
                }
                b'\\' if i + 1 < bytes.len() => i += 2,
                _ => i += 1,
            },
            Some((QuoteKind::Single, _)) => {
                if bytes[i] == b'\'' {
                    open = None;
                }
                i += 1;
            }
            Some((QuoteKind::Double, _)) => {
                if bytes[i] == b'"' {
                    open = None;
                    i += 1;
                    continue;
                }
                if bytes[i] == b'\\' && i + 1 < bytes.len() && matches!(bytes[i + 1], b'"' | b'\\')
                {
                    i += 2;
                    continue;
                }
                i += 1;
            }
        }
    }
    // Caller guarantees needs_continuation() is true, so open is Some.
    let (kind, pos) = open.unwrap();
    (kind, pos)
}

/// Compute the (relative_start, raw_len) span of each word in `segment`.
///
/// Mirrors the word-boundary logic that [`shlex`] uses, so the nth span
/// corresponds to the nth word shlex yields. The raw length includes quote
/// characters and backslashes; `span.len()` may therefore exceed the length
/// of the processed content bytes.
fn compute_word_spans(segment: &[u8]) -> Vec<(usize, usize)> {
    let mut spans = Vec::new();
    let mut i = 0;
    while i < segment.len() {
        while i < segment.len() && matches!(segment[i], b' ' | b'\t' | b'\n') {
            i += 1;
        }
        if i >= segment.len() {
            break;
        }
        let word_start = i;
        while i < segment.len() {
            match segment[i] {
                b' ' | b'\t' | b'\n' => break,
                b'\'' => {
                    i += 1;
                    while i < segment.len() && segment[i] != b'\'' {
                        i += 1;
                    }
                    if i < segment.len() {
                        i += 1;
                    }
                }
                b'"' => {
                    i += 1;
                    while i < segment.len() {
                        if segment[i] == b'"' {
                            i += 1;
                            break;
                        }
                        if segment[i] == b'\\'
                            && i + 1 < segment.len()
                            && matches!(segment[i + 1], b'"' | b'\\')
                        {
                            i += 2;
                            continue;
                        }
                        i += 1;
                    }
                }
                b'\\' if i + 1 < segment.len() => i += 2,
                _ => i += 1,
            }
        }
        if i > word_start {
            spans.push((word_start, i - word_start));
        }
    }
    spans
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
            TokenKind::Word(b) => b,
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
    fn adjacent_quoted_segments_merged_into_one_word() {
        // shlex merges pre'mid'post into a single word
        let tokens = lex_raw(b"pre'mid'post");
        assert_eq!(tokens.len(), 1);
        assert!(matches!(&tokens[0].kind, TokenKind::Word(b) if b.as_ref() == b"premidpost"));
        assert_eq!(tokens[0].span.offset(), 0);
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
    fn word_content_is_correct() {
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
