use std::str::FromStr;

use is_executable::IsExecutable;

use crate::arg::{Arg, Args};
use crate::command::builtin::BuiltInCommand;
use crate::command::executable::ExecutableCommand;
use crate::command::{Command, builtin};
use crate::env::get_path_files;

/// Parse a raw input line into a [`Command`] and its argument list.
pub fn parse(buffer: &[u8]) -> (Command, Args) {
    let (command, args) = split_command_and_args(buffer);
    let command = parse_command(&command);
    let args = args.into_iter().map(Arg::Literal).collect();
    (command, args)
}

/// Split the trimmed input buffer into a command token and zero or more argument tokens.
///
/// Handles single-quoted strings: characters inside `'...'` are treated literally —
/// whitespace is preserved (not used as a delimiter) and backslashes have no special
/// meaning. Adjacent quoted and unquoted segments are concatenated into a single token.
fn split_command_and_args(buffer: &[u8]) -> (Vec<u8>, Vec<Vec<u8>>) {
    let buffer = buffer.trim_ascii();
    let mut parts: Vec<Vec<u8>> = Vec::new();

    let mut current: Vec<u8> = Vec::new();
    let mut in_single_quote = false;
    // Track whether we've started building a token (needed to emit empty tokens
    // only when inside a quoted empty string at the word boundary — but for
    // single-quotes the shell spec says empty quotes produce an empty argument
    // only if they stand alone, which the concatenation logic handles naturally).
    let mut token_started = false;

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
        } else if byte == b'\'' {
            // Opening single quote — enter single-quote mode and mark token as started.
            // Even empty quotes (e.g. `''`) count as beginning a token so that a
            // standalone `''` produces one empty argument rather than being dropped.
            in_single_quote = true;
            token_started = true;
        } else if byte.is_ascii_whitespace() {
            if token_started || !current.is_empty() {
                parts.push(current);
                current = Vec::new();
                token_started = false;
            }
        } else {
            current.push(byte);
            token_started = true;
        }

        i += 1;
    }

    // Push the last token (may be empty if input was all whitespace after trim,
    // but trim_ascii above ensures the buffer is non-empty when we reach here).
    if token_started || !current.is_empty() {
        parts.push(current);
    }

    // Split into command and args. If parts is empty (blank input after trim),
    // return an empty command and no args.
    if parts.is_empty() {
        return (Vec::new(), Vec::new());
    }

    let mut iter = parts.into_iter();
    let command = iter.next().unwrap_or_default();
    let args: Vec<Vec<u8>> = iter.collect();

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

    // Helper: split and compare command + args as byte slices.
    fn split(input: &[u8]) -> (Vec<u8>, Vec<Vec<u8>>) {
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
        match parsed_arg {
            Arg::Literal(bytes) => assert_eq!(bytes, arg),
        }
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
        match parsed_arg {
            Arg::Literal(bytes) => assert!(bytes.is_empty()),
        }
    }

    #[test]
    fn test_parse_arg_special_chars() {
        let arg = "file-with_special.chars".as_bytes();
        let parsed_arg = parse_arg(arg);
        match parsed_arg {
            Arg::Literal(bytes) => assert_eq!(bytes, arg),
        }
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
}
