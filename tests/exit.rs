mod common;

use predicates::prelude::*;

use common::ferrish_cmd;

#[test]
fn exit_no_args_exits_zero() {
    ferrish_cmd().write_stdin("exit\n").assert().code(0);
}

#[test]
fn exit_valid_code_propagates() {
    ferrish_cmd().write_stdin("exit 42\n").assert().code(42);
}

#[test]
fn exit_out_of_range_reports_error_and_exits_one() {
    ferrish_cmd()
        .write_stdin("exit 256\n")
        .assert()
        .stderr(predicate::str::contains("numeric argument required"))
        .code(1);
}

#[test]
fn exit_non_numeric_reports_error_and_exits_one() {
    ferrish_cmd()
        .write_stdin("exit abc\n")
        .assert()
        .stderr(predicate::str::contains("numeric argument required"))
        .code(1);
}

#[test]
fn exit_stops_subsequent_commands() {
    ferrish_cmd()
        .write_stdin("echo a\nexit\necho b\n")
        .assert()
        .stdout(predicate::str::contains("a"))
        .stdout(predicate::str::contains("b").not());
}
