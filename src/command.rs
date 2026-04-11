use std::fmt::Display;

pub(crate) mod builtin;
pub(crate) mod executable;

/// Represents a shell command.
#[derive(Debug, Clone)]
pub enum Command {
    /// A shell built-in command (e.g. `echo`, `cd`, `exit`).
    BuiltIn(builtin::BuiltInCommand),
    /// An external executable found on `PATH`.
    Executable(executable::ExecutableCommand),
    /// A command token that could not be resolved to a built-in or executable.
    Unrecognized(Vec<u8>),
}

impl Display for Command {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Command::BuiltIn(builtin) => write!(f, "{}", builtin),
            Command::Executable(executable) => write!(f, "{}", executable),
            Command::Unrecognized(bytes) => write!(f, "{}", String::from_utf8_lossy(bytes)),
        }
    }
}
