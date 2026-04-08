mod harness;

use harness::ShellTest;

// ============================================================================
// Edge Cases
//
// This file covers scenarios not already exercised in tests/builtin.rs or
// tests/repl.rs. Scenarios that duplicate existing coverage (cd-nonexistent,
// cd-to-file, exit 42, type system executable, sequential echo, empty script)
// live in those files.
// ============================================================================

/// type echo specifically identifies echo as a shell builtin (not a PATH binary).
/// This is distinct from test_type_identifies_builtin in builtin.rs, which uses
/// `type exit`; it validates that a command with a common system binary namesake
/// is still correctly identified as a builtin by ferrish.
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

/// Whitespace-only lines (spaces and tabs) do not produce errors and the shell
/// continues processing subsequent commands.
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
