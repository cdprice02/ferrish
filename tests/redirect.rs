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
