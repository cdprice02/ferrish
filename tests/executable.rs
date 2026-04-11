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
fn test_executable_stdout_captured() {
    // Verifies that external command stdout is routed through ShellIo and captured by MockIo.
    #[cfg(unix)]
    let (script, expected) = ("which sh", "sh");
    #[cfg(windows)]
    let (script, expected) = ("where cmd", "cmd");

    let result = ShellTest::new()
        .with_isolated_home()
        .script(script)
        .run();

    assert!(
        result.error().is_empty(),
        "should run without error, got: {}",
        result.error()
    );
    assert!(
        result.output_contains(expected),
        "expected '{}' in stdout, got: {}",
        expected,
        result.output()
    );
}

#[test]
#[cfg(unix)]
fn test_executable_stderr_captured() {
    // Verifies that external command stderr is routed through ShellIo and captured by MockIo.
    let result = ShellTest::new()
        .with_isolated_home()
        .script("sh -c 'echo ferrish_stderr_test >&2'")
        .run();

    result.assert_error_contains("ferrish_stderr_test");
}

#[test]
#[cfg(unix)]
fn test_executable_both_streams_captured() {
    // Verifies that both stdout and stderr from an external command are captured
    // when a single command writes to both streams.  The shell runs two commands
    // back-to-back so that both streams exercise the threaded draining path.
    let result = ShellTest::new()
        .with_isolated_home()
        .script("which sh\nsh -c 'echo ferrish_err_both >&2'")
        .run();

    // stdout: `which sh` should print the path to sh
    assert!(
        result.output_contains("sh"),
        "stdout not captured: {}",
        result.output()
    );
    // stderr: the redirected echo should appear in stderr
    result.assert_error_contains("ferrish_err_both");
}
