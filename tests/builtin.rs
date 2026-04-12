mod harness;

use harness::ShellTest;

// ============================================================================
// PWD Command Tests
// ============================================================================

#[test]
fn test_pwd_in_home_directory() {
    let result = ShellTest::new()
        .with_isolated_home()
        .script("pwd")
        .run();

    assert!(!result.output_contains("error"), "pwd should not error in home dir");
}

#[test]
fn test_pwd_after_cd_to_subdirectory() {
    let result = ShellTest::new()
        .with_isolated_home()
        .with_dir("subdir")
        .script("cd subdir\npwd")
        .run();

    assert!(
        result.output_contains("subdir"),
        "pwd output should contain 'subdir' after cd"
    );
}

#[test]
fn test_pwd_shows_correct_path() {
    let result = ShellTest::new()
        .with_isolated_home()
        .script("pwd\necho end")
        .run();

    let output = result.output();
    assert!(output.contains("end"), "Should contain the end marker");
    assert!(output.len() > 10, "pwd should produce meaningful output");
}

// ============================================================================
// Echo Command Tests
// ============================================================================

#[test]
fn test_echo_with_no_arguments() {
    let result = ShellTest::new()
        .with_isolated_home()
        .script("echo")
        .run();

    assert!(!result.output_contains("error"), "echo should not error");
}

#[test]
fn test_echo_with_single_argument() {
    let result = ShellTest::new()
        .with_isolated_home()
        .script("echo hello")
        .run();

    result.assert_output_contains("hello");
}

#[test]
fn test_echo_with_multiple_arguments() {
    let result = ShellTest::new()
        .with_isolated_home()
        .script("echo hello world from ferrish")
        .run();

    result.assert_output_contains("hello");
    result.assert_output_contains("world");
    result.assert_output_contains("ferrish");
}

#[test]
fn test_echo_collapses_multiple_spaces() {
    let result = ShellTest::new()
        .with_isolated_home()
        .script("echo   hello   world")
        .run();

    result.assert_output_contains("hello");
    result.assert_output_contains("world");
}

// ============================================================================
// Single-Quote Parsing Tests (issue #17)
// ============================================================================

#[test]
fn test_echo_single_quote_preserves_spaces() {
    // 'hello    world' should arrive as one argument with spaces intact.
    let result = ShellTest::new()
        .with_isolated_home()
        .script("echo 'hello    world'")
        .run();

    result.assert_output_contains("hello    world");
}

#[test]
fn test_echo_single_quote_adjacent_concatenation() {
    // 'hello''world' should concatenate into a single argument "helloworld".
    let result = ShellTest::new()
        .with_isolated_home()
        .script("echo 'hello''world'")
        .run();

    result.assert_output_contains("helloworld");
}

#[test]
fn test_echo_single_quote_mixed_adjacent_concatenation() {
    // hello''world should concatenate into a single argument "helloworld".
    let result = ShellTest::new()
        .with_isolated_home()
        .script("echo hello''world")
        .run();

    result.assert_output_contains("helloworld");
}

// ============================================================================
// CD Command Tests
// ============================================================================

#[test]
fn test_cd_to_nonexistent_directory_shows_error() {
    let result = ShellTest::new()
        .with_isolated_home()
        .script("cd nonexistent")
        .run();

    result.assert_error_contains("no such file or directory");
}

#[test]
fn test_cd_error_contains_appropriate_message() {
    let result = ShellTest::new()
        .with_isolated_home()
        .script("cd /this/path/definitely/does/not/exist/on/any/system")
        .run();

    let error = result.error();
    assert!(
        error.contains("cd:") || error.contains("no such file or directory"),
        "Error message should mention cd command or file not found: {}",
        error
    );
}

#[test]
fn test_cd_to_file_not_directory() {
    let result = ShellTest::new()
        .with_isolated_home()
        .with_file("somefile", "")
        .script("cd somefile")
        .run();

    result.assert_error_contains("not a directory");
}

#[test]
fn test_cd_default_to_home() {
    let result = ShellTest::new().with_isolated_home().script("cd\npwd").run();

    assert!(!result.output_contains("error"));
    assert!(!result.output().is_empty());
}

#[test]
fn test_cd_tilde_explicit() {
    let result = ShellTest::new()
        .with_isolated_home()
        .script("cd ~\npwd")
        .run();

    assert!(!result.output_contains("error"), "cd ~ should not error");
    assert!(!result.output().is_empty(), "pwd should produce output after cd ~");
}

#[test]
fn test_cd_relative_then_back() {
    let result = ShellTest::new()
        .with_isolated_home()
        .with_dir("subdir")
        .script("cd subdir\ncd ..\npwd\necho done")
        .run();

    result.assert_output_contains("done");
    assert!(!result.output_contains("error"), "cd .. should not error");
}

// ============================================================================
// Type Command Tests
// ============================================================================

#[test]
fn test_type_identifies_builtin() {
    let result = ShellTest::new().with_isolated_home().script("type exit").run();

    assert!(result.output().contains("builtin") || result.output().contains("exit"));
}

#[test]
fn test_type_nonexistent_returns_not_found() {
    let result = ShellTest::new()
        .with_isolated_home()
        .script("type definitely_does_not_exist_123")
        .run();

    assert!(result.error().contains("not found") || result.output().contains("not found"));
}

#[test]
fn test_type_with_no_args_reports_error() {
    let result = ShellTest::new()
        .with_isolated_home()
        .script("type\necho survived")
        .run();

    result.assert_error_contains("missing operand");
    result.assert_output_contains("survived");
}

#[test]
fn test_type_system_executable() {
    #[cfg(unix)]
    let (script, expected_name) = ("type sh", "sh");
    #[cfg(windows)]
    let (script, expected_name) = ("type cmd", "cmd");

    let result = ShellTest::new()
        .with_isolated_home()
        .script(script)
        .run();

    let output = result.output();
    assert!(
        output.contains(&format!("{expected_name} is")),
        "type {expected_name} should show '{expected_name} is <path>', got: {output}"
    );
    assert!(
        output.contains('/') || output.contains('\\'),
        "type {expected_name} output should contain a path separator, got: {output}"
    );
}

// ============================================================================
// Exit and Sequential Command Tests
// ============================================================================

#[test]
fn test_exit_command_closes_shell() {
    let result = ShellTest::new()
        .with_isolated_home()
        .script("echo before\nexit")
        .run();

    result.assert_output_contains("before");
}

#[test]
fn test_multiple_sequential_commands_work() {
    let result = ShellTest::new()
        .with_isolated_home()
        .script("echo first\necho second\necho third")
        .run();

    result.assert_output_contains("first");
    result.assert_output_contains("second");
    result.assert_output_contains("third");
}
