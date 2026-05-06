mod common;

use assert_fs::TempDir;
use assert_fs::prelude::*;
use predicates::prelude::*;

use common::ferrish_cmd;

// ============================================================================
// PWD Tests
// ============================================================================

#[test]
fn pwd_in_home_directory() {
    ferrish_cmd()
        .write_stdin("pwd\n")
        .assert()
        .success()
        .stderr(predicate::str::is_empty());
}

#[test]
fn pwd_after_cd_to_subdirectory() {
    let temp = TempDir::new().unwrap();
    temp.child("subdir").create_dir_all().unwrap();
    ferrish_cmd()
        .current_dir(temp.path())
        .write_stdin("cd subdir\npwd\n")
        .assert()
        .success()
        .stdout(predicate::str::contains("subdir"));
}

// ============================================================================
// Echo Tests
// ============================================================================

#[test]
fn echo_with_no_arguments() {
    ferrish_cmd()
        .write_stdin("echo\n")
        .assert()
        .success()
        .stderr(predicate::str::is_empty());
}

#[test]
fn echo_with_single_argument() {
    ferrish_cmd()
        .write_stdin("echo hello\n")
        .assert()
        .success()
        .stdout(predicate::str::contains("hello"));
}

#[test]
fn echo_with_multiple_arguments() {
    ferrish_cmd()
        .write_stdin("echo hello world from ferrish\n")
        .assert()
        .success()
        .stdout(predicate::str::contains("hello"))
        .stdout(predicate::str::contains("world"))
        .stdout(predicate::str::contains("ferrish"));
}

#[test]
fn echo_collapses_multiple_spaces() {
    ferrish_cmd()
        .write_stdin("echo   hello   world\n")
        .assert()
        .success()
        .stdout(predicate::str::contains("hello"))
        .stdout(predicate::str::contains("world"));
}

// ============================================================================
// Single-Quote Parsing Tests
// ============================================================================

#[test]
fn echo_single_quote_preserves_spaces() {
    ferrish_cmd()
        .write_stdin("echo 'hello    world'\n")
        .assert()
        .success()
        .stdout(predicate::str::contains("hello    world"));
}

#[test]
fn echo_single_quote_adjacent_concatenation() {
    ferrish_cmd()
        .write_stdin("echo 'hello''world'\n")
        .assert()
        .success()
        .stdout(predicate::str::contains("helloworld"));
}

#[test]
fn echo_single_quote_mixed_adjacent_concatenation() {
    ferrish_cmd()
        .write_stdin("echo hello''world\n")
        .assert()
        .success()
        .stdout(predicate::str::contains("helloworld"));
}

// ============================================================================
// CD Tests
// ============================================================================

#[test]
fn cd_to_nonexistent_directory_shows_error() {
    ferrish_cmd()
        .write_stdin("cd nonexistent\n")
        .assert()
        .stderr(predicate::str::contains("no such file or directory"));
}

#[test]
fn cd_to_file_shows_not_a_directory_error() {
    let temp = TempDir::new().unwrap();
    temp.child("somefile").write_str("").unwrap();
    ferrish_cmd()
        .current_dir(temp.path())
        .write_stdin("cd somefile\n")
        .assert()
        .stderr(predicate::str::contains("not a directory"));
}

#[test]
fn cd_no_args_goes_to_home() {
    ferrish_cmd()
        .write_stdin("cd\necho ok\n")
        .assert()
        .success()
        .stdout(predicate::str::contains("ok"))
        .stderr(predicate::str::is_empty());
}

#[test]
fn cd_tilde_goes_to_home() {
    ferrish_cmd()
        .write_stdin("cd ~\necho ok\n")
        .assert()
        .success()
        .stdout(predicate::str::contains("ok"))
        .stderr(predicate::str::is_empty());
}

#[test]
fn cd_relative_then_back() {
    let temp = TempDir::new().unwrap();
    temp.child("subdir").create_dir_all().unwrap();
    ferrish_cmd()
        .current_dir(temp.path())
        .write_stdin("cd subdir\ncd ..\npwd\necho done\n")
        .assert()
        .success()
        .stdout(predicate::str::contains("done"))
        .stderr(predicate::str::is_empty());
}

#[test]
fn cd_no_args_without_home_shows_error() {
    ferrish_cmd()
        .env_remove("HOME")
        .env_remove("USERPROFILE")
        .env_remove("HOMEDRIVE")
        .env_remove("HOMEPATH")
        .write_stdin("cd\necho survived\n")
        .assert()
        .stderr(predicate::str::contains("home directory not set"))
        .stdout(predicate::str::contains("survived"));
}

#[test]
fn cd_tilde_without_home_shows_error() {
    ferrish_cmd()
        .env_remove("HOME")
        .env_remove("USERPROFILE")
        .env_remove("HOMEDRIVE")
        .env_remove("HOMEPATH")
        .write_stdin("cd ~\necho survived\n")
        .assert()
        .stderr(predicate::str::contains("home directory not set"))
        .stdout(predicate::str::contains("survived"));
}

// ============================================================================
// Type Tests
// ============================================================================

#[test]
fn type_identifies_exit_as_builtin() {
    ferrish_cmd()
        .write_stdin("type exit\n")
        .assert()
        .success()
        .stdout(predicate::str::contains("builtin"));
}

#[test]
fn type_identifies_echo_as_builtin() {
    ferrish_cmd()
        .write_stdin("type echo\n")
        .assert()
        .success()
        .stdout(predicate::str::contains("echo"))
        .stdout(predicate::str::contains("builtin"));
}

#[test]
fn type_nonexistent_command_reports_not_found_on_stderr() {
    ferrish_cmd()
        .write_stdin("type definitely_does_not_exist_123\n")
        .assert()
        .stderr(predicate::str::contains("not found"))
        .stdout(predicate::str::contains("not found").not());
}

#[test]
fn type_with_no_args_reports_missing_operand() {
    ferrish_cmd()
        .write_stdin("type\necho survived\n")
        .assert()
        .stderr(predicate::str::contains("missing operand"))
        .stdout(predicate::str::contains("survived"));
}

#[cfg(unix)]
#[test]
fn type_system_executable_shows_path() {
    let output = ferrish_cmd().write_stdin("type sh\n").output().unwrap();
    assert!(
        output.status.success(),
        "expected success, got: {:?}",
        output.status
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("sh is"), "got: {stdout}");
    assert!(stdout.contains('/'), "expected a path, got: {stdout}");
}

// ============================================================================
// Exit and Sequential Commands
// ============================================================================

#[test]
fn exit_command_closes_shell() {
    ferrish_cmd()
        .write_stdin("echo before\nexit\n")
        .assert()
        .success()
        .stdout(predicate::str::contains("before"));
}

#[test]
fn multiple_sequential_commands_work() {
    ferrish_cmd()
        .write_stdin("echo first\necho second\necho third\n")
        .assert()
        .success()
        .stdout(predicate::str::contains("first"))
        .stdout(predicate::str::contains("second"))
        .stdout(predicate::str::contains("third"));
}
