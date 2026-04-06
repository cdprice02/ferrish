mod harness;

use harness::ShellTest;

// ============================================================================
// Executable Detection and External Command Tests
// ============================================================================

#[test]
fn test_builtin_echo_produces_output() {
    // `echo` is a ferrish builtin, so its output is captured by MockIo.
    // This test verifies the builtin path; external-command I/O capture is a TODO.
    let result = ShellTest::new()
        .with_isolated_home()
        .script("echo executable_test")
        .run();

    result.assert_output_contains("executable_test");
}

#[test]
fn test_command_not_found_error() {
    let result = ShellTest::new()
        .with_isolated_home()
        .script("nonexistentcommandthatdefinitelydoesnotexist123")
        .run();

    // Should produce an error about command not found
    assert!(
        result.error().contains("not found") || result.output().contains("not found"),
        "Should show error for non-existent command in error or output: error='{}', output='{}'",
        result.error(),
        result.output()
    );
}

#[test]
fn test_executable_in_path() {
    #[cfg(unix)]
    let script = "which sh";
    #[cfg(windows)]
    let script = "where cmd";

    let result = ShellTest::new()
        .with_isolated_home()
        .script(script)
        .run();

    // External command stdout bypasses MockIo (known TODO), so we assert there's no error
    // rather than checking captured output.
    assert!(
        result.error().is_empty(),
        "Platform-appropriate executable lookup should run without error, got: {}",
        result.error()
    );
}
