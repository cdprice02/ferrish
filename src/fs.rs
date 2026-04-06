use std::{
    io,
    path::{Path, PathBuf},
};

pub(crate) fn resolve_path(path: &PathBuf, home_dir: Option<&Path>, cwd: &Path) -> io::Result<PathBuf> {
    let path = if path.is_absolute() {
        path.clone()
    } else if let Ok(stripped) = path.strip_prefix("~") {
        let home = home_dir.unwrap_or(Path::new("/"));
        let stripped = stripped.strip_prefix("/").unwrap_or(stripped);
        home.join(stripped)
    } else {
        cwd.join(path)
    };

    canonicalize_path(path)
}

pub fn canonicalize_path(path: PathBuf) -> io::Result<PathBuf> {
    soft_canonicalize::soft_canonicalize(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fake_home() -> PathBuf {
        PathBuf::from("/fake/home")
    }

    fn fake_cwd() -> PathBuf {
        PathBuf::from("/fake/cwd")
    }

    #[test]
    fn test_resolve_path_absolute() {
        let path = PathBuf::from("/");
        let resolved = resolve_path(&path, None, &fake_cwd());
        assert!(resolved.is_ok(), "Failed to resolve absolute path");
        let resolved = resolved.unwrap();
        let canonical = canonicalize_path(PathBuf::from("/")).unwrap();
        assert_eq!(resolved, canonical);
    }

    #[test]
    fn test_resolve_path_relative_dots_current() {
        let cwd = fake_cwd();
        let path = PathBuf::from("./tmp123");
        let resolved = resolve_path(&path, None, &cwd);
        assert!(resolved.is_ok(), "Failed to resolve relative path with .");
        assert_eq!(resolved.unwrap(), cwd.join("tmp123"));
    }

    #[test]
    fn test_resolve_path_relative_dots_parent() {
        let cwd = fake_cwd();
        let path = PathBuf::from("tmp123/inner/../");
        let resolved = resolve_path(&path, None, &cwd);
        assert!(resolved.is_ok(), "Failed to resolve relative path with ..");
        assert_eq!(resolved.unwrap(), cwd.join("tmp123"));
    }

    #[test]
    fn test_resolve_path_home_directory() {
        let home = fake_home();
        let path = PathBuf::from("~/tmp123");
        let resolved = resolve_path(&path, Some(&home), &fake_cwd());
        assert!(resolved.is_ok(), "Failed to resolve path with ~");
        assert_eq!(resolved.unwrap(), home.join("tmp123"));
    }

    #[test]
    fn test_resolve_path_tilde_slash() {
        let home = fake_home();
        let path = PathBuf::from("~/");
        let resolved = resolve_path(&path, Some(&home), &fake_cwd());
        assert!(resolved.is_ok());
        assert_eq!(resolved.unwrap(), home);
    }

    #[test]
    fn test_resolve_path_current_dir_relative() {
        let cwd = fake_cwd();
        let path = PathBuf::from("test_file");
        let resolved = resolve_path(&path, None, &cwd);
        assert!(resolved.is_ok(), "Failed to resolve relative path");
        assert_eq!(resolved.unwrap(), cwd.join("test_file"));
    }

    #[test]
    fn test_resolve_path_nested_relative() {
        let cwd = fake_cwd();
        let path = PathBuf::from("./subdir/nested/file");
        let resolved = resolve_path(&path, None, &cwd);
        assert!(resolved.is_ok());
        assert_eq!(resolved.unwrap(), cwd.join("subdir/nested/file"));
    }

    #[test]
    fn test_canonicalize_path_absolute() {
        let path = PathBuf::from("/");
        let result = canonicalize_path(path);
        assert!(result.is_ok());
    }
}
