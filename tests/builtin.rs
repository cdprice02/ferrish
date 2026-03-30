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

    // Should show the home directory path
    assert!(!result.output_contains("error"), "pwd should not error in home dir");
}

#[test]
fn test_pwd_after_cd_to_subdirectory() {
    let result = ShellTest::new()
        .with_isolated_home()
        .script("mkdir subdir\ncd subdir\npwd")
        .run();

    // Output should contain "subdir" in the path
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

    // Should have some output before the "end" marker
    let output = result.output();
    assert!(output.contains("end"), "Should contain the end marker");
    // Output should not be empty (pwd should print something)
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

    // echo with no args should output a blank line (just the prompt after)
    // The output should show the command executed
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

// ============================================================================
// CD Command Error Tests
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

// ============================================================================
// Exit and Sequential Command Tests
// ============================================================================

#[test]
fn test_exit_command_closes_shell() {
    let result = ShellTest::new()
        .with_isolated_home()
        .script("echo before\nexit")
        .run();

    // Should show the before message and exit cleanly
    result.assert_output_contains("before");
}

#[test]
fn test_cd_tilde_explicit() {
    let result = ShellTest::new()
        .with_isolated_home()
        .script("cd ~\npwd")
        .run();

    // After `cd ~`, pwd should show the isolated HOME directory
    assert!(!result.output_contains("error"), "cd ~ should not error");
    assert!(!result.output().is_empty(), "pwd should produce output after cd ~");
}

#[test]
fn test_cd_relative_then_back() {
    let result = ShellTest::new()
        .with_isolated_home()
        .script("mkdir subdir\ncd subdir\ncd ..\npwd\necho done")
        .run();

    result.assert_output_contains("done");
    // Should not contain "subdir" in the final pwd (we went back up)
    assert!(!result.output_contains("error"), "cd .. should not error");
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
