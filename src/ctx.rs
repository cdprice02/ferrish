use std::path::PathBuf;

use crate::env;

pub struct ShellConfig {
    pub prompt: String,
    pub history_path: Option<PathBuf>,
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

pub struct ShellCtx {
    pub home_dir: Option<PathBuf>,
    pub cwd: PathBuf,
    pub config: ShellConfig,
}

impl ShellCtx {
    pub fn new(home_dir: Option<PathBuf>, cwd: PathBuf) -> Self {
        Self { home_dir, cwd, config: ShellConfig::default() }
    }

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
