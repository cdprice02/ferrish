#[allow(dead_code)]
mod common;
use common::ShellHarness;

// --- ferrish -c <command> ---

#[test]
fn command_flag_echo() {
    ShellHarness::new()
        .run_command("echo hello")
        .assert_stdout_contains("hello")
        .assert_exit_success();
}

#[test]
fn command_flag_exit_code_propagates() {
    ShellHarness::new()
        .run_command("exit 7")
        .assert_exit_code(7);
}

// --- ferrish <script> ---

#[test]
fn script_file_single_command() {
    ShellHarness::new()
        .with_file("greet.sh", "echo hello\n")
        .run_file("greet.sh")
        .assert_stdout_contains("hello")
        .assert_exit_success();
}

#[test]
fn script_file_multiple_commands() {
    ShellHarness::new()
        .with_file("multi.sh", "echo first\necho second\n")
        .run_file("multi.sh")
        .assert_stdout_contains("first")
        .assert_stdout_contains("second")
        .assert_exit_success();
}

#[test]
fn script_file_exit_code_propagates() {
    ShellHarness::new()
        .with_file("fail.sh", "exit 42\n")
        .run_file("fail.sh")
        .assert_exit_code(42);
}

#[test]
fn script_file_no_prompt_in_output() {
    ShellHarness::new()
        .with_file("greet.sh", "echo hello\n")
        .run_file("greet.sh")
        .assert_stdout_not_contains("$ ")
        .assert_exit_success();
}

#[test]
fn script_file_continuation_lines() {
    ShellHarness::new()
        .with_file("cont.sh", "echo hello \\\nworld\n")
        .run_file("cont.sh")
        .assert_stdout_contains("hello world")
        .assert_exit_success();
}

#[test]
fn script_file_not_found_exits_nonzero() {
    ShellHarness::new()
        .run_file("nonexistent_script_xyz.sh")
        .assert_exit_nonzero();
}
