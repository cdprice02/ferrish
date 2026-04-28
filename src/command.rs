use std::fmt::Display;

use miette::SourceSpan;

use crate::input::InputSource;

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
    Unrecognized {
        /// The raw bytes of the unrecognized command token.
        bytes: Vec<u8>,
        /// Byte offset and length of this token in the raw input line.
        span: SourceSpan,
        /// The raw input line this token was parsed from.
        src: InputSource,
    },
}

impl PartialEq for Command {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Command::BuiltIn(a), Command::BuiltIn(b)) => a == b,
            (Command::Executable(a), Command::Executable(b)) => a == b,
            (Command::Unrecognized { bytes: a, .. }, Command::Unrecognized { bytes: b, .. }) => {
                a == b
            }
            _ => false,
        }
    }
}

impl Eq for Command {}

impl Display for Command {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Command::BuiltIn(builtin) => write!(f, "{}", builtin),
            Command::Executable(executable) => write!(f, "{}", executable),
            Command::Unrecognized { bytes, .. } => {
                write!(f, "{}", String::from_utf8_lossy(bytes))
            }
        }
    }
}
