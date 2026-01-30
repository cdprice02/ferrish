use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct ExecutableCommand {
    file_path: PathBuf,
}

impl std::fmt::Display for ExecutableCommand {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name())
    }
}

impl ExecutableCommand {
    pub fn new(file_path: PathBuf) -> Self {
        Self { file_path }
    }

    pub fn file_path(&self) -> &PathBuf {
        &self.file_path
    }

    pub fn name(&self) -> &str {
        self.file_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("")
    }
}
