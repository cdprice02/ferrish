use std::fmt::Display;

pub(crate) mod builtin;
pub(crate) mod executable;

/// The kind of a resolved shell command.
#[derive(Debug, Clone)]
pub enum CommandKind {
    /// A shell built-in command (e.g. `echo`, `cd`, `exit`).
    BuiltIn(builtin::BuiltInCommand),
    /// An external executable found on `PATH`.
    Executable(executable::ExecutableCommand),
}

impl PartialEq for CommandKind {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (CommandKind::BuiltIn(a), CommandKind::BuiltIn(b)) => a == b,
            (CommandKind::Executable(a), CommandKind::Executable(b)) => a == b,
            _ => false,
        }
    }
}

impl Eq for CommandKind {}

impl Display for CommandKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CommandKind::BuiltIn(builtin) => write!(f, "{}", builtin),
            CommandKind::Executable(executable) => write!(f, "{}", executable),
        }
    }
}
