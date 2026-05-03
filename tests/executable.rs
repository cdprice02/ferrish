mod common;

use assert_fs::TempDir;
use predicates::prelude::*;

use common::ferrish_with_home;

#[test]
fn command_not_found_reports_error() {
    let temp = TempDir::new().unwrap();
    let output = ferrish_with_home(&temp)
        .write_stdin("nonexistentcommandthatdefinitelydoesnotexist123\n")
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = format!("{}{}", stdout, stderr);
    assert!(
        combined.contains("not found"),
        "stdout={:?} stderr={:?}",
        stdout,
        stderr
    );
}

#[test]
fn external_command_stdout_is_captured() {
    #[cfg(unix)]
    let (script, expected) = ("which sh", "sh");
    #[cfg(windows)]
    let (script, expected) = ("where cmd", "cmd");

    let temp = TempDir::new().unwrap();
    ferrish_with_home(&temp)
        .write_stdin(format!("{script}\n"))
        .assert()
        .stderr(predicate::str::is_empty())
        .stdout(predicate::str::contains(expected));
}

#[cfg(unix)]
#[test]
fn external_command_stderr_is_captured() {
    let temp = TempDir::new().unwrap();
    ferrish_with_home(&temp)
        .write_stdin("sh -c 'echo ferrish_stderr_test >&2'\n")
        .assert()
        .stderr(predicate::str::contains("ferrish_stderr_test"));
}

#[cfg(unix)]
#[test]
fn external_command_both_streams_captured() {
    let temp = TempDir::new().unwrap();
    ferrish_with_home(&temp)
        .write_stdin("which sh\nsh -c 'echo ferrish_err_both >&2'\n")
        .assert()
        .stdout(predicate::str::contains("sh"))
        .stderr(predicate::str::contains("ferrish_err_both"));
}
