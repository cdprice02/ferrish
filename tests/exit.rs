#[allow(dead_code)]
mod common;

use common::ShellHarness;

#[test]
fn exit_no_args_exits_zero() {
    ShellHarness::new().run("exit").assert_exit_code(0);
}

#[test]
fn exit_valid_code_propagates() {
    ShellHarness::new().run("exit 42").assert_exit_code(42);
}

#[test]
fn exit_out_of_range_reports_error_and_exits_one() {
    ShellHarness::new()
        .run("exit 256")
        .assert_stderr_contains("numeric argument required")
        .assert_exit_code(1);
}

#[test]
fn exit_non_numeric_reports_error_and_exits_one() {
    ShellHarness::new()
        .run("exit abc")
        .assert_stderr_contains("numeric argument required")
        .assert_exit_code(1);
}

#[test]
fn exit_stops_subsequent_commands() {
    ShellHarness::new()
        .run("echo a\nexit\necho b")
        .assert_stdout_contains("a")
        .assert_stdout_not_contains("b");
}
