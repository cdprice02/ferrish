use std::str::FromStr;

use is_executable::IsExecutable;

use crate::arg::{Arg, Args, QuoteStyle};
use crate::command::builtin::BuiltInCommand;
use crate::command::executable::ExecutableCommand;
use crate::command::{Command, builtin};
use crate::env::get_path_files;
use crate::redirect::{Redirect, StderrRedirect};

/// Parse a raw input line into a [`Command`], its argument list, an optional
/// stdout [`Redirect`], and an optional stderr [`StderrRedirect`].
///
/// If the line contains `>` or `1>` followed by a filename, that operator and
/// the filename are stripped from the argument list and returned as a
/// [`Redirect`].  If the line contains `2>` followed by a filename, that
/// operator and the filename are returned as a [`StderrRedirect`].
///
/// Only unquoted redirect operators are recognised; an operator character that
/// appears inside quotes is treated as a literal argument character.
pub fn parse(buffer: &[u8]) -> (Command, Args, Option<Redirect>, Option<StderrRedirect>) {
    let (command, raw_args) = split_command_and_args(buffer);
    let command = parse_command(&command);

    let (args, redirect, stderr_redirect) = extract_redirects(raw_args);

    let args = args
        .into_iter()
        .map(|(bytes, style)| match style {
            QuoteStyle::None => Arg::Literal(bytes),
            style => Arg::Quoted { bytes, style },
        })
        .collect();
    (command, args, redirect, stderr_redirect)
}

/// Raw argument token: byte content and its quoting style.
type RawArg = (Vec<u8>, QuoteStyle);

/// Result of redirect extraction: remaining args, optional stdout redirect, optional stderr redirect.
type ExtractResult = (Vec<RawArg>, Option<Redirect>, Option<StderrRedirect>);

/// Scan `raw_args` for unquoted redirect operators (`>`, `1>`, `2>`).
///
/// Returns the remaining argument list (with operators and their target
/// filenames removed), an optional stdout [`Redirect`], and an optional
/// stderr [`StderrRedirect`]. If multiple operators of the same kind appear,
/// only the last one is returned; earlier ones are stripped from the argument
/// list but do not trigger intermediate bash-style side effects such as
/// creating or truncating their targets.
///
/// When a redirect operator appears without a following filename token (e.g.
/// a trailing `>`), the operator is kept as a normal argument rather than
/// silently dropped.
///
/// # Quoting caveat
/// Only tokens with [`QuoteStyle::None`] are treated as potential operators.
/// In this codebase `QuoteStyle::None` covers both truly-unquoted tokens
/// *and* mixed-quoting contexts (e.g. `1'>'`), so a mixed token whose bytes
/// happen to be `1>` would also be recognised as a redirect operator.
/// Distinguishing the two cases would require a dedicated `QuoteStyle::Mixed`
/// variant; that is tracked in issue #85.
fn extract_redirects(raw_args: Vec<RawArg>) -> ExtractResult {
    let mut out_args: Vec<RawArg> = Vec::new();
    let mut redirect: Option<Redirect> = None;
    let mut stderr_redirect: Option<StderrRedirect> = None;

    let mut iter = raw_args.into_iter().peekable();
    while let Some((bytes, style)) = iter.next() {
        // Only treat unquoted tokens as potential redirect operators.
        if style == QuoteStyle::None {
            let token = &bytes[..];
            if token == b">" || token == b"1>" {
                // The next token is the redirect target.
                if let Some((target_bytes, _)) = iter.next() {
                    let target = std::path::PathBuf::from(
                        String::from_utf8_lossy(&target_bytes).as_ref(),
                    );
                    redirect = Some(Redirect::new(target));
                } else {
                    // No filename follows the operator — keep it as a literal
                    // argument rather than silently dropping it.
                    out_args.push((bytes, style));
                }
                continue;
            }
            if token == b"2>" {
                // The next token is the stderr redirect target.
                if let Some((target_bytes, _)) = iter.next() {
                    let target = std::path::PathBuf::from(
                        String::from_utf8_lossy(&target_bytes).as_ref(),
                    );
                    stderr_redirect = Some(StderrRedirect::new(target));
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

    (out_args, redirect, stderr_redirect)
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
fn split_command_and_args(buffer: &[u8]) -> (Vec<u8>, Vec<(Vec<u8>, QuoteStyle)>) {
    let buffer = buffer.trim_ascii();
    let mut parts: Vec<(Vec<u8>, QuoteStyle)> = Vec::new();

    let mut current: Vec<u8> = Vec::new();
    let mut in_single_quote = false;
    let mut in_double_quote = false;
    // Track whether we've started building a token (needed to emit empty tokens
    // only when inside a quoted empty string at the word boundary — but for
    // single-quotes the shell spec says empty quotes produce an empty argument
    // only if they stand alone, which the concatenation logic handles naturally).
    let mut token_started = false;
    // Quote style for the token currently being built. None = unstarted; once set
    // to Single or Double it stays unless a byte from a different quoting context
    // arrives, at which point it becomes None (unquoted/mixed).
    let mut token_style: Option<QuoteStyle> = None;

    let mut i = 0;
    while i < buffer.len() {
        let byte = buffer[i];

        if in_single_quote {
            if byte == b'\'' {
                // Closing quote — exit single-quote mode but stay in current token.
                in_single_quote = false;
            } else {
                current.push(byte);
            }
        } else if in_double_quote {
            if byte == b'"' {
                // Closing double quote — exit double-quote mode but stay in current token.
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
            // Unquoted bytes make this a mixed/unquoted token.
            token_style = Some(QuoteStyle::None);
        } else if byte == b'\'' {
            // Opening single quote — enter single-quote mode and mark token as started.
            // Even empty quotes (e.g. `''`) count as beginning a token so that a
            // standalone `''` produces one empty argument rather than being dropped.
            in_single_quote = true;
            token_started = true;
            // Only keep Single style if the token has been purely single-quoted so far.
            if token_style.is_none() {
                token_style = Some(QuoteStyle::Single);
            } else if !matches!(token_style, Some(QuoteStyle::Single)) {
                token_style = Some(QuoteStyle::None);
            }
        } else if byte == b'"' {
            // Opening double quote — enter double-quote mode and mark token as started.
            // Same empty-quote semantics as single quotes.
            in_double_quote = true;
            token_started = true;
            // Only keep Double style if the token has been purely double-quoted so far.
            if token_style.is_none() {
                token_style = Some(QuoteStyle::Double);
            } else if !matches!(token_style, Some(QuoteStyle::Double)) {
                token_style = Some(QuoteStyle::None);
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
            // Any unquoted byte makes this a mixed/unquoted token.
            token_style = Some(QuoteStyle::None);
        }

        i += 1;
    }

    // Push the last token (may be empty if input was all whitespace after trim,
    // but trim_ascii above ensures the buffer is non-empty when we reach here).
    if token_started || !current.is_empty() {
        let style = token_style.unwrap_or(QuoteStyle::None);
        parts.push((current, style));
    }

    // Split into command and args. If parts is empty (blank input after trim),
    // return an empty command and no args.
    if parts.is_empty() {
        return (Vec::new(), Vec::new());
    }

    let mut iter = parts.into_iter();
    let (command, _) = iter.next().unwrap_or((Vec::new(), QuoteStyle::None));
    let args: Vec<(Vec<u8>, QuoteStyle)> = iter.collect();

    (command, args)
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
        let (cmd, args) = split_command_and_args(input);
        let arg_bytes = args.into_iter().map(|(b, _)| b).collect();
        (cmd, arg_bytes)
    }

    // Helper: split and return command + (bytes, style) pairs.
    fn split_styled(input: &[u8]) -> (Vec<u8>, Vec<(Vec<u8>, QuoteStyle)>) {
        split_command_and_args(input)
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
        // echo "hello    world" → one arg: "hello    world"
        let (command, args) = split(b"echo \"hello    world\"");
        assert_eq!(command, b"echo");
        assert_eq!(args, vec![b"hello    world".to_vec()]);
    }

    #[test]
    fn test_double_quote_adjacent_concatenation() {
        // echo "hello""world" → one arg: "helloworld"
        let (command, args) = split(b"echo \"hello\"\"world\"");
        assert_eq!(command, b"echo");
        assert_eq!(args, vec![b"helloworld".to_vec()]);
    }

    #[test]
    fn test_double_quote_mixed_adjacent_concatenation() {
        // echo hello"world" → one arg: "helloworld"
        let (command, args) = split(b"echo hello\"world\"");
        assert_eq!(command, b"echo");
        assert_eq!(args, vec![b"helloworld".to_vec()]);
    }

    #[test]
    fn test_double_quote_empty_quotes() {
        // echo "" → one empty arg
        let (command, args) = split(b"echo \"\"");
        assert_eq!(command, b"echo");
        assert_eq!(args, vec![b"".to_vec()]);
    }

    #[test]
    fn test_double_quote_multiple_args() {
        // echo "foo bar" "baz qux" → two args: "foo bar", "baz qux"
        let (command, args) = split(b"echo \"foo bar\" \"baz qux\"");
        assert_eq!(command, b"echo");
        assert_eq!(args, vec![b"foo bar".to_vec(), b"baz qux".to_vec()]);
    }

    #[test]
    fn test_double_quote_with_single_quote_inside() {
        // Single quotes inside double quotes are literal.
        let (command, args) = split(b"echo \"it's\"");
        assert_eq!(command, b"echo");
        assert_eq!(args, vec![b"it's".to_vec()]);
    }

    #[test]
    fn test_double_quote_unquoted_mixed() {
        // echo pre"mid"post → one arg: "premidpost"
        let (command, args) = split(b"echo pre\"mid\"post");
        assert_eq!(command, b"echo");
        assert_eq!(args, vec![b"premidpost".to_vec()]);
    }

    // --- Backslash-in-double-quote tests ---

    #[test]
    fn test_dquote_backslash_escapes_backslash() {
        // echo "A \\ escapes itself" → A \ escapes itself
        let (command, args) = split(b"echo \"A \\\\ escapes itself\"");
        assert_eq!(command, b"echo");
        assert_eq!(args, vec![b"A \\ escapes itself".to_vec()]);
    }

    #[test]
    fn test_dquote_backslash_escapes_dquote() {
        // echo "A \" inside double quotes" → A " inside double quotes
        let (command, args) = split(b"echo \"A \\\" inside double quotes\"");
        assert_eq!(command, b"echo");
        assert_eq!(args, vec![b"A \" inside double quotes".to_vec()]);
    }

    #[test]
    fn test_dquote_backslash_non_special_preserved() {
        // echo "\n" → \n (backslash preserved before non-special char)
        let (command, args) = split(b"echo \"\\n\"");
        assert_eq!(command, b"echo");
        assert_eq!(args, vec![b"\\n".to_vec()]);
    }

    // --- Single-quote tests ---

    #[test]
    fn test_single_quote_preserves_spaces() {
        // echo 'hello    world' → one arg: "hello    world"
        let (command, args) = split(b"echo 'hello    world'");
        assert_eq!(command, b"echo");
        assert_eq!(args, vec![b"hello    world".to_vec()]);
    }

    #[test]
    fn test_single_quote_adjacent_concatenation() {
        // echo 'hello''world' → one arg: "helloworld"
        let (command, args) = split(b"echo 'hello''world'");
        assert_eq!(command, b"echo");
        assert_eq!(args, vec![b"helloworld".to_vec()]);
    }

    #[test]
    fn test_single_quote_mixed_adjacent_concatenation() {
        // echo hello''world → one arg: "helloworld"
        let (command, args) = split(b"echo hello''world");
        assert_eq!(command, b"echo");
        assert_eq!(args, vec![b"helloworld".to_vec()]);
    }

    #[test]
    fn test_single_quote_backslash_literal() {
        // Inside single quotes, backslash is literal.
        let (command, args) = split(b"echo 'back\\slash'");
        assert_eq!(command, b"echo");
        assert_eq!(args, vec![b"back\\slash".to_vec()]);
    }

    #[test]
    fn test_single_quote_empty_quotes() {
        // echo hello''world → "helloworld" (empty quotes ignored mid-token)
        let (command, args) = split(b"echo hello''world");
        assert_eq!(command, b"echo");
        assert_eq!(args, vec![b"helloworld".to_vec()]);
    }

    // --- Backslash-escape tests (outside quotes) ---

    #[test]
    fn test_backslash_space_literal() {
        // echo three\ \ \ spaces → one arg: "three   spaces"
        let (command, args) = split(b"echo three\\ \\ \\ spaces");
        assert_eq!(command, b"echo");
        assert_eq!(args, vec![b"three   spaces".to_vec()]);
    }

    #[test]
    fn test_backslash_backslash() {
        // echo hello\\world → one arg: "hello\world"
        let (command, args) = split(b"echo hello\\\\world");
        assert_eq!(command, b"echo");
        assert_eq!(args, vec![b"hello\\world".to_vec()]);
    }

    #[test]
    fn test_backslash_letter() {
        // echo test\\nexample → one arg: "testnexample"  (backslash is consumed)
        let (command, args) = split(b"echo test\\nexample");
        assert_eq!(command, b"echo");
        assert_eq!(args, vec![b"testnexample".to_vec()]);
    }

    #[test]
    fn test_backslash_single_quote() {
        // echo \'hello\' → one arg: "'hello'"  (quotes are made literal)
        let (command, args) = split(b"echo \\'hello\\'");
        assert_eq!(command, b"echo");
        assert_eq!(args, vec![b"'hello'".to_vec()]);
    }

    #[test]
    fn test_backslash_inside_single_quote_is_literal() {
        // Inside single quotes, backslash is not special.
        // echo 'back\slash' → one arg: "back\slash"
        let (command, args) = split(b"echo 'back\\slash'");
        assert_eq!(command, b"echo");
        assert_eq!(args, vec![b"back\\slash".to_vec()]);
    }

    #[test]
    fn test_trailing_backslash_consumed_silently() {
        // A trailing backslash with no following character is silently consumed.
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
        // 'hello''world' is purely single-quoted
        let (_, args) = split_styled(b"echo 'hello''world'");
        assert_eq!(args, vec![(b"helloworld".to_vec(), QuoteStyle::Single)]);
    }

    #[test]
    fn test_style_mixed_is_none() {
        // pre"mid"post has unquoted bytes → QuoteStyle::None
        let (_, args) = split_styled(b"echo pre\"mid\"post");
        assert_eq!(args, vec![(b"premidpost".to_vec(), QuoteStyle::None)]);
    }

    #[test]
    fn test_style_single_then_double_is_none() {
        // 'a'"b" mixes single and double → QuoteStyle::None
        let (_, args) = split_styled(b"echo 'a'\"b\"");
        assert_eq!(args, vec![(b"ab".to_vec(), QuoteStyle::None)]);
    }

    // --- Redirect extraction tests ---

    #[test]
    fn test_parse_redirect_gt() {
        let (_, args, redirect, stderr_redirect) = parse(b"echo hello > out.txt");
        assert_eq!(args.iter().map(|a| a.to_string()).collect::<Vec<_>>(), vec!["hello"]);
        assert!(redirect.is_some(), "redirect should be Some");
        assert_eq!(redirect.unwrap().target, std::path::PathBuf::from("out.txt"));
        assert!(stderr_redirect.is_none());
    }

    #[test]
    fn test_parse_redirect_1gt() {
        let (_, args, redirect, stderr_redirect) = parse(b"echo hello 1> out.txt");
        assert_eq!(args.iter().map(|a| a.to_string()).collect::<Vec<_>>(), vec!["hello"]);
        assert!(redirect.is_some(), "redirect should be Some for 1>");
        assert_eq!(redirect.unwrap().target, std::path::PathBuf::from("out.txt"));
        assert!(stderr_redirect.is_none());
    }

    #[test]
    fn test_parse_redirect_2gt() {
        let (_, args, redirect, stderr_redirect) = parse(b"echo hello 2> err.txt");
        assert_eq!(args.iter().map(|a| a.to_string()).collect::<Vec<_>>(), vec!["hello"]);
        assert!(redirect.is_none(), "stdout redirect should be None for 2>");
        assert!(stderr_redirect.is_some(), "stderr redirect should be Some for 2>");
        assert_eq!(stderr_redirect.unwrap().target, std::path::PathBuf::from("err.txt"));
    }

    #[test]
    fn test_parse_redirect_trailing_operator_kept_as_arg() {
        // A trailing `>` with no following filename should be preserved as a literal argument.
        let (_, args, redirect, _) = parse(b"echo >");
        assert!(redirect.is_none(), "no redirect target means no Redirect");
        assert_eq!(
            args.iter().map(|a| a.to_string()).collect::<Vec<_>>(),
            vec![">"],
            "trailing `>` should be kept as a literal arg"
        );
    }

    #[test]
    fn test_parse_stderr_redirect_trailing_operator_kept_as_arg() {
        // A trailing `2>` with no following filename should be preserved as a literal argument.
        let (_, args, _, stderr_redirect) = parse(b"echo 2>");
        assert!(stderr_redirect.is_none(), "no redirect target means no StderrRedirect");
        assert_eq!(
            args.iter().map(|a| a.to_string()).collect::<Vec<_>>(),
            vec!["2>"],
            "trailing `2>` should be kept as a literal arg"
        );
    }

    #[test]
    fn test_parse_no_redirect() {
        let (_, args, redirect, stderr_redirect) = parse(b"echo hello world");
        assert_eq!(
            args.iter().map(|a| a.to_string()).collect::<Vec<_>>(),
            vec!["hello", "world"]
        );
        assert!(redirect.is_none());
        assert!(stderr_redirect.is_none());
    }
}
