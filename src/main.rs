use std::{
    fmt::Display,
    io::{self, BufRead, Write},
};
use thiserror::Error;

#[derive(Error, Debug)]
enum ShellError {
    #[error("{0}: not found")]
    CommandNotFound(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CommandType {
    BuiltIn,
    Executable,
    Unrecognized,
}

impl Display for CommandType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let typ = match self {
            CommandType::BuiltIn => "shell builtin",
            CommandType::Executable => "executable",
            CommandType::Unrecognized => "unrecognized",
        };
        write!(f, "{}", typ)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CommandName {
    Exit,
    Echo,
    Type,
}

impl Display for CommandName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let name = match self {
            CommandName::Exit => "exit",
            CommandName::Echo => "echo",
            CommandName::Type => "type",
        };
        write!(f, "{}", name)
    }
}

#[derive(Debug)]
struct Command {
    name: CommandName,
    typ: CommandType,
}

fn parse_command(command: &str) -> Result<Command, ShellError> {
    macro_rules! builtin {
        ($name:ident) => {
            Ok(Command {
                name: CommandName::$name,
                typ: CommandType::BuiltIn,
            })
        };
    }

    match command {
        "exit" => builtin!(Exit),
        "echo" => builtin!(Echo),
        "type" => builtin!(Type),
        _ => Err(ShellError::CommandNotFound(String::from(command))),
    }
}

fn main() -> anyhow::Result<()> {
    let stdin = io::stdin();
    let mut stdin = stdin.lock();
    let stdout = io::stdout();
    let mut stdout = stdout.lock();

    loop {
        write!(stdout, "$ ")?;
        stdout.flush()?;

        let mut buffer = String::new();
        stdin.read_line(&mut buffer)?;
        let buffer = buffer.trim();

        let (command, args) = if buffer.contains(' ') {
            let (command, args) = buffer.split_once(' ').expect("buffer contains ` `");
            let args = args.split(' ').collect::<Vec<_>>();
            (command, args)
        } else {
            (buffer, vec![])
        };

        let command = parse_command(command)?;
        match command {
            Command {
                name: CommandName::Exit,
                ..
            } => break,
            Command {
                name: CommandName::Echo,
                ..
            } => write!(stdout, "{}", args.join(" "))?,
            Command {
                name: CommandName::Type,
                ..
            } => {
                assert!(!args.is_empty(), "type must have at least one arg");
                match parse_command(args[0]) {
                    Ok(command) => write!(stdout, "{} is a {}", command.name, command.typ)?,
                    Err(e) => write!(stdout, "{}", e)?,
                }
            }
        }
        writeln!(stdout)?;
        stdout.flush()?;
    }

    Ok(())
}
