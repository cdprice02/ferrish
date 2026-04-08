mod harness;

use harness::ShellTest;

// ============================================================================
// Edge Cases: cd error conditions
// ============================================================================

/// cd to a nonexistent directory produces an error containing "no such file"
#[test]
fn test_cd_nonexistent_dir_error() {
    let result = ShellTest::new()
        .with_isolated_home()
        .script("cd /nonexistent_ferrish_test_path_xyz")
        .run();

    let error = result.error().to_lowercase();
    assert!(
        error.contains("no such file") || error.contains("not found"),
        "Expected 'no such file' or 'not found' in stderr, got: {}",
        result.error()
    );
}

/// cd to a regular file (not a directory) produces an error containing "not a directory"
#[test]
fn test_cd_to_file_produces_not_a_directory_error() {
    let result = ShellTest::new()
        .with_isolated_home()
        .with_file("regularfile.txt", "hello")
        .script("cd regularfile.txt")
        .run();

    let error = result.error().to_lowercase();
    assert!(
        error.contains("not a directory"),
        "Expected 'not a directory' in stderr, got: {}",
        result.error()
    );
}

// ============================================================================
// Edge Cases: exit with non-zero code
// ============================================================================

/// exit with code 42 propagates exactly 42 as the process exit code
#[test]
fn test_exit_nonzero_code_42() {
    let result = ShellTest::new().script("exit 42").run();
    assert_eq!(result.exit_code(), 42, "exit 42 should produce exit code 42");
}

// ============================================================================
// Edge Cases: type for a PATH executable
// ============================================================================

/// type for a builtin command includes "builtin" and the command name in output
#[test]
fn test_type_builtin_echo_shows_builtin() {
    let result = ShellTest::new()
        .with_isolated_home()
        .script("type echo")
        .run();

    let output = result.output();
    assert!(
        output.contains("echo"),
        "type echo should mention 'echo', got: {}",
        output
    );
    assert!(
        output.contains("builtin"),
        "type echo should identify echo as a builtin, got: {}",
        output
    );
}

/// type for a system executable (sh on Unix, cmd on Windows) includes "is" and a path
#[test]
fn test_type_path_executable_shows_path() {
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
        "Expected '{expected_name} is <path>' in output, got: {output}"
    );
    assert!(
        output.contains('/') || output.contains('\\'),
        "Expected a path separator in output, got: {output}"
    );
}

// ============================================================================
// Edge Cases: multiple sequential commands in one session
// ============================================================================

/// Running echo foo then echo bar in one session produces both in output
#[test]
fn test_multiple_sequential_echo_commands() {
    let result = ShellTest::new()
        .script("echo foo\necho bar")
        .run();

    result.assert_output_contains("foo");
    result.assert_output_contains("bar");
}

// ============================================================================
// Edge Cases: empty and whitespace-only input lines
// ============================================================================

/// Whitespace-only lines do not cause errors or crashes
#[test]
fn test_whitespace_only_lines_no_error() {
    let result = ShellTest::new()
        .script("   \n\t\n  \t  \necho alive")
        .run();

    assert!(
        result.error().is_empty(),
        "Whitespace-only lines should not produce errors, got: {}",
        result.error()
    );
    result.assert_output_contains("alive");
}

/// A completely empty script (no commands) runs without error
#[test]
fn test_empty_script_no_error() {
    let result = ShellTest::new().script("").run();

    assert!(
        result.error().is_empty(),
        "Empty input should not produce errors, got: {}",
        result.error()
    );
}
