use assert_cmd::Command;
use assert_fs::TempDir;

/// Returns a `Command` for the ferrish binary.
///
/// Propagates LLVM_PROFILE_FILE for cargo-llvm-cov coverage instrumentation.
#[allow(dead_code)]
pub fn ferrish_cmd() -> Command {
    #[allow(unused_mut)]
    let mut cmd = Command::cargo_bin("ferrish").unwrap();
    #[cfg(coverage)]
    if let Ok(template) = std::env::var("LLVM_PROFILE_FILE") {
        cmd.env("LLVM_PROFILE_FILE", template);
    }
    cmd
}

/// Returns a `Command` with HOME set to `temp` and cwd set to `temp`.
#[allow(dead_code)]
pub fn ferrish_with_home(temp: &TempDir) -> Command {
    let mut cmd = ferrish_cmd();
    cmd.current_dir(temp.path()).env("HOME", temp.path());
    // Also set Windows home env vars for cross-platform tests
    cmd.env("USERPROFILE", temp.path())
        .env(
            "HOMEDRIVE",
            temp.path().to_str().unwrap().get(..2).unwrap_or(""),
        )
        .env(
            "HOMEPATH",
            temp.path()
                .to_str()
                .unwrap()
                .get(2..)
                .unwrap_or(temp.path().to_str().unwrap()),
        );
    cmd
}
