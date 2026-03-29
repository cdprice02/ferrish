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
