/// A resolved shell built-in command ready for execution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuiltInCommand {
    name: BuiltInName,
}

impl std::fmt::Display for BuiltInCommand {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name)
    }
}

impl BuiltInCommand {
    /// Create a new `BuiltInCommand` for the given built-in name.
    pub fn new(name: BuiltInName) -> Self {
        Self { name }
    }

    /// Return the built-in name for this command.
    pub fn name(&self) -> BuiltInName {
        self.name
    }
}

/// The set of built-in commands supported by the shell.
#[derive(strum::EnumString, strum::AsRefStr, strum::Display, Debug, Clone, Copy, PartialEq, Eq)]
#[strum(serialize_all = "lowercase")]
pub enum BuiltInName {
    /// Terminate the shell process.
    Exit,
    /// Write arguments to standard output.
    Echo,
    /// Report the type of a command (built-in, executable, or unknown).
    Type,
    /// Print the current working directory.
    Pwd,
    /// Change the current working directory.
    Cd,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn test_builtin_name_parses_from_string() {
        assert_eq!(BuiltInName::from_str("exit").unwrap(), BuiltInName::Exit);
        assert_eq!(BuiltInName::from_str("echo").unwrap(), BuiltInName::Echo);
        assert_eq!(BuiltInName::from_str("pwd").unwrap(), BuiltInName::Pwd);
        assert_eq!(BuiltInName::from_str("cd").unwrap(), BuiltInName::Cd);
        assert_eq!(BuiltInName::from_str("type").unwrap(), BuiltInName::Type);
    }

    #[test]
    fn test_builtin_command_new_and_name() {
        let cmd = BuiltInCommand::new(BuiltInName::Echo);
        assert_eq!(cmd.name(), BuiltInName::Echo);
    }

    #[test]
    fn test_builtin_command_display() {
        let cmd = BuiltInCommand::new(BuiltInName::Exit);
        assert_eq!(cmd.to_string(), "exit");
        let cmd = BuiltInCommand::new(BuiltInName::Pwd);
        assert_eq!(cmd.to_string(), "pwd");
    }
}
