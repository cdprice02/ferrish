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
    /// Byte offset and length of this token in the raw input line.
    pub span: SourceSpan,
    /// The raw input line this token was lexed from.
    pub src: InputSource,
}

/// Lex a raw input line into a flat sequence of tokens.
///
/// Recognises single-quoted strings, double-quoted strings (with `\\`/`\"`
/// escapes), backslash escapes outside quotes, and unquoted `|` as a pipeline
/// separator. Whitespace delimits tokens but is not itself emitted.
///
/// All token spans are absolute byte offsets into the original (un-trimmed)
/// input, so diagnostics render correctly without any caller-side adjustment.
///
/// # Errors
///
/// Returns [`ShellError::UnclosedQuote`] if a quote is opened but never closed.
pub fn lex(input: &Input) -> Result<Vec<Token>, ShellError> {
    let buffer = input.trimmed_bytes();
    let abs_start = input.leading_offset();
    let src = input.as_source();

    let mut tokens: Vec<Token> = Vec::new();
    // Shared accumulation buffer — flushed on each Word/Quoted token emission.
    let mut word_buf: Vec<u8> = Vec::new();
    // Absolute start of the current Word token; None when no Word is in progress.
    let mut word_start_abs: Option<usize> = None;
    let mut in_single_quote = false;
    let mut in_double_quote = false;
    // Buffer index of the opening quote character, for span and error reporting.
    let mut quote_start_i: usize = 0;

    let mut i = 0;
    while i < buffer.len() {
        let byte = buffer[i];

        if in_single_quote {
            if byte == b'\'' {
                let span = SourceSpan::from((abs_start + quote_start_i, i - quote_start_i + 1));
                tokens.push(Token {
                    kind: TokenKind::SingleQuoted(Bytes::from(std::mem::take(&mut word_buf))),
                    span,
                    src: src.clone(),
                });
                in_single_quote = false;
            } else {
                word_buf.push(byte);
            }
        } else if in_double_quote {
            if byte == b'"' {
                let span = SourceSpan::from((abs_start + quote_start_i, i - quote_start_i + 1));
                tokens.push(Token {
                    kind: TokenKind::DoubleQuoted(Bytes::from(std::mem::take(&mut word_buf))),
                    span,
                    src: src.clone(),
                });
                in_double_quote = false;
            } else if byte == b'\\' && i + 1 < buffer.len() {
                let next = buffer[i + 1];
                if next == b'"' || next == b'\\' {
                    i += 1;
                    word_buf.push(next);
                } else {
                    word_buf.push(b'\\');
                }
            } else {
                word_buf.push(byte);
            }
        } else if byte == b'\'' {
            flush_word(
                &mut word_buf,
                &mut word_start_abs,
                abs_start + i,
                &src,
                &mut tokens,
            );
            quote_start_i = i;
            in_single_quote = true;
        } else if byte == b'"' {
            flush_word(
                &mut word_buf,
                &mut word_start_abs,
                abs_start + i,
                &src,
                &mut tokens,
            );
            quote_start_i = i;
            in_double_quote = true;
        } else if byte == b'\\' {
            if word_start_abs.is_none() {
                word_start_abs = Some(abs_start + i);
            }
            if i + 1 < buffer.len() {
                i += 1;
                word_buf.push(buffer[i]);
            }
        } else if byte.is_ascii_whitespace() {
            flush_word(
                &mut word_buf,
                &mut word_start_abs,
                abs_start + i,
                &src,
                &mut tokens,
            );
        } else if byte == b'|' {
            flush_word(
                &mut word_buf,
                &mut word_start_abs,
                abs_start + i,
                &src,
                &mut tokens,
            );
            tokens.push(Token {
                kind: TokenKind::Pipe,
                span: SourceSpan::from((abs_start + i, 1)),
                src: src.clone(),
            });
        } else {
            if word_start_abs.is_none() {
                word_start_abs = Some(abs_start + i);
            }
            word_buf.push(byte);
        }

        i += 1;
    }

    if in_single_quote {
        return Err(ShellError::UnclosedQuote {
            style: QuoteKind::Single,
            span: SourceSpan::from((abs_start + quote_start_i, 1)),
            src,
        });
    }
    if in_double_quote {
        return Err(ShellError::UnclosedQuote {
            style: QuoteKind::Double,
            span: SourceSpan::from((abs_start + quote_start_i, 1)),
            src,
        });
    }

    flush_word(
        &mut word_buf,
        &mut word_start_abs,
        abs_start + buffer.len(),
        &src,
        &mut tokens,
    );

    Ok(tokens)
}

fn flush_word(
    buf: &mut Vec<u8>,
    start_abs: &mut Option<usize>,
    end_abs: usize,
    src: &InputSource,
    tokens: &mut Vec<Token>,
) {
    if let Some(start) = start_abs.take() {
        let span = SourceSpan::from((start, end_abs - start));
        tokens.push(Token {
            kind: TokenKind::Word(Bytes::from(std::mem::take(buf))),
            span,
            src: src.clone(),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lex_raw(raw: &[u8]) -> Vec<Token> {
        lex(&Input::new(raw)).unwrap()
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
        // Spans are adjacent
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
        let err = lex(&Input::new(b"echo 'hello")).unwrap_err();
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
        let err = lex(&Input::new(b"echo \"hello")).unwrap_err();
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
        let err = lex(&Input::new(b"echo 'hello")).unwrap_err();
        if let ShellError::UnclosedQuote { span, .. } = err {
            assert_eq!(span.offset(), 5);
        } else {
            panic!("expected UnclosedQuote");
        }
    }

    #[test]
    fn spans_are_absolute_including_leading_whitespace() {
        // Input with 2 bytes of leading whitespace; leading_offset = 2.
        // The word "echo" starts at absolute position 2.
        let tokens = lex_raw(b"  echo");
        assert_eq!(tokens.len(), 1);
        assert_eq!(tokens[0].span.offset(), 2);
    }
}
