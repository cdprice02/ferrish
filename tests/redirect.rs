#[allow(dead_code)]
mod common;

use common::ShellHarness;

// ============================================================================
// Stdout redirection (> and 1>)
// ============================================================================

#[test]
fn redirect_stdout_creates_file() {
    let out = ShellHarness::new().run("echo hello > out.txt");
    out.assert_stdout_not_contains("hello");
    let contents = std::fs::read_to_string(out.home_dir().join("out.txt")).expect("out.txt");
    assert_eq!(contents, "hello\n");
}

#[test]
fn redirect_1gt_is_equivalent_to_gt() {
    let out = ShellHarness::new().run("echo world 1> out.txt");
    out.assert_stdout_not_contains("world");
    let contents = std::fs::read_to_string(out.home_dir().join("out.txt")).expect("out.txt");
    assert_eq!(contents, "world\n");
}

#[test]
fn redirect_overwrites_existing_file() {
    let out = ShellHarness::new()
        .with_file("existing.txt", "old content\n")
        .run("echo new content > existing.txt");
    let contents = std::fs::read_to_string(out.home_dir().join("existing.txt")).expect("existing.txt");
    assert_eq!(contents, "new content\n");
}

#[test]
fn redirect_multi_word_echo_writes_single_line() {
    let out = ShellHarness::new().run("echo foo bar baz > words.txt");
    out.assert_stdout_not_contains("foo");
    let contents = std::fs::read_to_string(out.home_dir().join("words.txt")).expect("words.txt");
    assert_eq!(contents, "foo bar baz\n");
}

#[test]
fn no_redirect_goes_to_stdout() {
    ShellHarness::new().run("echo visible").assert_stdout_contains("visible");
}

#[cfg(unix)]
#[test]
fn redirect_external_executable_stdout_goes_to_file() {
    let out = ShellHarness::new()
        .with_file("input.txt", "extout\n")
        .run("cat input.txt > out.txt");
    out.assert_stdout_not_contains("extout");
    let contents = std::fs::read_to_string(out.home_dir().join("out.txt")).expect("out.txt");
    assert_eq!(contents, "extout\n");
}

#[test]
fn redirect_does_not_persist_to_next_command() {
    let out = ShellHarness::new().run("echo first > first.txt\necho second");
    out.assert_stdout_contains("second")
        .assert_stdout_not_contains("first");
    let contents = std::fs::read_to_string(out.home_dir().join("first.txt")).expect("first.txt");
    assert_eq!(contents, "first\n");
}

// ============================================================================
// Stdout append redirection (>>)
// ============================================================================

#[test]
fn append_redirect_creates_file_when_absent() {
    let out = ShellHarness::new().run("echo line1 >> out.txt");
    out.assert_stdout_not_contains("line1");
    let contents = std::fs::read_to_string(out.home_dir().join("out.txt")).expect("out.txt");
    assert_eq!(contents, "line1\n");
}

#[test]
fn append_redirect_preserves_existing_content() {
    let out = ShellHarness::new()
        .with_file("out.txt", "existing\n")
        .run("echo appended >> out.txt");
    let contents = std::fs::read_to_string(out.home_dir().join("out.txt")).expect("out.txt");
    assert_eq!(contents, "existing\nappended\n");
}

#[test]
fn append_redirect_two_commands_accumulate() {
    let out = ShellHarness::new().run("echo first >> acc.txt\necho second >> acc.txt");
    let contents = std::fs::read_to_string(out.home_dir().join("acc.txt")).expect("acc.txt");
    assert_eq!(contents, "first\nsecond\n");
}

#[test]
fn truncate_redirect_after_append_overwrites() {
    let out = ShellHarness::new().run("echo appended >> out.txt\necho truncated > out.txt");
    let contents = std::fs::read_to_string(out.home_dir().join("out.txt")).expect("out.txt");
    assert_eq!(contents, "truncated\n");
}

#[test]
fn append_redirect_trailing_operator_kept_as_arg() {
    ShellHarness::new().run("echo >>").assert_stdout_contains(">>");
}

// ============================================================================
// Stderr redirection (2>)
// ============================================================================

#[test]
fn stderr_redirect_creates_file() {
    let out = ShellHarness::new().run("exit notanumber 2> err.txt");
    out.assert_stderr_not_contains("numeric argument required");
    let contents = std::fs::read_to_string(out.home_dir().join("err.txt")).expect("err.txt");
    assert!(contents.contains("numeric argument required"), "got: {contents}");
}

#[test]
fn stderr_redirect_stdout_still_goes_to_terminal() {
    let out = ShellHarness::new().run("echo visible 2> err.txt");
    out.assert_stdout_contains("visible");
    let contents = std::fs::read_to_string(out.home_dir().join("err.txt")).expect("err.txt");
    assert!(contents.is_empty(), "stderr file should be empty, got: {contents}");
}

#[test]
fn stderr_redirect_overwrites_existing_file() {
    let out = ShellHarness::new()
        .with_file("err.txt", "old content\n")
        .run("exit notanumber 2> err.txt");
    let contents = std::fs::read_to_string(out.home_dir().join("err.txt")).expect("err.txt");
    assert!(!contents.contains("old content"), "should have overwritten");
    assert!(contents.contains("numeric argument required"), "got: {contents}");
}

#[cfg(unix)]
#[test]
fn stderr_redirect_does_not_affect_subsequent_stdout() {
    let out = ShellHarness::new().run("cat /nonexistent 2> err.txt\necho after");
    out.assert_stdout_contains("after");
    let contents = std::fs::read_to_string(out.home_dir().join("err.txt")).expect("err.txt");
    assert!(!contents.is_empty(), "cat's stderr should have been captured");
}

// ============================================================================
// Stderr append redirection (2>>)
// ============================================================================

#[test]
fn stderr_append_redirect_creates_file_when_absent() {
    let out = ShellHarness::new().run("exit notanumber 2>> err.txt");
    out.assert_stderr_not_contains("numeric argument required");
    let contents = std::fs::read_to_string(out.home_dir().join("err.txt")).expect("err.txt");
    assert!(contents.contains("numeric argument required"), "got: {contents}");
}

#[test]
fn stderr_append_redirect_preserves_existing_content() {
    let out = ShellHarness::new()
        .with_file("err.txt", "existing\n")
        .run("exit notanumber 2>> err.txt");
    let contents = std::fs::read_to_string(out.home_dir().join("err.txt")).expect("err.txt");
    assert!(contents.starts_with("existing\n"), "original content gone: {contents}");
    assert!(contents.contains("numeric argument required"), "appended error missing: {contents}");
}

#[test]
fn stderr_append_redirect_stdout_still_goes_to_terminal() {
    let out = ShellHarness::new().run("echo visible 2>> err.txt");
    out.assert_stdout_contains("visible");
    let contents = std::fs::read_to_string(out.home_dir().join("err.txt")).expect("err.txt");
    assert!(contents.is_empty(), "stderr file should be empty, got: {contents}");
}

#[test]
fn stderr_append_redirect_trailing_operator_kept_as_arg() {
    ShellHarness::new().run("echo 2>>").assert_stdout_contains("2>>");
}

#[cfg(unix)]
#[test]
fn stderr_append_redirect_external_executable_appends_to_file() {
    let out = ShellHarness::new()
        .with_file("err.txt", "existing\n")
        .run("cat /nonexistent 2>> err.txt");
    let contents = std::fs::read_to_string(out.home_dir().join("err.txt")).expect("err.txt");
    assert!(contents.starts_with("existing\n"), "original content gone: {contents}");
    assert!(contents.contains("/nonexistent"), "cat's stderr not captured: {contents}");
    out.assert_stderr_not_contains("/nonexistent");
}
