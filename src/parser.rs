use std::str::FromStr;

use is_executable::IsExecutable;

use crate::arg::{Arg, Args};
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

pub(crate) fn parse_command(command: &[u8]) -> Command {
    if !command.is_ascii() {
        return Command::unrecognized(command.into());
    }

    let command = std::str::from_utf8(command).expect("checked ASCII above");

    let name = builtin::BuiltInName::from_str(command);
    if let Ok(name) = name {
        Command::builtin(name)
    } else {
        for file in get_path_files().filter(|p| p.is_executable()) {
            let executable_command = ExecutableCommand::new(file);

            if executable_command.name() == command {
                return Command::Executable(executable_command);
            }
        }

        Command::unrecognized(command.into())
    }
}

pub(crate) fn parse_arg(arg: &[u8]) -> Arg {
    Arg::Literal(arg.to_vec())
}
