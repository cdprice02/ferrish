mod harness;

use harness::ShellTest;

// ============================================================================
// Stdout redirection (> and 1>) integration tests
// ============================================================================

/// `echo hello > out.txt` should create the file containing "hello\n" and
/// the text "hello" should NOT appear in terminal stdout.
#[test]
fn test_redirect_creates_file() {
    let result = ShellTest::new()
        .with_isolated_home()
        .script("echo hello > out.txt")
        .run();

    // "hello" should have gone to the file, not to terminal stdout.
    assert!(
        !result.output_contains("hello"),
        "redirected output must not appear on terminal stdout"
    );

    let home = result.home_dir().expect("has home dir");
    let contents = std::fs::read_to_string(home.join("out.txt"))
        .expect("out.txt should have been created by redirect");
    assert_eq!(contents, "hello\n");
}

/// `1>` is equivalent to `>`.
#[test]
fn test_redirect_1gt_equivalent_to_gt() {
    let result = ShellTest::new()
        .with_isolated_home()
        .script("echo world 1> out.txt")
        .run();

    assert!(
        !result.output_contains("world"),
        "redirected output must not appear on terminal stdout"
    );

    let home = result.home_dir().expect("has home dir");
    let contents = std::fs::read_to_string(home.join("out.txt"))
        .expect("out.txt should have been created by 1>");
    assert_eq!(contents, "world\n");
}

/// A second redirect to the same file should overwrite (not append) it.
#[test]
fn test_redirect_overwrites_existing_file() {
    let result = ShellTest::new()
        .with_isolated_home()
        .with_file("existing.txt", "old content\n")
        .script("echo new content > existing.txt")
        .run();

    assert!(
        !result.output_contains("new content"),
        "redirected output must not appear on terminal stdout"
    );

    let home = result.home_dir().expect("has home dir");
    let contents = std::fs::read_to_string(home.join("existing.txt"))
        .expect("existing.txt should still exist");
    assert_eq!(contents, "new content\n");
}

/// Multiple words are written as a single space-separated line (standard echo behaviour).
#[test]
fn test_redirect_multi_word_echo() {
    let result = ShellTest::new()
        .with_isolated_home()
        .script("echo foo bar baz > words.txt")
        .run();

    assert!(
        !result.output_contains("foo"),
        "redirected output must not appear on terminal stdout"
    );

    let home = result.home_dir().expect("has home dir");
    let contents = std::fs::read_to_string(home.join("words.txt"))
        .expect("words.txt should exist");
    assert_eq!(contents, "foo bar baz\n");
}

/// Commands without a redirect still produce normal terminal output.
#[test]
fn test_no_redirect_goes_to_stdout() {
    let result = ShellTest::new()
        .with_isolated_home()
        .script("echo visible")
        .run();

    assert!(
        result.output_contains("visible"),
        "non-redirected echo should appear on stdout"
    );
}

/// Redirect should work for external executables, not just builtins.
/// Uses `cat` (a PATH-resolved executable, not a ferrish builtin) to verify
/// that the threaded stdout drain/write path correctly routes output to the
/// redirect file rather than the terminal.
#[cfg(unix)]
#[test]
fn test_redirect_external_executable_stdout_goes_to_file() {
    let result = ShellTest::new()
        .with_isolated_home()
        .with_file("input.txt", "extout\n")
        .script("cat input.txt > out.txt")
        .run();

    assert!(
        !result.output_contains("extout"),
        "external executable stdout should not appear on terminal when redirected"
    );

    let home = result.home_dir().expect("has home dir");
    let contents = std::fs::read_to_string(home.join("out.txt"))
        .expect("out.txt should have been created by redirect");
    assert_eq!(contents, "extout\n");
}

/// After a redirect command, the next command (without redirect) still goes to stdout.
#[test]
fn test_redirect_does_not_persist_to_next_command() {
    let result = ShellTest::new()
        .with_isolated_home()
        .script("echo first > first.txt\necho second")
        .run();

    // "second" should be on stdout; "first" should only be in the file.
    assert!(
        result.output_contains("second"),
        "second echo should be on stdout"
    );
    assert!(
        !result.output_contains("first"),
        "first echo should not appear on stdout"
    );

    let home = result.home_dir().expect("has home dir");
    let contents = std::fs::read_to_string(home.join("first.txt")).expect("first.txt");
    assert_eq!(contents, "first\n");
}

// ============================================================================
// Stderr redirection (2>) integration tests
// ============================================================================

/// `exit notanumber 2> err.txt` should write the error message to the file
/// and NOT to the terminal stderr.
#[test]
fn test_stderr_redirect_creates_file() {
    let result = ShellTest::new()
        .with_isolated_home()
        .script("exit notanumber 2> err.txt")
        .run();

    // Error message should have gone to the file, not to terminal stderr.
    assert!(
        !result.error().contains("numeric argument required"),
        "redirected stderr must not appear on terminal stderr"
    );

    let home = result.home_dir().expect("has home dir");
    let contents = std::fs::read_to_string(home.join("err.txt"))
        .expect("err.txt should have been created by 2> redirect");
    assert!(
        contents.contains("numeric argument required"),
        "expected error message in redirect file, got: {contents}"
    );
}

/// `2>` should capture only stderr; stdout should still reach the terminal.
#[test]
fn test_stderr_redirect_stdout_still_goes_to_terminal() {
    let result = ShellTest::new()
        .with_isolated_home()
        .script("echo visible 2> err.txt")
        .run();

    // Stdout (echo output) should still appear on terminal.
    assert!(
        result.output_contains("visible"),
        "stdout should still go to terminal when only stderr is redirected"
    );

    let home = result.home_dir().expect("has home dir");
    let err_file = home.join("err.txt");
    // File should exist (created on open) and be empty (echo writes nothing to stderr).
    let contents = std::fs::read_to_string(&err_file)
        .expect("err.txt should have been created by 2>");
    assert!(
        contents.is_empty(),
        "stderr redirect file should be empty when no errors occur: {contents}"
    );
}

/// A second `2>` redirect to the same file should overwrite (not append) it.
#[test]
fn test_stderr_redirect_overwrites_existing_file() {
    let result = ShellTest::new()
        .with_isolated_home()
        .with_file("err.txt", "old content\n")
        .script("exit notanumber 2> err.txt")
        .run();

    let home = result.home_dir().expect("has home dir");
    let contents = std::fs::read_to_string(home.join("err.txt"))
        .expect("err.txt should exist after redirect");
    assert!(
        !contents.contains("old content"),
        "redirect should have overwritten old content"
    );
    assert!(
        contents.contains("numeric argument required"),
        "new error message should be in file: {contents}"
    );
}

/// `2>` captures a builtin's stderr output to a file and not to the terminal.
#[test]
fn test_stderr_redirect_captures_builtin_error_to_file() {
    let result = ShellTest::new()
        .with_isolated_home()
        .script("exit notanumber 2> err.txt")
        .run();

    assert!(
        !result.error().contains("numeric argument required"),
        "error written with 2> must not appear on terminal stderr"
    );

    let home = result.home_dir().expect("has home dir");
    let contents = std::fs::read_to_string(home.join("err.txt"))
        .expect("err.txt should exist");
    assert!(
        contents.contains("numeric argument required"),
        "error should be in the redirect file: {contents}"
    );
}

/// A `2>` on one command does not affect stdout of a subsequent command.
/// Uses `cat /nonexistent` which writes to stderr but is non-fatal (the shell
/// continues after reporting the non-zero exit), so `echo after` still runs.
#[cfg(unix)]
#[test]
fn test_stderr_redirect_does_not_affect_subsequent_stdout() {
    let result = ShellTest::new()
        .with_isolated_home()
        .script("cat /nonexistent 2> err.txt\necho after")
        .run();

    // `echo after` must have run and reached terminal stdout.
    assert!(
        result.output_contains("after"),
        "stdout of subsequent command must reach the terminal"
    );

    // cat's stderr should be in the file, not the terminal.
    let home = result.home_dir().expect("has home dir");
    let contents = std::fs::read_to_string(home.join("err.txt"))
        .expect("err.txt should exist");
    assert!(!contents.is_empty(), "cat's stderr should have been captured to file");
}
