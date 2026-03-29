mod harness;

use harness::ShellTest;

/// Verify shell shows prompt at startup
#[test]
fn test_repl_prompt_display() {
    let result = ShellTest::new().script("").run();
    let output = result.output();
    // The prompt should appear in the output
    assert!(
        output.contains(">") || output.contains("❯") || output.contains("$"),
        "Expected prompt character in output, got:\n{}",
        output
    );
}

/// Test multiple commands in REPL
#[test]
fn test_repl_sequential_commands() {
    let result = ShellTest::new().script("echo hello\npwd").run();
    let output = result.output();
    // Verify echo output
    assert_output_contains(output, "hello");
    // Verify pwd output contains a path separator
    assert_output_contains(output, "/");
}

/// Test handling of empty lines
#[test]
fn test_repl_empty_input() {
    let result = ShellTest::new().script("\n\necho test").run();
    let output = result.output();
    // Verify that empty lines don't crash the shell
    assert_output_contains(output, "test");
}

/// Test commands with multiple words
#[test]
fn test_repl_multiword_input() {
    let result = ShellTest::new().script("echo hello world").run();
    let output = result.output();
    assert_output_contains(output, "hello world");
}

// Helper function to check output contains text
fn assert_output_contains(output: &str, text: &str) {
    assert!(
        output.contains(text),
        "Expected '{}' in output:\n{}",
        text,
        output
    );
}
