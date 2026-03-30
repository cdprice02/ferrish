mod harness;

use harness::ShellTest;

// ============================================================================
// Executable Detection and External Command Tests
// ============================================================================

#[test]
fn test_executable_detection_with_system_command() {
    let result = ShellTest::new()
        .with_isolated_home()
        .script("echo executable_test")
        .run();

    // System echo should work (running as external command)
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

    let output = result.output();
    let error = result.error();
    assert!(
        !output.is_empty() || !error.contains("not found"),
        "Should be able to execute a platform-appropriate executable lookup command"
    );
}
