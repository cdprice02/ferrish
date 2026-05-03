mod common;

use assert_fs::TempDir;
use predicates::prelude::*;

use common::ferrish_with_home;

#[test]
fn exit_no_args_exits_zero() {
    let temp = TempDir::new().unwrap();
    ferrish_with_home(&temp)
        .write_stdin("exit\n")
        .assert()
        .code(0);
}

#[test]
fn exit_valid_code_propagates() {
    let temp = TempDir::new().unwrap();
    ferrish_with_home(&temp)
        .write_stdin("exit 42\n")
        .assert()
        .code(42);
}

#[test]
fn exit_out_of_range_reports_error_and_exits_one() {
    let temp = TempDir::new().unwrap();
    ferrish_with_home(&temp)
        .write_stdin("exit 256\n")
        .assert()
        .stderr(predicate::str::contains("numeric argument required"))
        .code(1);
}

#[test]
fn exit_non_numeric_reports_error_and_exits_one() {
    let temp = TempDir::new().unwrap();
    ferrish_with_home(&temp)
        .write_stdin("exit abc\n")
        .assert()
        .stderr(predicate::str::contains("numeric argument required"))
        .code(1);
}

#[test]
fn exit_stops_subsequent_commands() {
    let temp = TempDir::new().unwrap();
    ferrish_with_home(&temp)
        .write_stdin("echo a\nexit\necho b\n")
        .assert()
        .stdout(predicate::str::contains("a"))
        .stdout(predicate::str::contains("b").not());
}
