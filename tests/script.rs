mod common;

use assert_fs::prelude::*;
use assert_fs::TempDir;
use predicates::prelude::*;

use common::ferrish_cmd;

// --- ferrish -c <command> ---

#[test]
fn command_flag_echo() {
    ferrish_cmd()
        .arg("-c")
        .arg("echo hello")
        .assert()
        .stdout(predicate::str::contains("hello"))
        .success();
}

#[test]
fn command_flag_exit_code_propagates() {
    ferrish_cmd().arg("-c").arg("exit 7").assert().code(7);
}

// --- ferrish <script> ---

#[test]
fn script_file_single_command() {
    let temp = TempDir::new().unwrap();
    temp.child("greet.sh").write_str("echo hello\n").unwrap();
    ferrish_cmd()
        .arg(temp.child("greet.sh").path())
        .assert()
        .stdout(predicate::str::contains("hello"))
        .success();
}

#[test]
fn script_file_multiple_commands() {
    let temp = TempDir::new().unwrap();
    temp.child("multi.sh")
        .write_str("echo first\necho second\n")
        .unwrap();
    ferrish_cmd()
        .arg(temp.child("multi.sh").path())
        .assert()
        .stdout(predicate::str::contains("first"))
        .stdout(predicate::str::contains("second"))
        .success();
}

#[test]
fn script_file_exit_code_propagates() {
    let temp = TempDir::new().unwrap();
    temp.child("fail.sh").write_str("exit 42\n").unwrap();
    ferrish_cmd()
        .arg(temp.child("fail.sh").path())
        .assert()
        .code(42);
}

#[test]
fn script_file_no_prompt_in_output() {
    let temp = TempDir::new().unwrap();
    temp.child("greet.sh").write_str("echo hello\n").unwrap();
    ferrish_cmd()
        .arg(temp.child("greet.sh").path())
        .assert()
        .stdout(predicate::str::contains("$ ").not())
        .success();
}

#[test]
fn script_file_continuation_lines() {
    let temp = TempDir::new().unwrap();
    temp.child("cont.sh")
        .write_str("echo hello \\\nworld\n")
        .unwrap();
    ferrish_cmd()
        .arg(temp.child("cont.sh").path())
        .assert()
        .stdout(predicate::str::contains("hello world"))
        .success();
}

#[test]
fn script_file_not_found_exits_nonzero() {
    let temp = TempDir::new().unwrap();
    ferrish_cmd()
        .arg(temp.path().join("nonexistent_script_xyz.sh"))
        .assert()
        .failure();
}
