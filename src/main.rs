use is_executable::IsExecutable;
use std::env;
use std::ffi::OsString;
use std::fs;
use std::path::PathBuf;
use std::{
    fmt::Display,
    io::{self, BufRead, Write},
};

#[derive(Debug)]
enum Command {
    BuiltIn(BuiltInCommand),
    Executable(ExecutableCommand),
    Unrecognized(String),
}

#[derive(Debug)]
struct BuiltInCommand {
    name: CommandName,
}

impl Display for BuiltInCommand {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name.as_ref())
    }
}

#[derive(strum::EnumString, strum::AsRefStr, Debug, Clone, Copy, PartialEq, Eq)]
enum CommandName {
    #[strum(serialize = "exit")]
    Exit,
    #[strum(serialize = "echo")]
    Echo,
    #[strum(serialize = "type")]
    Type,
}

#[derive(Debug)]
struct ExecutableCommand {
    file_path: PathBuf,
}

impl Display for ExecutableCommand {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name())
    }
}

impl ExecutableCommand {
    pub(crate) fn name(&self) -> &str {
        self.file_path
            .file_stem()
            .unwrap_or_default()
            .to_str()
            .expect("executable file path is valid unicode")
    }
}

fn parse_command(command: &str) -> Command {
    macro_rules! builtin {
        ($name:ident) => {
            Command::BuiltIn(BuiltInCommand {
                name: CommandName::$name,
            })
        };
    }

    match command {
        "exit" => builtin!(Exit),
        "echo" => builtin!(Echo),
        "type" => builtin!(Type),
        command => {
            let path: OsString = env::var_os("PATH").expect("machine has env PATH set");
            let dirs = env::split_paths(&path);
            let files = dirs.flat_map(|d| {
                if d.is_dir() {
                    fs::read_dir(d)
                        .expect("PATH dirs exist and have read permissions")
                        .map(|entry| {
                            entry
                                .expect("PATH files exist and have read permissions")
                                .path()
                        })
                        .collect::<Vec<_>>()
                } else {
                    vec![d]
                }
            });

            for file in files {
                let stem = file.file_stem();
                if stem.is_none() {
                    continue;
                }
                let name = stem.unwrap().to_str().expect("PATH name is valid unicode");

                if name == command && file.is_executable() {
                    return Command::Executable(ExecutableCommand { file_path: file });
                }
            }

            Command::Unrecognized(command.to_string())
        }
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

        match parse_command(command) {
            Command::BuiltIn(BuiltInCommand { name }) => match name {
                CommandName::Exit => break,
                CommandName::Echo => write!(stdout, "{}", args.join(" "))?,
                CommandName::Type => {
                    assert!(!args.is_empty(), "type must have at least one arg");
                    match parse_command(args[0]) {
                        Command::BuiltIn(builtin) => {
                            write!(stdout, "{} is a shell builtin", builtin)?
                        }
                        Command::Executable(executable) => write!(
                            stdout,
                            "{} is {}",
                            executable,
                            executable.file_path.display()
                        )?,
                        Command::Unrecognized(name) => write!(stdout, "{}: not found", name)?,
                    }
                }
            },
            Command::Executable(_) => todo!("Execute the command"),
            Command::Unrecognized(name) => write!(stdout, "{}: not found", name)?,
        };
        writeln!(stdout)?;
        stdout.flush()?;
    }

    Ok(())
}
