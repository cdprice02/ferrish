use crate::error::ShellError;
use crate::lexer::{Lexer, Token, TokenKind};
use crate::redirect::{RedirectMode, StderrRedirection, StdoutRedirection};
use crate::word::Word;
use std::iter::Peekable;

/// The command position of a pipeline stage before resolution.
#[derive(Debug)]
pub struct UnresolvedCommand {
    /// The word representing the command.
    pub word: Word,
}

/// One stage of a pipeline before command resolution.
#[derive(Debug)]
pub struct UnresolvedStage {
    /// The command to run.
    pub command: UnresolvedCommand,
    /// Arguments to pass to the command.
    pub args: Vec<Word>,
    /// Optional stdout redirection for this stage.
    pub stdout_redirect: Option<StdoutRedirection>,
    /// Optional stderr redirection for this stage.
    pub stderr_redirect: Option<StderrRedirection>,
}

/// Observable state of the parser at any point during iteration.
#[derive(Clone, Debug, Default)]
pub struct ParserState {
    /// Whether the segment currently being accumulated has received at least
    /// one token.
    pub segment_has_tokens: bool,
}

/// Lazy pipeline iterator backed by a [`Lexer`].
///
/// Yields one [`UnresolvedStage`] per `|`-separated segment. An empty
/// segment, unclosed quote, or other lex error is reported as a final
/// `Err` item; after that the iterator returns `None`.
pub struct Parser {
    lexer: Peekable<Lexer>,
    state: ParserState,
    pending: Vec<Token>,
    last_pipe: Option<Token>,
    done: bool,
}

impl Parser {
    /// Create a parser that pulls tokens from `lexer`.
    ///
    /// `lexer` must have been finalized by the caller before construction so
    /// that any trailing word and unclosed-quote errors are already queued.
    pub fn new(lexer: Lexer) -> Self {
        Self {
            lexer: lexer.peekable(),
            state: ParserState::default(),
            pending: Vec::new(),
            last_pipe: None,
            done: false,
        }
    }

    /// The parser's current state.
    pub fn state(&self) -> &ParserState {
        &self.state
    }
}

impl Iterator for Parser {
    type Item = Result<UnresolvedStage, ShellError>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.done {
            return None;
        }

        loop {
            match self.lexer.next() {
                None => {
                    self.done = true;
                    if !self.state.segment_has_tokens {
                        if let Some(pipe) = self.last_pipe.take() {
                            return Some(Err(ShellError::EmptyPipelineSegment { span: pipe.span }));
                        }
                        return None;
                    }
                    let tokens = std::mem::take(&mut self.pending);
                    return Some(build_stage(tokens));
                }
                Some(Err(e)) => {
                    self.done = true;
                    return Some(Err(e));
                }
                Some(Ok(token)) => {
                    if matches!(token.kind, TokenKind::Pipe) {
                        if !self.state.segment_has_tokens {
                            self.done = true;
                            return Some(Err(ShellError::EmptyPipelineSegment {
                                span: token.span,
                            }));
                        }
                        let tokens = std::mem::take(&mut self.pending);
                        self.state.segment_has_tokens = false;
                        self.last_pipe = Some(token);
                        return Some(build_stage(tokens));
                    } else {
                        self.state.segment_has_tokens = true;
                        self.pending.push(token);
                    }
                }
            }
        }
    }
}

/// Map a non-Pipe token to a [`Word`].
fn token_to_word(token: Token) -> Word {
    match token.kind {
        TokenKind::Word(bytes) => Word {
            bytes,
            span: token.span,
        },
        TokenKind::Pipe => unreachable!("pipe tokens stripped before build_stage"),
    }
}

/// Build a pipeline stage from an accumulated segment's tokens.
fn build_stage(tokens: Vec<Token>) -> Result<UnresolvedStage, ShellError> {
    let mut words = tokens.into_iter().map(token_to_word);
    let command_word = words.next().expect("empty segment checked above");
    let rest: Vec<Word> = words.collect();
    let (args, stdout_redirect, stderr_redirect) = extract_redirects(rest);
    Ok(UnresolvedStage {
        command: UnresolvedCommand { word: command_word },
        args,
        stdout_redirect,
        stderr_redirect,
    })
}

/// Scan `args` for redirect operators and return the remaining args plus
/// any stdout and stderr redirections.
///
/// An arg is a redirect operator only when `span.len() == bytes.len()` —
/// meaning pure unquoted, unescaped bytes — and those bytes match a known
/// operator spelling. Mixed-quoted or backslash-escaped look-alikes have
/// `span.len() > bytes.len()` and are never treated as operators.
///
/// "Last redirect wins" semantics per fd. A trailing operator with no
/// following filename is kept as a literal argument.
fn extract_redirects(
    args: Vec<Word>,
) -> (
    Vec<Word>,
    Option<StdoutRedirection>,
    Option<StderrRedirection>,
) {
    let mut out: Vec<Word> = Vec::new();
    let mut stdout_redirect: Option<StdoutRedirection> = None;
    let mut stderr_redirect: Option<StderrRedirection> = None;

    let mut iter = args.into_iter().peekable();
    while let Some(word) = iter.next() {
        if let Some((is_stdout, mode)) = classify_redirect(&word) {
            if let Some(target) = iter.next() {
                let path = std::path::PathBuf::from(target.to_string());
                if is_stdout {
                    stdout_redirect = Some(StdoutRedirection::new(mode, path));
                } else {
                    stderr_redirect = Some(StderrRedirection::new(mode, path));
                }
            } else {
                out.push(word);
            }
        } else {
            out.push(word);
        }
    }

    (out, stdout_redirect, stderr_redirect)
}

/// Return the redirect class for `word` if it is a redirect operator.
///
/// Only pure unquoted, unescaped words are eligible: `span.len() ==
/// bytes.len()` ensures quote characters and backslash escapes would
/// expand the span beyond the content length.
fn classify_redirect(word: &Word) -> Option<(bool, RedirectMode)> {
    if word.span.len() != word.bytes.len() {
        return None;
    }
    match word.bytes.as_ref() {
        b">>" | b"1>>" => Some((true, RedirectMode::Append)),
        b">" | b"1>" => Some((true, RedirectMode::Overwrite)),
        b"2>>" => Some((false, RedirectMode::Append)),
        b"2>" => Some((false, RedirectMode::Overwrite)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::{Lexer, QuoteKind};
    use proptest::prelude::*;

    fn parse_raw(raw: &[u8]) -> Result<Vec<UnresolvedStage>, ShellError> {
        let mut lexer = Lexer::new();
        lexer.push(raw);
        lexer.finalize();
        Parser::new(lexer).collect()
    }

    fn parse_single(raw: &[u8]) -> UnresolvedStage {
        let mut stages = parse_raw(raw).unwrap();
        assert_eq!(stages.len(), 1, "expected exactly one stage");
        stages.remove(0)
    }

    fn stage_bytes(stage: &UnresolvedStage) -> (Vec<u8>, Vec<Vec<u8>>) {
        let cmd = stage.command.word.as_bytes().to_vec();
        let args = stage.args.iter().map(|a| a.as_bytes().to_vec()).collect();
        (cmd, args)
    }

    // --- Basic splitting ---

    #[test]
    fn test_split_command_and_args() {
        let stage = parse_single(b"ls -la /home/user");
        let (cmd, args) = stage_bytes(&stage);
        assert_eq!(cmd, b"ls");
        assert_eq!(args, vec![b"-la".to_vec(), b"/home/user".to_vec()]);
    }

    #[test]
    fn test_split_command_no_args() {
        let stage = parse_single(b"ls");
        let (cmd, args) = stage_bytes(&stage);
        assert_eq!(cmd, b"ls");
        assert!(args.is_empty());
    }

    #[test]
    fn test_split_command_single_arg() {
        let stage = parse_single(b"ls -la");
        let (cmd, args) = stage_bytes(&stage);
        assert_eq!(cmd, b"ls");
        assert_eq!(args, vec![b"-la".to_vec()]);
    }

    #[test]
    fn test_split_command_multiple_spaces() {
        let stage = parse_single(b"    command    arg1    arg2    arg3    ");
        let (cmd, args) = stage_bytes(&stage);
        assert_eq!(cmd, b"command");
        assert_eq!(
            args,
            vec![b"arg1".to_vec(), b"arg2".to_vec(), b"arg3".to_vec()]
        );
    }

    // --- Double-quote tests ---

    #[test]
    fn test_double_quote_preserves_spaces() {
        let stage = parse_single(b"echo \"hello    world\"");
        let (_, args) = stage_bytes(&stage);
        assert_eq!(args, vec![b"hello    world".to_vec()]);
    }

    #[test]
    fn test_double_quote_adjacent_concatenation() {
        let stage = parse_single(b"echo \"hello\"\"world\"");
        let (_, args) = stage_bytes(&stage);
        assert_eq!(args, vec![b"helloworld".to_vec()]);
    }

    #[test]
    fn test_double_quote_mixed_adjacent_concatenation() {
        let stage = parse_single(b"echo hello\"world\"");
        let (_, args) = stage_bytes(&stage);
        assert_eq!(args, vec![b"helloworld".to_vec()]);
    }

    #[test]
    fn test_double_quote_empty_quotes() {
        let stage = parse_single(b"echo \"\"");
        let (_, args) = stage_bytes(&stage);
        assert_eq!(args, vec![b"".to_vec()]);
    }

    #[test]
    fn test_double_quote_multiple_args() {
        let stage = parse_single(b"echo \"foo bar\" \"baz qux\"");
        let (_, args) = stage_bytes(&stage);
        assert_eq!(args, vec![b"foo bar".to_vec(), b"baz qux".to_vec()]);
    }

    #[test]
    fn test_double_quote_with_single_quote_inside() {
        let stage = parse_single(b"echo \"it's\"");
        let (_, args) = stage_bytes(&stage);
        assert_eq!(args, vec![b"it's".to_vec()]);
    }

    #[test]
    fn test_double_quote_unquoted_mixed() {
        let stage = parse_single(b"echo pre\"mid\"post");
        let (_, args) = stage_bytes(&stage);
        assert_eq!(args, vec![b"premidpost".to_vec()]);
    }

    // --- Backslash-in-double-quote tests ---

    #[test]
    fn test_dquote_backslash_escapes_backslash() {
        let stage = parse_single(b"echo \"A \\\\ escapes itself\"");
        let (_, args) = stage_bytes(&stage);
        assert_eq!(args, vec![b"A \\ escapes itself".to_vec()]);
    }

    #[test]
    fn test_dquote_backslash_escapes_dquote() {
        let stage = parse_single(b"echo \"A \\\" inside double quotes\"");
        let (_, args) = stage_bytes(&stage);
        assert_eq!(args, vec![b"A \" inside double quotes".to_vec()]);
    }

    #[test]
    fn test_dquote_backslash_non_special_preserved() {
        let stage = parse_single(b"echo \"\\n\"");
        let (_, args) = stage_bytes(&stage);
        assert_eq!(args, vec![b"\\n".to_vec()]);
    }

    // --- Single-quote tests ---

    #[test]
    fn test_single_quote_preserves_spaces() {
        let stage = parse_single(b"echo 'hello    world'");
        let (_, args) = stage_bytes(&stage);
        assert_eq!(args, vec![b"hello    world".to_vec()]);
    }

    #[test]
    fn test_single_quote_adjacent_concatenation() {
        let stage = parse_single(b"echo 'hello''world'");
        let (_, args) = stage_bytes(&stage);
        assert_eq!(args, vec![b"helloworld".to_vec()]);
    }

    #[test]
    fn test_single_quote_mixed_adjacent_concatenation() {
        let stage = parse_single(b"echo hello''world");
        let (_, args) = stage_bytes(&stage);
        assert_eq!(args, vec![b"helloworld".to_vec()]);
    }

    #[test]
    fn test_single_quote_backslash_literal() {
        let stage = parse_single(b"echo 'back\\slash'");
        let (_, args) = stage_bytes(&stage);
        assert_eq!(args, vec![b"back\\slash".to_vec()]);
    }

    // --- Backslash-escape tests (outside quotes) ---

    #[test]
    fn test_backslash_space_literal() {
        let stage = parse_single(b"echo three\\ \\ \\ spaces");
        let (_, args) = stage_bytes(&stage);
        assert_eq!(args, vec![b"three   spaces".to_vec()]);
    }

    #[test]
    fn test_backslash_backslash() {
        let stage = parse_single(b"echo hello\\\\world");
        let (_, args) = stage_bytes(&stage);
        assert_eq!(args, vec![b"hello\\world".to_vec()]);
    }

    #[test]
    fn test_backslash_letter() {
        let stage = parse_single(b"echo test\\nexample");
        let (_, args) = stage_bytes(&stage);
        assert_eq!(args, vec![b"testnexample".to_vec()]);
    }

    #[test]
    fn test_backslash_single_quote() {
        let stage = parse_single(b"echo \\'hello\\'");
        let (_, args) = stage_bytes(&stage);
        assert_eq!(args, vec![b"'hello'".to_vec()]);
    }

    #[test]
    fn test_backslash_inside_single_quote_is_literal() {
        let stage = parse_single(b"echo 'back\\slash'");
        let (_, args) = stage_bytes(&stage);
        assert_eq!(args, vec![b"back\\slash".to_vec()]);
    }

    #[test]
    fn test_trailing_backslash_consumed_silently() {
        let stage = parse_single(b"echo hello\\");
        let (_, args) = stage_bytes(&stage);
        assert_eq!(args, vec![b"hello".to_vec()]);
    }

    // --- Redirect classification ---

    #[test]
    fn test_backslash_escaped_gt_not_a_redirect() {
        let stage = parse_single(b"echo \\>");
        assert!(stage.stdout_redirect.is_none());
        assert_eq!(stage.args[0].as_bytes(), b">");
    }

    #[test]
    fn test_backslash_escaped_append_redirect_not_a_redirect() {
        let stage = parse_single(b"echo 1\\>>");
        assert!(stage.stdout_redirect.is_none());
        assert_eq!(stage.args[0].as_bytes(), b"1>>");
    }

    #[test]
    fn test_mixed_quoted_operator_not_a_redirect() {
        // 1'>' — the span covers 4 raw bytes, content is 2 bytes → not a redirect.
        let stage = parse_single(b"echo 1'>'");
        assert!(stage.stdout_redirect.is_none());
        assert_eq!(stage.args[0].as_bytes(), b"1>");
    }

    // --- Unclosed quote error tests ---

    #[test]
    fn test_unclosed_double_quote_returns_error() {
        let err = parse_raw(b"echo \"hello").unwrap_err();
        assert!(
            matches!(
                err,
                ShellError::UnclosedQuote {
                    style: QuoteKind::Double,
                    ..
                }
            ),
            "expected UnclosedQuote(Double), got: {err:?}"
        );
    }

    #[test]
    fn test_unclosed_double_quote_span_offset() {
        let err = parse_raw(b"echo \"hello").unwrap_err();
        if let ShellError::UnclosedQuote { span, .. } = err {
            assert_eq!(span.offset(), 5, "quote opens at byte 5 in 'echo \"hello'");
        } else {
            panic!("expected UnclosedQuote");
        }
    }

    #[test]
    fn test_unclosed_single_quote_returns_error() {
        let err = parse_raw(b"echo 'hello").unwrap_err();
        assert!(
            matches!(
                err,
                ShellError::UnclosedQuote {
                    style: QuoteKind::Single,
                    ..
                }
            ),
            "expected UnclosedQuote(Single), got: {err:?}"
        );
    }

    #[test]
    fn test_unclosed_single_quote_span_offset() {
        let err = parse_raw(b"echo 'hello").unwrap_err();
        if let ShellError::UnclosedQuote { span, .. } = err {
            assert_eq!(span.offset(), 5, "quote opens at byte 5 in \"echo 'hello\"");
        } else {
            panic!("expected UnclosedQuote");
        }
    }

    #[test]
    fn test_unclosed_quote_is_non_fatal() {
        let err = parse_raw(b"echo \"unterminated").unwrap_err();
        assert!(!err.is_fatal());
    }

    #[test]
    fn test_pipeline_unclosed_quote_span_accounts_for_leading_whitespace() {
        let raw = b"echo foo | echo \"bar";
        let err = parse_raw(raw).unwrap_err();
        if let ShellError::UnclosedQuote { span, .. } = err {
            assert_eq!(
                span.offset(),
                16,
                "quote `\"` is at byte 16 in the full input"
            );
        } else {
            panic!("expected UnclosedQuote, got: {err:?}");
        }
    }

    // --- Redirect extraction tests ---

    #[test]
    fn test_parse_redirect_gt() {
        let stage = parse_single(b"echo hello > out.txt");
        let (_, args) = stage_bytes(&stage);
        assert_eq!(args, vec![b"hello".to_vec()]);
        let r = stage
            .stdout_redirect
            .expect("stdout redirect should be Some for >");
        assert_eq!(r.mode, RedirectMode::Overwrite);
        assert_eq!(r.target, std::path::PathBuf::from("out.txt"));
        assert!(stage.stderr_redirect.is_none());
    }

    #[test]
    fn test_parse_redirect_1gt() {
        let stage = parse_single(b"echo hello 1> out.txt");
        let (_, args) = stage_bytes(&stage);
        assert_eq!(args, vec![b"hello".to_vec()]);
        let r = stage
            .stdout_redirect
            .expect("stdout redirect should be Some for 1>");
        assert_eq!(r.mode, RedirectMode::Overwrite);
        assert_eq!(r.target, std::path::PathBuf::from("out.txt"));
        assert!(stage.stderr_redirect.is_none());
    }

    #[test]
    fn test_parse_redirect_gtgt() {
        let stage = parse_single(b"echo hello >> out.txt");
        let (_, args) = stage_bytes(&stage);
        assert_eq!(args, vec![b"hello".to_vec()]);
        let r = stage
            .stdout_redirect
            .expect("stdout redirect should be Some for >>");
        assert_eq!(r.mode, RedirectMode::Append);
        assert_eq!(r.target, std::path::PathBuf::from("out.txt"));
        assert!(stage.stderr_redirect.is_none());
    }

    #[test]
    fn test_parse_redirect_1gtgt() {
        let stage = parse_single(b"echo hello 1>> out.txt");
        let (_, args) = stage_bytes(&stage);
        assert_eq!(args, vec![b"hello".to_vec()]);
        let r = stage
            .stdout_redirect
            .expect("stdout redirect should be Some for 1>>");
        assert_eq!(r.mode, RedirectMode::Append);
        assert_eq!(r.target, std::path::PathBuf::from("out.txt"));
        assert!(stage.stderr_redirect.is_none());
    }

    #[test]
    fn test_parse_redirect_2gt() {
        let stage = parse_single(b"echo hello 2> err.txt");
        let (_, args) = stage_bytes(&stage);
        assert_eq!(args, vec![b"hello".to_vec()]);
        assert!(stage.stdout_redirect.is_none());
        let r = stage
            .stderr_redirect
            .expect("stderr redirect should be Some for 2>");
        assert_eq!(r.mode, RedirectMode::Overwrite);
        assert_eq!(r.target, std::path::PathBuf::from("err.txt"));
    }

    #[test]
    fn test_parse_redirect_2gtgt() {
        let stage = parse_single(b"exit notanumber 2>> err.txt");
        let (_, args) = stage_bytes(&stage);
        assert_eq!(args, vec![b"notanumber".to_vec()]);
        assert!(stage.stdout_redirect.is_none());
        let r = stage
            .stderr_redirect
            .expect("stderr redirect should be Some for 2>>");
        assert_eq!(r.mode, RedirectMode::Append);
        assert_eq!(r.target, std::path::PathBuf::from("err.txt"));
    }

    #[test]
    fn test_parse_redirect_last_wins_stdout() {
        let stage = parse_single(b"echo hi > first.txt >> last.txt");
        let r = stage
            .stdout_redirect
            .expect("stdout redirect should be Some");
        assert_eq!(r.mode, RedirectMode::Append);
        assert_eq!(r.target, std::path::PathBuf::from("last.txt"));
    }

    #[test]
    fn test_parse_redirect_last_wins_stderr() {
        let stage = parse_single(b"echo hi 2> first.txt 2>> last.txt");
        let r = stage
            .stderr_redirect
            .expect("stderr redirect should be Some");
        assert_eq!(r.mode, RedirectMode::Append);
        assert_eq!(r.target, std::path::PathBuf::from("last.txt"));
    }

    #[test]
    fn test_parse_redirect_trailing_operator_kept_as_arg() {
        let stage = parse_single(b"echo >");
        assert!(stage.stdout_redirect.is_none());
        let (_, args) = stage_bytes(&stage);
        assert_eq!(
            args,
            vec![b">".to_vec()],
            "trailing `>` should be kept as a literal arg"
        );
    }

    #[test]
    fn test_parse_redirect_trailing_gtgt_kept_as_arg() {
        let stage = parse_single(b"echo >>");
        assert!(stage.stdout_redirect.is_none());
        let (_, args) = stage_bytes(&stage);
        assert_eq!(args, vec![b">>".to_vec()]);
    }

    #[test]
    fn test_parse_stderr_redirect_trailing_operator_kept_as_arg() {
        let stage = parse_single(b"echo 2>");
        assert!(stage.stderr_redirect.is_none());
        let (_, args) = stage_bytes(&stage);
        assert_eq!(args, vec![b"2>".to_vec()]);
    }

    #[test]
    fn test_parse_stderr_append_redirect_trailing_operator_kept_as_arg() {
        let stage = parse_single(b"echo 2>>");
        assert!(stage.stderr_redirect.is_none());
        let (_, args) = stage_bytes(&stage);
        assert_eq!(args, vec![b"2>>".to_vec()]);
    }

    #[test]
    fn test_parse_no_redirect() {
        let stage = parse_single(b"echo hello world");
        let (_, args) = stage_bytes(&stage);
        assert_eq!(args, vec![b"hello".to_vec(), b"world".to_vec()]);
        assert!(stage.stdout_redirect.is_none());
        assert!(stage.stderr_redirect.is_none());
    }

    // --- Pipeline splitting tests ---

    #[test]
    fn test_pipeline_single_command_gives_one_stage() {
        let p = parse_raw(b"echo hello").unwrap();
        assert_eq!(p.len(), 1);
    }

    #[test]
    fn test_pipeline_two_commands_gives_two_stages() {
        let p = parse_raw(b"echo foo | cat").unwrap();
        assert_eq!(p.len(), 2);
        assert_eq!(p[0].command.word.as_bytes(), b"echo");
        assert_eq!(p[0].args[0].as_bytes(), b"foo");
        assert_eq!(p[1].command.word.as_bytes(), b"cat");
        assert!(p[1].args.is_empty());
    }

    #[test]
    fn test_pipeline_quoted_pipe_is_not_separator() {
        let p = parse_raw(b"echo 'foo | bar'").unwrap();
        assert_eq!(p.len(), 1, "quoted | must not split the pipeline");
        assert_eq!(p[0].args[0].as_bytes(), b"foo | bar");
    }

    #[test]
    fn test_pipeline_double_quoted_pipe_is_not_separator() {
        let p = parse_raw(b"echo \"foo | bar\"").unwrap();
        assert_eq!(p.len(), 1, "double-quoted | must not split the pipeline");
        assert_eq!(p[0].args[0].as_bytes(), b"foo | bar");
    }

    #[test]
    fn test_pipeline_last_stage_redirect() {
        let p = parse_raw(b"echo foo | cat > out.txt").unwrap();
        assert_eq!(p.len(), 2);
        let r = p[1]
            .stdout_redirect
            .as_ref()
            .expect("last stage should have redirect");
        assert_eq!(r.target, std::path::PathBuf::from("out.txt"));
    }

    // --- EmptyPipelineSegment tests ---

    #[test]
    fn test_trailing_pipe_returns_empty_segment_error() {
        let err = parse_raw(b"echo hello |").unwrap_err();
        assert!(
            matches!(err, ShellError::EmptyPipelineSegment { .. }),
            "got: {err:?}"
        );
    }

    #[test]
    fn test_trailing_pipe_span_points_at_pipe() {
        let err = parse_raw(b"echo hello |").unwrap_err();
        if let ShellError::EmptyPipelineSegment { span, .. } = err {
            assert_eq!(span.offset(), 11, "`|` is at byte 11 in 'echo hello |'");
            assert_eq!(span.len(), 1);
        } else {
            panic!("expected EmptyPipelineSegment");
        }
    }

    #[test]
    fn test_leading_pipe_returns_empty_segment_error() {
        let err = parse_raw(b"| cat").unwrap_err();
        assert!(
            matches!(err, ShellError::EmptyPipelineSegment { .. }),
            "got: {err:?}"
        );
    }

    #[test]
    fn test_leading_pipe_span_points_at_pipe() {
        let err = parse_raw(b"| cat").unwrap_err();
        if let ShellError::EmptyPipelineSegment { span, .. } = err {
            assert_eq!(span.offset(), 0, "`|` is at byte 0 in '| cat'");
            assert_eq!(span.len(), 1);
        } else {
            panic!("expected EmptyPipelineSegment");
        }
    }

    #[test]
    fn test_empty_middle_segment_returns_error() {
        let err = parse_raw(b"echo a | | cat").unwrap_err();
        assert!(
            matches!(err, ShellError::EmptyPipelineSegment { .. }),
            "got: {err:?}"
        );
    }

    #[test]
    fn test_empty_middle_segment_span_points_at_second_pipe() {
        let err = parse_raw(b"echo a | | cat").unwrap_err();
        if let ShellError::EmptyPipelineSegment { span, .. } = err {
            assert_eq!(
                span.offset(),
                9,
                "second `|` is at byte 9 in 'echo a | | cat'"
            );
            assert_eq!(span.len(), 1);
        } else {
            panic!("expected EmptyPipelineSegment");
        }
    }

    #[test]
    fn test_whitespace_only_segment_returns_error() {
        let err = parse_raw(b"echo a |   | cat").unwrap_err();
        assert!(
            matches!(err, ShellError::EmptyPipelineSegment { .. }),
            "got: {err:?}"
        );
    }

    #[test]
    fn test_whitespace_only_segment_span_points_at_second_pipe() {
        let err = parse_raw(b"echo a |   | cat").unwrap_err();
        if let ShellError::EmptyPipelineSegment { span, .. } = err {
            assert_eq!(
                span.offset(),
                11,
                "second `|` is at byte 11 in 'echo a |   | cat'"
            );
        } else {
            panic!("expected EmptyPipelineSegment");
        }
    }

    #[test]
    fn test_empty_pipeline_segment_is_non_fatal() {
        let err = parse_raw(b"echo hello |").unwrap_err();
        assert!(!err.is_fatal());
    }

    #[test]
    fn test_empty_pipeline_segment_display() {
        let err = parse_raw(b"| cat").unwrap_err();
        assert!(
            err.to_string().contains("|"),
            "expected `|` in error message, got: {err}"
        );
    }

    // --- Property tests ---

    proptest! {
        #[test]
        fn parse_never_panics(bytes in proptest::collection::vec(any::<u8>(), 0..256)) {
            let _ = parse_raw(&bytes);
        }
    }

    proptest! {
        #[test]
        fn quoted_token_never_classified_as_redirect(
            body in "[^'\"\\\\\\n]{0,20}"
        ) {
            for quoted in [format!("echo '{}'", body), format!("echo \"{}\"", body)] {
                let stages = parse_raw(quoted.as_bytes());
                prop_assert!(stages.is_ok(), "well-formed quoted input failed to parse: {:?}", quoted);
                for stage in stages.unwrap() {
                    prop_assert!(stage.stdout_redirect.is_none(),
                        "quoted token produced a stdout redirect in: {:?}", quoted);
                    prop_assert!(stage.stderr_redirect.is_none(),
                        "quoted token produced a stderr redirect in: {:?}", quoted);
                }
            }
        }
    }

    proptest! {
        #[test]
        fn clean_args_produce_no_redirects(
            cmd in "[a-zA-Z][a-zA-Z0-9_]{0,10}",
            args in proptest::collection::vec("[a-zA-Z0-9_]{1,10}", 0..5)
        ) {
            let input = if args.is_empty() {
                cmd.clone()
            } else {
                format!("{} {}", cmd, args.join(" "))
            };
            let stages = parse_raw(input.as_bytes());
            prop_assert!(stages.is_ok(), "clean identifier input failed to parse: {:?}", input);
            for stage in stages.unwrap() {
                prop_assert!(stage.stdout_redirect.is_none());
                prop_assert!(stage.stderr_redirect.is_none());
            }
        }
    }
}
