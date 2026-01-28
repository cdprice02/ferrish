use is_executable::IsExecutable;
use std::env;
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
    fn name(&self) -> &str {
        self.file_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("")
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
            let path = env::var_os("PATH").unwrap_or_default();
            let dirs = env::split_paths(&path);
            let files = dirs.flat_map(|d| {
                if d.is_dir() {
                    match fs::read_dir(d) {
                        Ok(entries) => entries
                            .filter_map(|entry_res| entry_res.ok().map(|entry| entry.path()))
                            .collect::<Vec<_>>(),
                        Err(_) => Vec::new(),
                    }
                } else {
                    // Not a directory, skip it
                    Vec::new()
                }
            });

            for file in files {
                if !file.is_executable() {
                    continue;
                }

                let executable_command = ExecutableCommand { file_path: file };

                if executable_command.name() == command {
                    return Command::Executable(executable_command);
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

        let (command, args) = if buffer.is_empty() {
            (buffer, Vec::new())
        } else {
            let mut parts = buffer.split_whitespace();
            let command = parts.next().expect("buffer is not empty");
            let args = parts.collect::<Vec<_>>();
            (command, args)
        };

        match parse_command(command) {
            Command::BuiltIn(BuiltInCommand { name }) => match name {
                CommandName::Exit => break,
                CommandName::Echo => writeln!(stdout, "{}", args.join(" "))?,
                CommandName::Type => {
                    if args.is_empty() {
                        writeln!(stdout, "type: missing operand")?;
                    } else {
                        match parse_command(args[0]) {
                            Command::BuiltIn(builtin) => {
                                writeln!(stdout, "{} is a shell builtin", builtin)?
                            }
                            Command::Executable(executable) => writeln!(
                                stdout,
                                "{} is {}",
                                executable,
                                executable.file_path.display()
                            )?,
                            Command::Unrecognized(name) => writeln!(stdout, "{}: not found", name)?,
                        }
                    }
                }
            },
            Command::Executable(executable) => {
                let output = std::process::Command::new(executable.file_path)
                    .args(args)
                    .output()?;
                stdout.write_all(&output.stdout)?;
                stdout.write_all(&output.stderr)?;
            }
            Command::Unrecognized(name) => writeln!(stdout, "{}: not found", name)?,
        };
        stdout.flush()?;
    }

    Ok(())
}
