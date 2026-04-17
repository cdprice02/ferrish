use std::path::PathBuf;

/// An external executable command located on `PATH`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecutableCommand {
    file_path: PathBuf,
}

impl std::fmt::Display for ExecutableCommand {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name())
    }
}

impl ExecutableCommand {
    /// Create a new `ExecutableCommand` for the given path.
    pub fn new(file_path: PathBuf) -> Self {
        Self { file_path }
    }

    /// Return the path to the executable.
    pub fn file_path(&self) -> &PathBuf {
        &self.file_path
    }

    /// Return the executable name (file stem without extension).
    pub fn name(&self) -> &str {
        self.file_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_executable_name_extracts_file_stem() {
        let cmd = ExecutableCommand::new(PathBuf::from("/usr/bin/ls"));
        assert_eq!(cmd.name(), "ls");

        let cmd = ExecutableCommand::new(PathBuf::from("./cargo"));
        assert_eq!(cmd.name(), "cargo");
    }

    #[test]
    fn test_executable_file_path_getter() {
        let path = PathBuf::from("/usr/local/bin/rustc");
        let cmd = ExecutableCommand::new(path.clone());
        assert_eq!(cmd.file_path(), &path);
    }

}
