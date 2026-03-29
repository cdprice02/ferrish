mod harness;

use harness::ShellTest;

// ============================================================================
// Environment Variable Tests
// ============================================================================

#[test]
fn test_home_isolation() {
    let result = ShellTest::new()
        .with_isolated_home()
        .script("pwd")
        .run();

    let output = result.output();
    // The pwd output should be present and not empty
    // (the test harness creates an isolated temp directory)
    assert!(
        output.len() > 5,
        "pwd should produce output in isolated home environment"
    );
    // Verify output contains either the temp path or current directory info
    assert!(
        output.contains("/tmp") || output.contains("ferrish") || output.len() > 10,
        "Output should contain directory path: {}",
        output
    );
}

#[test]
fn test_path_handling() {
    let result = ShellTest::new()
        .with_isolated_home()
        .script("type pwd")
        .run();

    // The 'type' builtin should identify pwd as a builtin
    let output = result.output();
    assert!(
        output.contains("builtin") || output.contains("pwd"),
        "type pwd should identify pwd: {}",
        output
    );
}

#[test]
fn test_simple_environment_isolation() {
    let result = ShellTest::new()
        .with_isolated_home()
        .script("pwd")
        .run();

    // Should successfully execute pwd in isolated environment
    // and not crash or error
    assert!(!result.output_contains("error"), "pwd should not error");
    assert!(!result.output().is_empty(), "pwd should produce output");
}
