mod harness;

use harness::ShellTest;

// ============================================================================
// Prompt and Basic Parsing Tests
// ============================================================================

/// Verify shell shows prompt at startup
#[test]
fn test_repl_prompt_display() {
    let result = ShellTest::new().script("").run();
    let output = result.output();
    assert!(
        output.contains(">") || output.contains("❯") || output.contains("$"),
        "Expected prompt character in output, got:\n{}",
        output
    );
}

/// Test commands with multiple words
#[test]
fn test_repl_multiword_input() {
    let result = ShellTest::new().script("echo hello world").run();
    result.assert_output_contains("hello world");
}

/// Test handling of empty lines
#[test]
fn test_repl_empty_input() {
    let result = ShellTest::new().script("\n\necho test").run();
    result.assert_output_contains("test");
}

/// Test multiple commands in sequence
#[test]
fn test_repl_sequential_commands() {
    let result = ShellTest::new().script("echo hello\npwd").run();
    result.assert_output_contains("hello");
}

// ============================================================================
// Error Handling and Recovery Tests
// ============================================================================

/// Non-zero exit from an external command reports an error
#[test]
fn test_nonzero_exit_reports_error() {
    let result = ShellTest::new().with_isolated_home().script("false").run();

    assert!(
        result.error().contains("exited with status")
            || result.error().to_lowercase().contains("non-zero")
            || result.error().contains("exited"),
        "Expected exit status error, got: {}",
        result.error()
    );
}

/// Shell continues running after a non-fatal error
#[test]
fn test_nonfatal_error_then_continue() {
    let result = ShellTest::new()
        .with_isolated_home()
        .script("false\necho survived")
        .run();

    assert!(
        result.error().contains("exited with status") || !result.error().is_empty(),
        "Expected error output"
    );
    result.assert_output_contains("survived");
}

/// Unknown command error is written to stderr, not stdout
#[test]
fn test_error_goes_to_stderr_not_stdout() {
    let result = ShellTest::new()
        .with_isolated_home()
        .script("commandthatdoesnotexist_xyz")
        .run();

    assert!(
        !result.error().is_empty(),
        "Error output should be non-empty for unknown command"
    );
    assert!(
        !result.output().contains("not found"),
        "Error text should not appear on stdout, got: {}",
        result.output()
    );
}
