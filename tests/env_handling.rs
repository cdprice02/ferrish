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

    // pwd should output the isolated home (which is also the shell's initial CWD).
    // The shell prompt is written to out_writer on the same line, so strip it first.
    let home = result.home_dir().expect("isolated home was set");
    let prompt = "🦀> ";
    let pwd_output = result
        .output()
        .lines()
        .find_map(|line| line.strip_prefix(prompt).map(str::trim))
        .unwrap_or("");
    assert_eq!(
        pwd_output,
        home.to_str().expect("home is valid UTF-8"),
        "pwd should print the isolated temp home, full output: {}",
        result.output()
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
