use std::str::FromStr;

use is_executable::IsExecutable;
use miette::SourceSpan;

use crate::arg::{Arg, Args, QuoteStyle};
use crate::command::builtin::BuiltInCommand;
use crate::command::executable::ExecutableCommand;
use crate::command::{Command, builtin};
use crate::env::get_path_files;
use crate::error::ShellError;
use crate::redirect::{RedirectMode, StderrRedirection, StdoutRedirection};

/// Parse a raw input line into a [`Command`], its argument list, an optional
/// stdout [`Redirection`], and an optional stderr [`Redirection`].
///
/// Recognised redirect operators and their meanings:
///
/// | Operator      | fd     | mode      |
/// |---------------|--------|-----------|
/// | `>` / `1>`    | stdout | overwrite |
/// | `>>` / `1>>`  | stdout | append    |
/// | `2>`           | stderr | overwrite |
/// | `2>>`          | stderr | append    |
///
/// When the same fd appears more than once, the **last** operator wins.
/// Only unquoted redirect operators are recognised; an operator character that
/// appears inside quotes is treated as a literal argument character.
pub fn parse(
    buffer: &[u8],
) -> Result<
    (
        Command,
        Args,
        Option<StdoutRedirection>,
        Option<StderrRedirection>,
    ),
    ShellError,
> {
    let (command, raw_args) = split_command_and_args(buffer)?;
    let command = parse_command(&command);

    let (args, stdout_redirect, stderr_redirect) = extract_redirects(raw_args);

    let args = args
        .into_iter()
        .map(|(bytes, style)| match style {
            QuoteStyle::None => Arg::Literal(bytes),
            style => Arg::Quoted { bytes, style },
        })
        .collect();
    Ok((command, args, stdout_redirect, stderr_redirect))
}

/// Raw argument token: byte content and its quoting style.
type RawArg = (Vec<u8>, QuoteStyle);

/// Result of redirect extraction: remaining args, optional stdout [`StdoutRedirection`],
/// and optional stderr [`StderrRedirection`].
type ExtractResult = (Vec<RawArg>, Option<StdoutRedirection>, Option<StderrRedirection>);

/// Scan `raw_args` for unquoted redirect operators
/// (`>`, `1>`, `>>`, `1>>`, `2>`, `2>>`).
///
/// Returns the remaining argument list (operators and their target filenames
/// are removed), an optional stdout [`StdoutRedirection`], and an optional stderr
/// [`StderrRedirection`].  Multiple operators targeting the same fd are allowed;
/// **only the last one is recorded** (earlier ones are stripped silently
/// without side-effects such as creating or truncating intermediate targets).
///
/// When a redirect operator appears without a following filename token (e.g.
/// a trailing `>>`), the operator is kept as a normal argument rather than
/// silently dropped.
///
/// # Quoting
/// Only tokens with [`QuoteStyle::None`] are treated as potential operators.
/// Mixed-quoted tokens (e.g. `1'>'`, which produces bytes `1>`) carry
/// [`QuoteStyle::Mixed`] and are therefore never mistaken for operators.
fn extract_redirects(raw_args: Vec<RawArg>) -> ExtractResult {
    let mut out_args: Vec<RawArg> = Vec::new();
    let mut stdout_redirect: Option<StdoutRedirection> = None;
    let mut stderr_redirect: Option<StderrRedirection> = None;

    let mut iter = raw_args.into_iter().peekable();
    while let Some((bytes, style)) = iter.next() {
        // Only treat unquoted tokens as potential redirect operators.
        if style == QuoteStyle::None {
            let token = &bytes[..];

            // Classify the token as (is_stdout, mode). Check longer tokens
            // first so `1>>` / `2>>` are not mistaken for `1>` / `2>`.
            let op: Option<(bool, RedirectMode)> = if token == b">>" || token == b"1>>" {
                Some((true, RedirectMode::Append))
            } else if token == b">" || token == b"1>" {
                Some((true, RedirectMode::Overwrite))
            } else if token == b"2>>" {
                Some((false, RedirectMode::Append))
            } else if token == b"2>" {
                Some((false, RedirectMode::Overwrite))
            } else {
                None
            };

            if let Some((is_stdout, mode)) = op {
                if let Some((target_bytes, _)) = iter.next() {
                    let target = std::path::PathBuf::from(
                        String::from_utf8_lossy(&target_bytes).as_ref(),
                    );
                    if is_stdout {
                        stdout_redirect = Some(StdoutRedirection::new(mode, target));
                    } else {
                        stderr_redirect = Some(StderrRedirection::new(mode, target));
                    }
                } else {
                    // No filename follows the operator — keep it as a literal
                    // argument rather than silently dropping it.
                    out_args.push((bytes, style));
                }
                continue;
            }
        }
        out_args.push((bytes, style));
    }

    (out_args, stdout_redirect, stderr_redirect)
}

/// Split the trimmed input buffer into a command token and zero or more argument tokens.
///
/// Handles single-quoted strings: characters inside `'...'` are treated literally —
/// whitespace is preserved (not used as a delimiter) and backslashes have no special
/// meaning.
///
/// Handles double-quoted strings: characters inside `"..."` are treated mostly literally —
/// whitespace is preserved (not used as a delimiter). Inside double quotes, `\\` → `\`
/// and `\"` → `"`; all other `\X` sequences preserve the backslash literally.
///
/// Outside quotes, a `\` before any character emits that character literally and
/// consumes the backslash (e.g. `\ ` → space, `\\` → `\`).
///
/// Adjacent quoted and unquoted segments are concatenated into a single token.
///
/// Returns `Err(ShellError::UnclosedQuote)` if a quote is opened but never closed.
fn split_command_and_args(buffer: &[u8]) -> Result<(Vec<u8>, Vec<RawArg>), ShellError> {
    let buffer = buffer.trim_ascii();
    let mut parts: Vec<(Vec<u8>, QuoteStyle)> = Vec::new();

    let mut current: Vec<u8> = Vec::new();
    let mut in_single_quote = false;
    let mut in_double_quote = false;
    let mut token_started = false;
    let mut token_style: Option<QuoteStyle> = None;
    // Byte offset of the opening quote character (relative to the trimmed buffer).
    let mut quote_open_pos: usize = 0;

    let mut i = 0;
    while i < buffer.len() {
        let byte = buffer[i];

        if in_single_quote {
            if byte == b'\'' {
                in_single_quote = false;
            } else {
                current.push(byte);
            }
        } else if in_double_quote {
            if byte == b'"' {
                in_double_quote = false;
            } else if byte == b'\\' && i + 1 < buffer.len() {
                // Inside double quotes, backslash only escapes `"` and `\`.
                // For all other characters, the backslash is preserved literally.
                let next = buffer[i + 1];
                if next == b'"' || next == b'\\' {
                    i += 1;
                    current.push(next);
                } else {
                    current.push(b'\\');
                }
            } else {
                current.push(byte);
            }
        } else if byte == b'\\' {
            // Outside any quotes: consume the backslash and emit the next byte literally.
            // If there is no next byte (trailing backslash), consume the backslash silently.
            if i + 1 < buffer.len() {
                i += 1;
                current.push(buffer[i]);
            }
            token_started = true;
            token_style = match token_style {
                None | Some(QuoteStyle::None) => Some(QuoteStyle::None),
                _ => Some(QuoteStyle::Mixed),
            };
        } else if byte == b'\'' {
            in_single_quote = true;
            quote_open_pos = i;
            token_started = true;
            if token_style.is_none() {
                token_style = Some(QuoteStyle::Single);
            } else if !matches!(token_style, Some(QuoteStyle::Single)) {
                token_style = Some(QuoteStyle::Mixed);
            }
        } else if byte == b'"' {
            in_double_quote = true;
            quote_open_pos = i;
            token_started = true;
            if token_style.is_none() {
                token_style = Some(QuoteStyle::Double);
            } else if !matches!(token_style, Some(QuoteStyle::Double)) {
                token_style = Some(QuoteStyle::Mixed);
            }
        } else if byte.is_ascii_whitespace() {
            if token_started || !current.is_empty() {
                let style = token_style.take().unwrap_or(QuoteStyle::None);
                parts.push((std::mem::take(&mut current), style));
                token_started = false;
            }
        } else {
            current.push(byte);
            token_started = true;
            token_style = match token_style {
                None | Some(QuoteStyle::None) => Some(QuoteStyle::None),
                _ => Some(QuoteStyle::Mixed),
            };
        }

        i += 1;
    }

    if in_single_quote {
        return Err(ShellError::UnclosedQuote {
            style: QuoteStyle::Single,
            span: SourceSpan::from((quote_open_pos, 1)),
        });
    }
    if in_double_quote {
        return Err(ShellError::UnclosedQuote {
            style: QuoteStyle::Double,
            span: SourceSpan::from((quote_open_pos, 1)),
        });
    }

    if token_started || !current.is_empty() {
        let style = token_style.unwrap_or(QuoteStyle::None);
        parts.push((current, style));
    }

    if parts.is_empty() {
        return Ok((Vec::new(), Vec::new()));
    }

    let mut iter = parts.into_iter();
    let (command, _) = iter.next().unwrap_or((Vec::new(), QuoteStyle::None));
    let args: Vec<(Vec<u8>, QuoteStyle)> = iter.collect();

    Ok((command, args))
}

/// Resolve a raw command token to a [`Command`] variant.
pub fn parse_command(command: &[u8]) -> Command {
    if !command.is_ascii() {
        return Command::Unrecognized(command.into());
    }

    let command = std::str::from_utf8(command).expect("checked ASCII above");

    let name = builtin::BuiltInName::from_str(command);
    if let Ok(name) = name {
        Command::BuiltIn(BuiltInCommand::new(name))
    } else {
        for file in get_path_files().filter(|p| p.is_executable()) {
            let executable_command = ExecutableCommand::new(file);

            if executable_command.name() == command {
                return Command::Executable(executable_command);
            }
        }

        Command::Unrecognized(command.into())
    }
}

/// Parse a raw byte slice into an [`Arg`].
pub fn parse_arg(arg: &[u8]) -> Arg {
    Arg::Literal(arg.to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;

    // Helper: split and return command + arg bytes (quoting metadata stripped).
    fn split(input: &[u8]) -> (Vec<u8>, Vec<Vec<u8>>) {
        let (cmd, args) = split_command_and_args(input).unwrap();
        let arg_bytes = args.into_iter().map(|(b, _)| b).collect();
        (cmd, arg_bytes)
    }

    // Helper: split and return command + (bytes, style) pairs.
    fn split_styled(input: &[u8]) -> (Vec<u8>, Vec<(Vec<u8>, QuoteStyle)>) {
        split_command_and_args(input).unwrap()
    }

    #[test]
    fn test_split_command_and_args() {
        let (command, args) = split(b"  ls   -la  /home/user  ");
        assert_eq!(command, b"ls");
        assert_eq!(args, vec![b"-la".to_vec(), b"/home/user".to_vec()]);
    }

    #[test]
    fn test_parse_command_empty() {
        let command = "".as_bytes();
        let command = parse_command(command);
        match command {
            Command::Unrecognized(command) => assert_eq!(command.len(), 0),
            _ => panic!("Empty command unexpectedly found as: {}", command),
        }
    }

    #[test]
    fn test_parse_command_unrecognized() {
        let command = "some_non_existent_command".as_bytes();
        let command = parse_command(command);
        match command {
            Command::Unrecognized(cmd) => {
                assert_eq!(cmd, b"some_non_existent_command");
            }
            _ => panic!("Unrecognized command was recognized: {}", command),
        }
    }

    #[test]
    fn test_parse_arg() {
        let arg = "/home/user".as_bytes();
        let parsed_arg = parse_arg(arg);
        assert_eq!(parsed_arg.as_bytes(), arg);
        assert!(matches!(parsed_arg, Arg::Literal(_)));
    }

    #[test]
    fn test_split_command_and_args_no_args() {
        let (command, args) = split(b"ls");
        assert_eq!(command, b"ls");
        assert!(args.is_empty());
    }

    #[test]
    fn test_split_command_and_args_single_arg() {
        let (command, args) = split(b"ls -la");
        assert_eq!(command, b"ls");
        assert_eq!(args, vec![b"-la".to_vec()]);
    }

    #[test]
    fn test_split_command_and_args_multiple_spaces() {
        let (command, args) = split(b"    command    arg1    arg2    arg3    ");
        assert_eq!(command, b"command");
        assert_eq!(
            args,
            vec![b"arg1".to_vec(), b"arg2".to_vec(), b"arg3".to_vec()]
        );
    }

    #[test]
    fn test_parse_arg_empty() {
        let arg = "".as_bytes();
        let parsed_arg = parse_arg(arg);
        assert!(parsed_arg.as_bytes().is_empty());
        assert!(matches!(parsed_arg, Arg::Literal(_)));
    }

    #[test]
    fn test_parse_arg_special_chars() {
        let arg = "file-with_special.chars".as_bytes();
        let parsed_arg = parse_arg(arg);
        assert_eq!(parsed_arg.as_bytes(), arg);
        assert!(matches!(parsed_arg, Arg::Literal(_)));
    }

    // --- Double-quote tests ---

    #[test]
    fn test_double_quote_preserves_spaces() {
        let (command, args) = split(b"echo \"hello    world\"");
        assert_eq!(command, b"echo");
        assert_eq!(args, vec![b"hello    world".to_vec()]);
    }

    #[test]
    fn test_double_quote_adjacent_concatenation() {
        let (command, args) = split(b"echo \"hello\"\"world\"");
        assert_eq!(command, b"echo");
        assert_eq!(args, vec![b"helloworld".to_vec()]);
    }

    #[test]
    fn test_double_quote_mixed_adjacent_concatenation() {
        let (command, args) = split(b"echo hello\"world\"");
        assert_eq!(command, b"echo");
        assert_eq!(args, vec![b"helloworld".to_vec()]);
    }

    #[test]
    fn test_double_quote_empty_quotes() {
        let (command, args) = split(b"echo \"\"");
        assert_eq!(command, b"echo");
        assert_eq!(args, vec![b"".to_vec()]);
    }

    #[test]
    fn test_double_quote_multiple_args() {
        let (command, args) = split(b"echo \"foo bar\" \"baz qux\"");
        assert_eq!(command, b"echo");
        assert_eq!(args, vec![b"foo bar".to_vec(), b"baz qux".to_vec()]);
    }

    #[test]
    fn test_double_quote_with_single_quote_inside() {
        let (command, args) = split(b"echo \"it's\"");
        assert_eq!(command, b"echo");
        assert_eq!(args, vec![b"it's".to_vec()]);
    }

    #[test]
    fn test_double_quote_unquoted_mixed() {
        let (command, args) = split(b"echo pre\"mid\"post");
        assert_eq!(command, b"echo");
        assert_eq!(args, vec![b"premidpost".to_vec()]);
    }

    // --- Backslash-in-double-quote tests ---

    #[test]
    fn test_dquote_backslash_escapes_backslash() {
        let (command, args) = split(b"echo \"A \\\\ escapes itself\"");
        assert_eq!(command, b"echo");
        assert_eq!(args, vec![b"A \\ escapes itself".to_vec()]);
    }

    #[test]
    fn test_dquote_backslash_escapes_dquote() {
        let (command, args) = split(b"echo \"A \\\" inside double quotes\"");
        assert_eq!(command, b"echo");
        assert_eq!(args, vec![b"A \" inside double quotes".to_vec()]);
    }

    #[test]
    fn test_dquote_backslash_non_special_preserved() {
        let (command, args) = split(b"echo \"\\n\"");
        assert_eq!(command, b"echo");
        assert_eq!(args, vec![b"\\n".to_vec()]);
    }

    // --- Single-quote tests ---

    #[test]
    fn test_single_quote_preserves_spaces() {
        let (command, args) = split(b"echo 'hello    world'");
        assert_eq!(command, b"echo");
        assert_eq!(args, vec![b"hello    world".to_vec()]);
    }

    #[test]
    fn test_single_quote_adjacent_concatenation() {
        let (command, args) = split(b"echo 'hello''world'");
        assert_eq!(command, b"echo");
        assert_eq!(args, vec![b"helloworld".to_vec()]);
    }

    #[test]
    fn test_single_quote_mixed_adjacent_concatenation() {
        let (command, args) = split(b"echo hello''world");
        assert_eq!(command, b"echo");
        assert_eq!(args, vec![b"helloworld".to_vec()]);
    }

    #[test]
    fn test_single_quote_backslash_literal() {
        let (command, args) = split(b"echo 'back\\slash'");
        assert_eq!(command, b"echo");
        assert_eq!(args, vec![b"back\\slash".to_vec()]);
    }

    #[test]
    fn test_single_quote_empty_quotes() {
        let (command, args) = split(b"echo hello''world");
        assert_eq!(command, b"echo");
        assert_eq!(args, vec![b"helloworld".to_vec()]);
    }

    // --- Backslash-escape tests (outside quotes) ---

    #[test]
    fn test_backslash_space_literal() {
        let (command, args) = split(b"echo three\\ \\ \\ spaces");
        assert_eq!(command, b"echo");
        assert_eq!(args, vec![b"three   spaces".to_vec()]);
    }

    #[test]
    fn test_backslash_backslash() {
        let (command, args) = split(b"echo hello\\\\world");
        assert_eq!(command, b"echo");
        assert_eq!(args, vec![b"hello\\world".to_vec()]);
    }

    #[test]
    fn test_backslash_letter() {
        let (command, args) = split(b"echo test\\nexample");
        assert_eq!(command, b"echo");
        assert_eq!(args, vec![b"testnexample".to_vec()]);
    }

    #[test]
    fn test_backslash_single_quote() {
        let (command, args) = split(b"echo \\'hello\\'");
        assert_eq!(command, b"echo");
        assert_eq!(args, vec![b"'hello'".to_vec()]);
    }

    #[test]
    fn test_backslash_inside_single_quote_is_literal() {
        let (command, args) = split(b"echo 'back\\slash'");
        assert_eq!(command, b"echo");
        assert_eq!(args, vec![b"back\\slash".to_vec()]);
    }

    #[test]
    fn test_trailing_backslash_consumed_silently() {
        let (command, args) = split(b"echo hello\\");
        assert_eq!(command, b"echo");
        assert_eq!(args, vec![b"hello".to_vec()]);
    }

    // --- QuoteStyle variant tests ---

    #[test]
    fn test_style_unquoted_is_literal() {
        let (_, args) = split_styled(b"echo hello");
        assert_eq!(args, vec![(b"hello".to_vec(), QuoteStyle::None)]);
    }

    #[test]
    fn test_style_single_quoted() {
        let (_, args) = split_styled(b"echo 'hello world'");
        assert_eq!(args, vec![(b"hello world".to_vec(), QuoteStyle::Single)]);
    }

    #[test]
    fn test_style_double_quoted() {
        let (_, args) = split_styled(b"echo \"hello world\"");
        assert_eq!(args, vec![(b"hello world".to_vec(), QuoteStyle::Double)]);
    }

    #[test]
    fn test_style_adjacent_single_quoted_stays_single() {
        let (_, args) = split_styled(b"echo 'hello''world'");
        assert_eq!(args, vec![(b"helloworld".to_vec(), QuoteStyle::Single)]);
    }

    #[test]
    fn test_style_mixed_is_mixed() {
        let (_, args) = split_styled(b"echo pre\"mid\"post");
        assert_eq!(args, vec![(b"premidpost".to_vec(), QuoteStyle::Mixed)]);
    }

    #[test]
    fn test_style_single_then_double_is_mixed() {
        let (_, args) = split_styled(b"echo 'a'\"b\"");
        assert_eq!(args, vec![(b"ab".to_vec(), QuoteStyle::Mixed)]);
    }

    #[test]
    fn test_style_unquoted_then_single_is_mixed() {
        let (_, args) = split_styled(b"echo 1'>'");
        assert_eq!(args, vec![(b"1>".to_vec(), QuoteStyle::Mixed)]);
    }

    // --- Unclosed quote error tests ---

    #[test]
    fn test_unclosed_double_quote_returns_error() {
        let result = split_command_and_args(b"echo \"hello");
        let err = result.unwrap_err();
        assert!(
            matches!(
                err,
                ShellError::UnclosedQuote {
                    style: QuoteStyle::Double,
                    ..
                }
            ),
            "expected UnclosedQuote(Double), got: {err:?}"
        );
    }

    #[test]
    fn test_unclosed_double_quote_span_offset() {
        let result = split_command_and_args(b"echo \"hello");
        let err = result.unwrap_err();
        if let ShellError::UnclosedQuote { span, .. } = err {
            assert_eq!(span.offset(), 5, "quote opens at byte 5 in 'echo \"hello'");
        } else {
            panic!("expected UnclosedQuote");
        }
    }

    #[test]
    fn test_unclosed_single_quote_returns_error() {
        let result = split_command_and_args(b"echo 'hello");
        let err = result.unwrap_err();
        assert!(
            matches!(
                err,
                ShellError::UnclosedQuote {
                    style: QuoteStyle::Single,
                    ..
                }
            ),
            "expected UnclosedQuote(Single), got: {err:?}"
        );
    }

    #[test]
    fn test_unclosed_single_quote_span_offset() {
        let result = split_command_and_args(b"echo 'hello");
        let err = result.unwrap_err();
        if let ShellError::UnclosedQuote { span, .. } = err {
            assert_eq!(span.offset(), 5, "quote opens at byte 5 in \"echo 'hello\"");
        } else {
            panic!("expected UnclosedQuote");
        }
    }

    #[test]
    fn test_unclosed_quote_is_non_fatal() {
        let result = split_command_and_args(b"echo \"unterminated");
        let err = result.unwrap_err();
        assert!(!err.is_fatal());
    }

    // --- Redirect extraction tests ---

    #[test]
    fn test_parse_redirect_gt() {
        let (_, args, stdout_redirect, stderr_redirect) =
            parse(b"echo hello > out.txt").unwrap();
        assert_eq!(
            args.iter().map(|a: &Arg| a.to_string()).collect::<Vec<_>>(),
            vec!["hello"]
        );
        let r = stdout_redirect.expect("stdout redirect should be Some for >");
        assert_eq!(r.mode, RedirectMode::Overwrite);
        assert_eq!(r.target, std::path::PathBuf::from("out.txt"));
        assert!(stderr_redirect.is_none());
    }

    #[test]
    fn test_parse_redirect_1gt() {
        let (_, args, stdout_redirect, stderr_redirect) =
            parse(b"echo hello 1> out.txt").unwrap();
        assert_eq!(
            args.iter().map(|a: &Arg| a.to_string()).collect::<Vec<_>>(),
            vec!["hello"]
        );
        let r = stdout_redirect.expect("stdout redirect should be Some for 1>");
        assert_eq!(r.mode, RedirectMode::Overwrite);
        assert_eq!(r.target, std::path::PathBuf::from("out.txt"));
        assert!(stderr_redirect.is_none());
    }

    #[test]
    fn test_parse_redirect_gtgt() {
        let (_, args, stdout_redirect, stderr_redirect) =
            parse(b"echo hello >> out.txt").unwrap();
        assert_eq!(
            args.iter().map(|a: &Arg| a.to_string()).collect::<Vec<_>>(),
            vec!["hello"]
        );
        let r = stdout_redirect.expect("stdout redirect should be Some for >>");
        assert_eq!(r.mode, RedirectMode::Append);
        assert_eq!(r.target, std::path::PathBuf::from("out.txt"));
        assert!(stderr_redirect.is_none());
    }

    #[test]
    fn test_parse_redirect_1gtgt() {
        let (_, args, stdout_redirect, stderr_redirect) =
            parse(b"echo hello 1>> out.txt").unwrap();
        assert_eq!(
            args.iter().map(|a: &Arg| a.to_string()).collect::<Vec<_>>(),
            vec!["hello"]
        );
        let r = stdout_redirect.expect("stdout redirect should be Some for 1>>");
        assert_eq!(r.mode, RedirectMode::Append);
        assert_eq!(r.target, std::path::PathBuf::from("out.txt"));
        assert!(stderr_redirect.is_none());
    }

    #[test]
    fn test_parse_redirect_2gt() {
        let (_, args, stdout_redirect, stderr_redirect) =
            parse(b"echo hello 2> err.txt").unwrap();
        assert_eq!(
            args.iter().map(|a: &Arg| a.to_string()).collect::<Vec<_>>(),
            vec!["hello"]
        );
        assert!(stdout_redirect.is_none(), "stdout redirect should be None for 2>");
        let r = stderr_redirect.expect("stderr redirect should be Some for 2>");
        assert_eq!(r.mode, RedirectMode::Overwrite);
        assert_eq!(r.target, std::path::PathBuf::from("err.txt"));
    }

    #[test]
    fn test_parse_redirect_2gtgt() {
        let (_, args, stdout_redirect, stderr_redirect) =
            parse(b"exit notanumber 2>> err.txt").unwrap();
        assert_eq!(
            args.iter().map(|a: &Arg| a.to_string()).collect::<Vec<_>>(),
            vec!["notanumber"]
        );
        assert!(stdout_redirect.is_none());
        let r = stderr_redirect.expect("stderr redirect should be Some for 2>>");
        assert_eq!(r.mode, RedirectMode::Append);
        assert_eq!(r.target, std::path::PathBuf::from("err.txt"));
    }

    #[test]
    fn test_parse_redirect_last_wins_stdout() {
        let (_, args, stdout_redirect, _) =
            parse(b"echo hi > first.txt >> last.txt").unwrap();
        assert_eq!(
            args.iter().map(|a: &Arg| a.to_string()).collect::<Vec<_>>(),
            vec!["hi"]
        );
        let r = stdout_redirect.expect("stdout redirect should be Some");
        assert_eq!(r.mode, RedirectMode::Append);
        assert_eq!(r.target, std::path::PathBuf::from("last.txt"));
    }

    #[test]
    fn test_parse_redirect_last_wins_stderr() {
        let (_, args, _, stderr_redirect) =
            parse(b"echo hi 2> first.txt 2>> last.txt").unwrap();
        assert_eq!(
            args.iter().map(|a: &Arg| a.to_string()).collect::<Vec<_>>(),
            vec!["hi"]
        );
        let r = stderr_redirect.expect("stderr redirect should be Some");
        assert_eq!(r.mode, RedirectMode::Append);
        assert_eq!(r.target, std::path::PathBuf::from("last.txt"));
    }

    #[test]
    fn test_parse_redirect_trailing_operator_kept_as_arg() {
        let (_, args, stdout_redirect, _) = parse(b"echo >").unwrap();
        assert!(stdout_redirect.is_none(), "no redirect target means no Redirection");
        assert_eq!(
            args.iter().map(|a: &Arg| a.to_string()).collect::<Vec<_>>(),
            vec![">"],
            "trailing `>` should be kept as a literal arg"
        );
    }

    #[test]
    fn test_parse_redirect_trailing_gtgt_kept_as_arg() {
        let (_, args, stdout_redirect, _) = parse(b"echo >>").unwrap();
        assert!(stdout_redirect.is_none(), "no redirect target means no Redirection");
        assert_eq!(
            args.iter().map(|a: &Arg| a.to_string()).collect::<Vec<_>>(),
            vec![">>"],
            "trailing `>>` should be kept as a literal arg"
        );
    }

    #[test]
    fn test_parse_stderr_redirect_trailing_operator_kept_as_arg() {
        let (_, args, _, stderr_redirect) = parse(b"echo 2>").unwrap();
        assert!(stderr_redirect.is_none(), "no redirect target means no Redirection");
        assert_eq!(
            args.iter().map(|a: &Arg| a.to_string()).collect::<Vec<_>>(),
            vec!["2>"],
            "trailing `2>` should be kept as a literal arg"
        );
    }

    #[test]
    fn test_parse_mixed_quoted_operator_not_a_redirect() {
        let (_, args, stdout_redirect, stderr_redirect) = parse(b"echo 1'>'").unwrap();
        assert!(
            stdout_redirect.is_none(),
            "mixed-quoted 1'>' must not trigger a redirect"
        );
        assert!(stderr_redirect.is_none());
        assert_eq!(
            args.iter().map(|a: &Arg| a.to_string()).collect::<Vec<_>>(),
            vec!["1>"],
        );
    }

    #[test]
    fn test_parse_stderr_append_redirect_trailing_operator_kept_as_arg() {
        let (_, args, _, stderr_redirect) = parse(b"echo 2>>").unwrap();
        assert!(stderr_redirect.is_none(), "no redirect target means no Redirection");
        assert_eq!(
            args.iter().map(|a: &Arg| a.to_string()).collect::<Vec<_>>(),
            vec!["2>>"],
            "trailing `2>>` should be kept as a literal arg"
        );
    }

    #[test]
    fn test_parse_no_redirect() {
        let (_, args, stdout_redirect, stderr_redirect) =
            parse(b"echo hello world").unwrap();
        assert_eq!(
            args.iter().map(|a: &Arg| a.to_string()).collect::<Vec<_>>(),
            vec!["hello", "world"]
        );
        assert!(stdout_redirect.is_none());
        assert!(stderr_redirect.is_none());
    }
}
