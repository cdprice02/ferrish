use std::path::PathBuf;

use crate::env;

/// Configurable shell settings.
pub struct ShellConfig {
    /// The prompt string displayed before each input line.
    pub prompt: String,
    /// Optional path to the shell history file.
    pub history_path: Option<PathBuf>,
    /// Maximum number of history entries to retain.
    pub max_history: usize,
}

impl Default for ShellConfig {
    fn default() -> Self {
        Self {
            prompt: "\u{1F980}> ".to_string(), // 🦀>
            history_path: None,
            max_history: 1000,
        }
    }
}

/// Runtime context shared across all shell operations.
pub struct ShellCtx {
    /// The user's home directory, if known.
    pub home_dir: Option<PathBuf>,
    /// The current working directory.
    pub cwd: PathBuf,
    /// Shell configuration.
    pub config: ShellConfig,
}

impl ShellCtx {
    /// Create a new context with default configuration.
    pub fn new(home_dir: Option<PathBuf>, cwd: PathBuf) -> Self {
        Self { home_dir, cwd, config: ShellConfig::default() }
    }

    /// Create a new context with explicit configuration.
    pub fn with_config(home_dir: Option<PathBuf>, cwd: PathBuf, config: ShellConfig) -> Self {
        Self { home_dir, cwd, config }
    }

    /// Initialize from the current process environment.
    pub fn from_env() -> Self {
        Self {
            home_dir: env::home_dir(),
            cwd: env::current_dir().unwrap_or_else(|_| PathBuf::from("/")),
            config: ShellConfig::default(),
        }
    }
}
