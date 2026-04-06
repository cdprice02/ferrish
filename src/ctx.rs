use std::path::PathBuf;

use crate::env;

pub struct ShellCtx {
    pub home_dir: Option<PathBuf>,
    pub cwd: PathBuf,
}

impl ShellCtx {
    pub fn new(home_dir: Option<PathBuf>, cwd: PathBuf) -> Self {
        Self { home_dir, cwd }
    }

    /// Initialize from the current process environment.
    pub fn from_env() -> Self {
        Self {
            home_dir: env::home_dir(),
            cwd: env::current_dir().unwrap_or_else(|_| PathBuf::from("/")),
        }
    }
}
