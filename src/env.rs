use std::{env, fs, io, path::PathBuf};

pub fn home_dir() -> Option<PathBuf> {
    #[cfg(unix)]
    {
        env::var_os("HOME").map(PathBuf::from)
    }
    #[cfg(windows)]
    {
        env::var_os("USERPROFILE")
            .or_else(|| {
                let drive = env::var_os("HOMEDRIVE")?;
                let path_suffix = env::var_os("HOMEPATH")?;
                let mut buf = PathBuf::from(drive);
                buf.push(path_suffix);
                Some(buf.into_os_string())
            })
            .map(PathBuf::from)
    }
    #[cfg(not(any(unix, windows)))]
    {
        None
    }
}

pub fn current_dir() -> Result<PathBuf, io::Error> {
    env::current_dir()
}

pub fn set_current_dir(path: &PathBuf) -> io::Result<()> {
    env::set_current_dir(path)
}

pub(crate) fn get_path_files() -> impl Iterator<Item = PathBuf> {
    get_path_dirs().flat_map(|d| {
        fs::read_dir(d)
            .ok()
            .into_iter()
            .flatten()
            .filter_map(|entry| entry.ok().map(|e| e.path()))
            .collect::<Vec<_>>()
    })
}

fn get_path_dirs() -> impl Iterator<Item = PathBuf> {
    let path = env::var_os("PATH").unwrap_or_default();
    env::split_paths(&path)
        .collect::<Vec<_>>()
        .into_iter()
        .filter(|d| d.is_dir() && d.exists())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_current_dir_returns_valid_path() {
        let result = current_dir();
        assert!(result.is_ok());
        let cwd = result.unwrap();
        assert!(cwd.is_absolute());
    }

    #[test]
    fn test_get_path_dirs_filters_existing_directories() {
        let dirs: Vec<_> = get_path_dirs().collect();
        // Verify all returned paths exist and are directories
        for dir in dirs {
            assert!(dir.exists(), "path {} should exist", dir.display());
            assert!(dir.is_dir(), "path {} should be a directory", dir.display());
        }
    }

    #[test]
    fn test_home_dir_returns_valid_path_or_none() {
        if let Some(home) = home_dir() {
            assert!(home.is_absolute());
        }
    }
}
