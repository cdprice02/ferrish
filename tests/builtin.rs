mod harness;

use harness::ShellHarness;

// ============================================================================
// PWD Tests
// ============================================================================

#[test]
fn pwd_in_home_directory() {
    ShellHarness::new().run("pwd").assert_stderr_empty();
}

#[test]
fn pwd_after_cd_to_subdirectory() {
    ShellHarness::new()
        .with_dir("subdir")
        .run("cd subdir\npwd")
        .assert_stdout_contains("subdir");
}

// ============================================================================
// Echo Tests
// ============================================================================

#[test]
fn echo_with_no_arguments() {
    ShellHarness::new().run("echo").assert_stderr_empty();
}

#[test]
fn echo_with_single_argument() {
    ShellHarness::new().run("echo hello").assert_stdout_contains("hello");
}

#[test]
fn echo_with_multiple_arguments() {
    ShellHarness::new()
        .run("echo hello world from ferrish")
        .assert_stdout_contains("hello")
        .assert_stdout_contains("world")
        .assert_stdout_contains("ferrish");
}

#[test]
fn echo_collapses_multiple_spaces() {
    ShellHarness::new()
        .run("echo   hello   world")
        .assert_stdout_contains("hello")
        .assert_stdout_contains("world");
}

// ============================================================================
// Single-Quote Parsing Tests
// ============================================================================

#[test]
fn echo_single_quote_preserves_spaces() {
    ShellHarness::new()
        .run("echo 'hello    world'")
        .assert_stdout_contains("hello    world");
}

#[test]
fn echo_single_quote_adjacent_concatenation() {
    ShellHarness::new()
        .run("echo 'hello''world'")
        .assert_stdout_contains("helloworld");
}

#[test]
fn echo_single_quote_mixed_adjacent_concatenation() {
    ShellHarness::new()
        .run("echo hello''world")
        .assert_stdout_contains("helloworld");
}

// ============================================================================
// CD Tests
// ============================================================================

#[test]
fn cd_to_nonexistent_directory_shows_error() {
    ShellHarness::new()
        .run("cd nonexistent")
        .assert_stderr_contains("no such file or directory");
}

#[test]
fn cd_to_file_shows_not_a_directory_error() {
    ShellHarness::new()
        .with_file("somefile", "")
        .run("cd somefile")
        .assert_stderr_contains("not a directory");
}

#[test]
fn cd_no_args_goes_to_home() {
    ShellHarness::new().run("cd\npwd").assert_stderr_empty();
}

#[test]
fn cd_tilde_goes_to_home() {
    let out = ShellHarness::new().run("cd ~\npwd");
    let home = out.home_dir().to_str().unwrap().to_string();
    out.assert_stdout_contains(&home).assert_stderr_empty();
}

#[test]
fn cd_relative_then_back() {
    ShellHarness::new()
        .with_dir("subdir")
        .run("cd subdir\ncd ..\npwd\necho done")
        .assert_stdout_contains("done")
        .assert_stderr_empty();
}

// ============================================================================
// Type Tests
// ============================================================================

#[test]
fn type_identifies_exit_as_builtin() {
    ShellHarness::new()
        .run("type exit")
        .assert_stdout_contains("builtin");
}

#[test]
fn type_identifies_echo_as_builtin() {
    ShellHarness::new()
        .run("type echo")
        .assert_stdout_contains("echo")
        .assert_stdout_contains("builtin");
}

#[test]
fn type_nonexistent_command_reports_not_found_on_stderr() {
    ShellHarness::new()
        .run("type definitely_does_not_exist_123")
        .assert_stderr_contains("not found")
        .assert_stdout_not_contains("not found");
}

#[test]
fn type_with_no_args_reports_missing_operand() {
    ShellHarness::new()
        .run("type\necho survived")
        .assert_stderr_contains("missing operand")
        .assert_stdout_contains("survived");
}

#[cfg(unix)]
#[test]
fn type_system_executable_shows_path() {
    let out = ShellHarness::new().run("type sh");
    let stdout = out.stdout();
    assert!(stdout.contains("sh is"), "got: {stdout}");
    assert!(stdout.contains('/'), "expected a path, got: {stdout}");
}

// ============================================================================
// Exit and Sequential Commands
// ============================================================================

#[test]
fn exit_command_closes_shell() {
    ShellHarness::new()
        .run("echo before\nexit")
        .assert_stdout_contains("before");
}

#[test]
fn multiple_sequential_commands_work() {
    ShellHarness::new()
        .run("echo first\necho second\necho third")
        .assert_stdout_contains("first")
        .assert_stdout_contains("second")
        .assert_stdout_contains("third");
}
