use std::str::FromStr;

use is_executable::IsExecutable;

use crate::arg::{Arg, Args};
use crate::command::builtin::BuiltInCommand;
use crate::command::executable::ExecutableCommand;
use crate::command::{Command, builtin};
use crate::env::get_path_files;

pub fn parse(buffer: &[u8]) -> (Command, Args) {
    let (command, args) = split_command_and_args(buffer);
    let command = parse_command(command);
    let args = args.into_iter().map(parse_arg).collect();
    (command, args)
}

fn split_command_and_args(buffer: &[u8]) -> (&[u8], Vec<&[u8]>) {
    let mut parts = Vec::<&[u8]>::new();
    let mut start = 0;

    let buffer = buffer.trim_ascii();
    for (i, byte) in buffer.iter().enumerate() {
        if byte.is_ascii_whitespace() {
            if i - start > 0 {
                parts.push(&buffer[start..i]);
            }

            start = i + 1;
        }
        // TODO: handle quotes
    }

    parts.push(&buffer[start..]);

    let (command, args) = parts
        .split_first()
        .unwrap_or((parts.first().expect("at least one part"), &[]));
    let args = args.into();

    (command, args)
}

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

pub fn parse_arg(arg: &[u8]) -> Arg {
    Arg::Literal(arg.to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_split_command_and_args() {
        let buffer = b"  ls   -la  /home/user  ";
        let (command, args) = split_command_and_args(buffer);
        assert_eq!(command, b"ls");
        assert_eq!(args, vec!["-la".as_bytes(), "/home/user".as_bytes()]);
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
        let buffer = b"ls";
        let (command, args) = split_command_and_args(buffer);
        assert_eq!(command, b"ls");
        assert!(args.is_empty());
    }

    #[test]
    fn test_split_command_and_args_single_arg() {
        let buffer = b"ls -la";
        let (command, args) = split_command_and_args(buffer);
        assert_eq!(command, b"ls");
        assert_eq!(args, vec!["-la".as_bytes()]);
    }

    #[test]
    fn test_split_command_and_args_multiple_spaces() {
        let buffer = b"    command    arg1    arg2    arg3    ";
        let (command, args) = split_command_and_args(buffer);
        assert_eq!(command, b"command");
        assert_eq!(
            args,
            vec!["arg1".as_bytes(), "arg2".as_bytes(), "arg3".as_bytes()]
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
}
