pub(crate) mod builtin;
pub(crate) mod executable;

#[derive(Debug)]
pub enum Command {
    BuiltIn(builtin::BuiltInCommand),
    Executable(executable::ExecutableCommand),
    Unrecognized(Vec<u8>),
}

impl Command {
    pub fn builtin(name: builtin::BuiltInName) -> Self {
        Command::BuiltIn(builtin::BuiltInCommand::new(name))
    }

    pub fn unrecognized(name: Vec<u8>) -> Self {
        Command::Unrecognized(name)
    }
}
