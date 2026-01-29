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
    Unrecognized(Vec<u8>),
}

macro_rules! builtin {
    ($name:ident) => {
        Command::BuiltIn(BuiltInCommand { name: $name })
    };
}

macro_rules! unrecognized {
    ($name:expr) => {
        Command::Unrecognized($name.into())
    };
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

type Args = Vec<Arg>;

#[derive(Debug, Clone, PartialEq, Eq)]
enum Arg {
    Literal(Vec<u8>),
}

impl Display for Arg {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Arg::Literal(bytes) => write!(f, "{}", String::from_utf8_lossy(bytes)),
        }
    }
}

impl From<&Arg> for PathBuf {
    fn from(val: &Arg) -> Self {
        match val {
            Arg::Literal(_) => PathBuf::from(val.to_string()),
        }
    }
}

impl Arg {
    fn to_command(&self) -> Command {
        parse_command(match self {
            Arg::Literal(bytes) => bytes,
        })
    }

    fn from_bytes(bytes: &[u8]) -> Self {
        Arg::Literal(bytes.to_vec())
    }
}

fn parse(buffer: &[u8]) -> (Command, Args) {
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

fn parse_command(command: &[u8]) -> Command {
    if !command.is_ascii() {
        return unrecognized!(command);
    }

    let command = std::str::from_utf8(command).expect("checked ASCII above");

    let name = BuiltInName::from_str(command);
    if let Ok(name) = name {
        builtin!(name)
    } else {
        for file in get_path_files().filter(|p| p.is_executable()) {
            let executable_command = ExecutableCommand { file_path: file };

            if executable_command.name() == command {
                return Command::Executable(executable_command);
            }
        }

        unrecognized!(command)
    }
}

fn parse_arg(arg: &[u8]) -> Arg {
    Arg::Literal(arg.to_vec())
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
        write!(stdout, "🦀> ")?;
        stdout.flush()?;

        let mut buffer = Vec::<u8>::new();
        stdin.read_until(b'\n', &mut buffer)?;

        let buffer = buffer.trim_ascii();
        if buffer.is_empty() {
            // Empty command, just prompt again
            continue;
        }

        let (command, args) = parse(buffer);

        match command {
            Command::BuiltIn(BuiltInCommand { name }) => match name {
                BuiltInName::Exit => break,
                BuiltInName::Echo => writeln!(
                    stdout,
                    "{}",
                    args.iter()
                        .map(|a| a.to_string())
                        .collect::<Vec<_>>()
                        .join(" ")
                )?,
                BuiltInName::Type => {
                    if args.is_empty() {
                        writeln!(stdout, "{}: missing operand", name)?;
                    } else {
                        match args.first().expect("at least one arg").to_command() {
                            Command::BuiltIn(builtin) => {
                                writeln!(stdout, "{} is a shell builtin", builtin)?
                            }
                            Command::Executable(executable) => writeln!(
                                stdout,
                                "{} is {}",
                                executable,
                                executable.file_path.display()
                            )?,
                            Command::Unrecognized(name) => {
                                writeln!(stdout, "{}: not found", String::from_utf8_lossy(&name))?
                            }
                        }
                    }
                }
                BuiltInName::Pwd => writeln!(stdout, "{}", working_dir.display())?,
                BuiltInName::Cd => {
                    let default_target = Arg::from_bytes(b"~");
                    let target = args.first().unwrap_or(&default_target);
                    let new_dir = resolve_path(&target.into())?;

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
                let args = args.iter().map(|a| a.to_string()).collect::<Vec<_>>();
                let mut child = std::process::Command::new(executable.file_path.clone())
                    .args(args)
                    .spawn()?;
                let status = child.wait()?;

                if !status.success() {
                    writeln!(stdout, "{}: exited with status {}", executable, status)?;
                }
            }
            Command::Unrecognized(name) => {
                writeln!(stdout, "{}: not found", String::from_utf8_lossy(&name))?
            }
        };
        stdout.flush()?;
    }

    Ok(())
}
