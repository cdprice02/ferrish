use std::{env, fs, path::PathBuf};

pub(crate) fn home_dir() -> Option<PathBuf> {
    env::home_dir()
}

pub(crate) fn current_dir() -> Option<PathBuf> {
    env::current_dir().ok()
}

pub(crate) fn set_current_dir(path: &PathBuf) -> std::io::Result<()> {
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
