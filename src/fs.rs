use std::{io, path::PathBuf};

use crate::env;

pub(crate) fn resolve_path(path: &PathBuf) -> io::Result<PathBuf> {
    let path = if path.is_absolute() {
        path.clone()
    } else if let Ok(stripped) = path.strip_prefix("~") {
        let home_dir = env::home_dir().unwrap_or_else(|| PathBuf::from("/"));
        let stripped = stripped.strip_prefix("/").unwrap_or(stripped);
        home_dir.join(stripped)
    } else {
        let current_dir = env::current_dir()
            .unwrap_or_else(|| env::home_dir().unwrap_or_else(|| PathBuf::from("/")));
        current_dir.join(path)
    };

    canonicalize_path(path)
}

pub fn canonicalize_path(path: PathBuf) -> io::Result<PathBuf> {
    soft_canonicalize::soft_canonicalize(path)
}
