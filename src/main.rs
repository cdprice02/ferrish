use is_executable::IsExecutable;
use std::env;
use std::fs;
use std::path::PathBuf;
use std::str::FromStr;
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
    name: BuiltInName,
}

impl Display for BuiltInCommand {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name)
    }
}

#[derive(strum::EnumString, strum::AsRefStr, strum::Display, Debug, Clone, Copy, PartialEq, Eq)]
enum BuiltInName {
    #[strum(serialize = "exit")]
    Exit,
    #[strum(serialize = "echo")]
    Echo,
    #[strum(serialize = "type")]
    Type,
    #[strum(serialize = "pwd")]
    Pwd,
    #[strum(serialize = "cd")]
    Cd,
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
            Command::BuiltIn(BuiltInCommand { name: $name })
        };
    }

    let name = BuiltInName::from_str(command);
    if let Ok(name) = name {
        builtin!(name)
    } else {
        for file in get_path_files() {
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

fn get_path_files() -> impl Iterator<Item = PathBuf> {
    let path = env::var_os("PATH").unwrap_or_default();
    env::split_paths(&path)
        .collect::<Vec<_>>()
        .into_iter()
        .filter(|d| d.is_dir() && d.exists())
        .flat_map(|d| {
            fs::read_dir(d)
                .ok()
                .into_iter()
                .flatten()
                .filter_map(|entry| entry.ok().map(|e| e.path()))
                .collect::<Vec<_>>()
        })
}

fn resolve_path(path: &PathBuf) -> io::Result<PathBuf> {
    let path = if path.is_absolute() {
        path.clone()
    } else if let Ok(stripped) = path.strip_prefix("~") {
        let home_dir = env::home_dir().unwrap_or_else(|| PathBuf::from("/"));
        let stripped = stripped.strip_prefix("/").unwrap_or(stripped);
        home_dir.join(stripped)
    } else {
        let current_dir = env::current_dir()?;
        current_dir.join(path)
    };

    soft_canonicalize::soft_canonicalize(path)
}

fn main() -> anyhow::Result<()> {
    let stdin = io::stdin();
    let mut stdin = stdin.lock();
    let stdout = io::stdout();
    let mut stdout = stdout.lock();

    let mut working_dir = env::current_dir()?;

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
                BuiltInName::Exit => break,
                BuiltInName::Echo => writeln!(stdout, "{}", args.join(" "))?,
                BuiltInName::Type => {
                    if args.is_empty() {
                        writeln!(stdout, "{}: missing operand", name)?;
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
                BuiltInName::Pwd => writeln!(stdout, "{}", working_dir.display())?,
                BuiltInName::Cd => {
                    let target = args.first().unwrap_or(&"~");
                    let new_dir = PathBuf::from(target);
                    let new_dir = resolve_path(&new_dir)?;

                    if !new_dir.exists() {
                        writeln!(stdout, "{}: no such file or directory: {}", name, target)?;
                    } else if !new_dir.is_dir() {
                        writeln!(stdout, "{}: not a directory: {}", name, target)?;
                    } else {
                        match env::set_current_dir(new_dir) {
                            Ok(()) => {
                                working_dir = env::current_dir().expect("current_dir was just set");
                            }
                            Err(e) if e.kind() == io::ErrorKind::PermissionDenied => {
                                writeln!(stdout, "{}: permission denied: {}", name, target)?;
                            }
                            Err(e) => {
                                writeln!(stdout, "{}: {}", name, e)?;
                            }
                        }
                    }
                }
            },
            Command::Executable(executable) => {
                let mut child = std::process::Command::new(executable.file_path.clone())
                    .args(args)
                    .spawn()?;
                let status = child.wait()?;

                if !status.success() {
                    writeln!(stdout, "{}: exited with status {}", executable, status)?;
                }
            }
            Command::Unrecognized(name) => writeln!(stdout, "{}: not found", name)?,
        };
        stdout.flush()?;
    }

    Ok(())
}
