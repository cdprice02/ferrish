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
        env::current_dir()?.join(path)
    };

    canonicalize_path(path)
}

pub fn canonicalize_path(path: PathBuf) -> io::Result<PathBuf> {
    soft_canonicalize::soft_canonicalize(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_path_absolute() {
        let path = PathBuf::from("/usr/bin");
        let resolved = resolve_path(&path);
        assert!(resolved.is_ok(), "Failed to resolve absolute path");
        let resolved = resolved.unwrap();
        if cfg!(target_os = "windows") {
            // On Windows, canonicalizing /usr/bin may lead to different results
            // depending on the environment. So we just check that it ends with usr\bin
            assert!(resolved.ends_with("usr\\bin") || resolved.ends_with("usr/bin"));
        } else {
            assert_eq!(resolved, PathBuf::from("/usr/bin"));
        }
    }

    #[test]
    fn test_resolve_path_relative_dots_current() {
        let current_dir = env::current_dir().unwrap();
        let path = PathBuf::from("./tmp123");
        let resolved = resolve_path(&path);
        assert!(resolved.is_ok(), "Failed to resolve relative path with .");
        let resolved = resolved.unwrap();
        assert_eq!(resolved, current_dir.join("tmp123"));
    }

    #[test]
    fn test_resolve_path_relative_dots_parent() {
        let current_dir = env::current_dir().unwrap();
        let path = PathBuf::from("tmp123/inner/../");
        let resolved = resolve_path(&path);
        assert!(resolved.is_ok(), "Failed to resolve relative path with ..");
        let resolved = resolved.unwrap();
        assert_eq!(resolved, current_dir.join("tmp123"));
    }

    #[test]
    fn test_resolve_path_home_directory() {
        let home_dir = env::home_dir();
        if home_dir.is_none() {
            // Skip test if home directory is not set
            return;
        }
        let home_dir = home_dir.unwrap();
        let path = PathBuf::from("~/tmp123");
        let resolved = resolve_path(&path);
        assert!(resolved.is_ok(), "Failed to resolve path with ~");
        let resolved = resolved.unwrap();
        assert_eq!(resolved, home_dir.join("tmp123"));
    }
}
