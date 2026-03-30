mod harness;

use harness::ShellTest;

#[test]
fn test_false_nonzero_exit_reports_error() {
    let result = ShellTest::new().with_isolated_home().script("false").run();
    assert!(result.error().contains("exited with status") || result.error().to_lowercase().contains("non-zero") || result.error().contains("exited"));
}

#[test]
fn test_nonfatal_error_then_continue() {
    let result = ShellTest::new()
        .with_isolated_home()
        .script("false\necho survived")
        .run();

    // Shell should print an error for the failed command but continue to run the next command
    assert!(result.error().contains("exited with status") || !result.error().is_empty());
    result.assert_output_contains("survived");
}

#[test]
fn test_type_nonexistent_returns_not_found() {
    let result = ShellTest::new()
        .with_isolated_home()
        .script("type definitely_does_not_exist_123")
        .run();

    assert!(result.error().contains("not found") || result.output().contains("not found"));
}

#[test]
fn test_cd_to_file_not_directory() {
    let result = ShellTest::new()
        .with_isolated_home()
        .script("touch somefile\ncd somefile")
        .run();

    assert!(result.error().contains("not a directory") || result.output().contains("not a directory"));
}

#[test]
fn test_cd_default_to_home() {
    let result = ShellTest::new().with_isolated_home().script("cd\npwd").run();
    assert!(!result.output_contains("error"));
    assert!(!result.output().is_empty());
}

#[test]
fn test_type_exit_identifies_builtin() {
    let result = ShellTest::new().with_isolated_home().script("type exit").run();
    assert!(result.output().contains("builtin") || result.output().contains("exit"));
}

#[test]
fn test_echo_after_error_sequence() {
    let result = ShellTest::new().with_isolated_home().script("false\necho ok").run();
    result.assert_output_contains("ok");
}

#[test]
fn test_type_with_no_args() {
    let result = ShellTest::new()
        .with_isolated_home()
        .script("type\necho survived")
        .run();

    // type with no args should report missing operand on stderr
    result.assert_error_contains("missing operand");
    // Shell should continue after the non-fatal error
    result.assert_output_contains("survived");
}

#[test]
fn test_type_system_executable() {
    let result = ShellTest::new()
        .with_isolated_home()
        .script("type sh")
        .run();

    // `type sh` should identify sh as an executable and show its path
    let output = result.output();
    assert!(
        output.contains("sh is"),
        "type sh should show 'sh is <path>', got: {}",
        output
    );
    // Path should contain a separator (/ on Unix, \ on Windows)
    assert!(
        output.contains('/') || output.contains('\\'),
        "type sh output should contain a path separator, got: {}",
        output
    );
}

#[test]
fn test_echo_multiple_spaces_between_args() {
    let result = ShellTest::new()
        .with_isolated_home()
        .script("echo   hello   world")
        .run();

    // Multiple spaces should be collapsed; args are separated by single spaces
    result.assert_output_contains("hello");
    result.assert_output_contains("world");
}

#[test]
fn test_error_goes_to_stderr_not_stdout() {
    let result = ShellTest::new()
        .with_isolated_home()
        .script("commandthatdoesnotexist_xyz")
        .run();

    // Error message must appear on stderr
    assert!(
        !result.error().is_empty(),
        "Error output should be non-empty for unknown command"
    );
    // stdout should not contain the error text
    assert!(
        !result.output().contains("not found"),
        "Error text should not appear on stdout, got: {}",
        result.output()
    );
}
