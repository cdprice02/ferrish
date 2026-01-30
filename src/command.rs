use std::fmt::Display;

pub(crate) mod builtin;
pub(crate) mod executable;

#[derive(Debug, Clone)]
pub enum Command {
    BuiltIn(builtin::BuiltInCommand),
    Executable(executable::ExecutableCommand),
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

impl Command {
    pub fn builtin(name: builtin::BuiltInName) -> Self {
        Command::BuiltIn(builtin::BuiltInCommand::new(name))
    }

    pub fn unrecognized(name: Vec<u8>) -> Self {
        Command::Unrecognized(name)
    }
}
