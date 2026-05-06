mod common;

use assert_fs::TempDir;
use assert_fs::prelude::*;
use predicates::prelude::*;

use common::ferrish_cmd;

// ============================================================================
// Stdout redirection (> and 1>)
// ============================================================================

#[test]
fn redirect_stdout_creates_file() {
    let temp = TempDir::new().unwrap();
    ferrish_cmd()
        .current_dir(temp.path())
        .write_stdin("echo hello > out.txt\n")
        .assert()
        .success()
        .stdout(predicate::str::contains("hello").not());
    let contents = std::fs::read_to_string(temp.path().join("out.txt")).expect("out.txt");
    assert_eq!(contents, "hello\n");
}

#[test]
fn redirect_1gt_is_equivalent_to_gt() {
    let temp = TempDir::new().unwrap();
    ferrish_cmd()
        .current_dir(temp.path())
        .write_stdin("echo world 1> out.txt\n")
        .assert()
        .success()
        .stdout(predicate::str::contains("world").not());
    let contents = std::fs::read_to_string(temp.path().join("out.txt")).expect("out.txt");
    assert_eq!(contents, "world\n");
}

#[test]
fn redirect_overwrites_existing_file() {
    let temp = TempDir::new().unwrap();
    temp.child("existing.txt")
        .write_str("old content\n")
        .unwrap();
    ferrish_cmd()
        .current_dir(temp.path())
        .write_stdin("echo new content > existing.txt\n")
        .output()
        .unwrap();
    let contents = std::fs::read_to_string(temp.path().join("existing.txt")).expect("existing.txt");
    assert_eq!(contents, "new content\n");
}

#[test]
fn redirect_multi_word_echo_writes_single_line() {
    let temp = TempDir::new().unwrap();
    ferrish_cmd()
        .current_dir(temp.path())
        .write_stdin("echo foo bar baz > words.txt\n")
        .assert()
        .success()
        .stdout(predicate::str::contains("foo").not());
    let contents = std::fs::read_to_string(temp.path().join("words.txt")).expect("words.txt");
    assert_eq!(contents, "foo bar baz\n");
}

#[test]
fn no_redirect_goes_to_stdout() {
    let temp = TempDir::new().unwrap();
    ferrish_cmd()
        .current_dir(temp.path())
        .write_stdin("echo visible\n")
        .assert()
        .success()
        .stdout(predicate::str::contains("visible"));
}

#[test]
fn redirect_builtin_stdout_goes_to_file() {
    let temp = TempDir::new().unwrap();
    temp.child("input.txt").write_str("extout\n").unwrap();
    ferrish_cmd()
        .current_dir(temp.path())
        .write_stdin("cat input.txt > out.txt\n")
        .assert()
        .success()
        .stdout(predicate::str::contains("extout").not());
    let contents = std::fs::read_to_string(temp.path().join("out.txt")).expect("out.txt");
    assert_eq!(contents, "extout\n");
}

#[test]
fn redirect_does_not_persist_to_next_command() {
    let temp = TempDir::new().unwrap();
    ferrish_cmd()
        .current_dir(temp.path())
        .write_stdin("echo first > first.txt\necho second\n")
        .assert()
        .success()
        .stdout(predicate::str::contains("second"))
        .stdout(predicate::str::contains("first").not());
    let contents = std::fs::read_to_string(temp.path().join("first.txt")).expect("first.txt");
    assert_eq!(contents, "first\n");
}

// ============================================================================
// Stdout append redirection (>>)
// ============================================================================

#[test]
fn append_redirect_creates_file_when_absent() {
    let temp = TempDir::new().unwrap();
    ferrish_cmd()
        .current_dir(temp.path())
        .write_stdin("echo line1 >> out.txt\n")
        .assert()
        .success()
        .stdout(predicate::str::contains("line1").not());
    let contents = std::fs::read_to_string(temp.path().join("out.txt")).expect("out.txt");
    assert_eq!(contents, "line1\n");
}

#[test]
fn append_redirect_preserves_existing_content() {
    let temp = TempDir::new().unwrap();
    temp.child("out.txt").write_str("existing\n").unwrap();
    ferrish_cmd()
        .current_dir(temp.path())
        .write_stdin("echo appended >> out.txt\n")
        .output()
        .unwrap();
    let contents = std::fs::read_to_string(temp.path().join("out.txt")).expect("out.txt");
    assert_eq!(contents, "existing\nappended\n");
}

#[test]
fn append_redirect_two_commands_accumulate() {
    let temp = TempDir::new().unwrap();
    ferrish_cmd()
        .current_dir(temp.path())
        .write_stdin("echo first >> acc.txt\necho second >> acc.txt\n")
        .output()
        .unwrap();
    let contents = std::fs::read_to_string(temp.path().join("acc.txt")).expect("acc.txt");
    assert_eq!(contents, "first\nsecond\n");
}

#[test]
fn truncate_redirect_after_append_overwrites() {
    let temp = TempDir::new().unwrap();
    ferrish_cmd()
        .current_dir(temp.path())
        .write_stdin("echo appended >> out.txt\necho truncated > out.txt\n")
        .output()
        .unwrap();
    let contents = std::fs::read_to_string(temp.path().join("out.txt")).expect("out.txt");
    assert_eq!(contents, "truncated\n");
}

#[test]
fn append_redirect_trailing_operator_kept_as_arg() {
    let temp = TempDir::new().unwrap();
    ferrish_cmd()
        .current_dir(temp.path())
        .write_stdin("echo >>\n")
        .assert()
        .success()
        .stdout(predicate::str::contains(">>"));
}

// ============================================================================
// Stderr redirection (2>)
// ============================================================================

#[test]
fn stderr_redirect_creates_file() {
    let temp = TempDir::new().unwrap();
    ferrish_cmd()
        .current_dir(temp.path())
        .write_stdin("exit notanumber 2> err.txt\n")
        .assert()
        .code(1)
        .stderr(predicate::str::contains("numeric argument required").not());
    let contents = std::fs::read_to_string(temp.path().join("err.txt")).expect("err.txt");
    assert!(
        contents.contains("numeric argument required"),
        "got: {contents}"
    );
}

#[test]
fn stderr_redirect_stdout_still_goes_to_terminal() {
    let temp = TempDir::new().unwrap();
    ferrish_cmd()
        .current_dir(temp.path())
        .write_stdin("echo visible 2> err.txt\n")
        .assert()
        .success()
        .stdout(predicate::str::contains("visible"));
    let contents = std::fs::read_to_string(temp.path().join("err.txt")).expect("err.txt");
    assert!(
        contents.is_empty(),
        "stderr file should be empty, got: {contents}"
    );
}

#[test]
fn stderr_redirect_overwrites_existing_file() {
    let temp = TempDir::new().unwrap();
    temp.child("err.txt").write_str("old content\n").unwrap();
    ferrish_cmd()
        .current_dir(temp.path())
        .write_stdin("exit notanumber 2> err.txt\n")
        .output()
        .unwrap();
    let contents = std::fs::read_to_string(temp.path().join("err.txt")).expect("err.txt");
    assert!(!contents.contains("old content"), "should have overwritten");
    assert!(
        contents.contains("numeric argument required"),
        "got: {contents}"
    );
}

#[test]
fn stderr_redirect_does_not_affect_subsequent_stdout() {
    let temp = TempDir::new().unwrap();
    ferrish_cmd()
        .current_dir(temp.path())
        .write_stdin("cat /nonexistent 2> err.txt\necho after\n")
        .assert()
        .success()
        .stdout(predicate::str::contains("after"));
    let contents = std::fs::read_to_string(temp.path().join("err.txt")).expect("err.txt");
    assert!(
        !contents.is_empty(),
        "cat's stderr should have been captured"
    );
}

// ============================================================================
// Stderr append redirection (2>>)
// ============================================================================

#[test]
fn stderr_append_redirect_creates_file_when_absent() {
    let temp = TempDir::new().unwrap();
    ferrish_cmd()
        .current_dir(temp.path())
        .write_stdin("exit notanumber 2>> err.txt\n")
        .assert()
        .code(1)
        .stderr(predicate::str::contains("numeric argument required").not());
    let contents = std::fs::read_to_string(temp.path().join("err.txt")).expect("err.txt");
    assert!(
        contents.contains("numeric argument required"),
        "got: {contents}"
    );
}

#[test]
fn stderr_append_redirect_preserves_existing_content() {
    let temp = TempDir::new().unwrap();
    temp.child("err.txt").write_str("existing\n").unwrap();
    ferrish_cmd()
        .current_dir(temp.path())
        .write_stdin("exit notanumber 2>> err.txt\n")
        .output()
        .unwrap();
    let contents = std::fs::read_to_string(temp.path().join("err.txt")).expect("err.txt");
    assert!(
        contents.starts_with("existing\n"),
        "original content gone: {contents}"
    );
    assert!(
        contents.contains("numeric argument required"),
        "appended error missing: {contents}"
    );
}

#[test]
fn stderr_append_redirect_stdout_still_goes_to_terminal() {
    let temp = TempDir::new().unwrap();
    ferrish_cmd()
        .current_dir(temp.path())
        .write_stdin("echo visible 2>> err.txt\n")
        .assert()
        .success()
        .stdout(predicate::str::contains("visible"));
    let contents = std::fs::read_to_string(temp.path().join("err.txt")).expect("err.txt");
    assert!(
        contents.is_empty(),
        "stderr file should be empty, got: {contents}"
    );
}

#[test]
fn stderr_append_redirect_trailing_operator_kept_as_arg() {
    let temp = TempDir::new().unwrap();
    ferrish_cmd()
        .current_dir(temp.path())
        .write_stdin("echo 2>>\n")
        .assert()
        .success()
        .stdout(predicate::str::contains("2>>"));
}

#[test]
fn stderr_append_redirect_builtin_appends_to_file() {
    let temp = TempDir::new().unwrap();
    temp.child("err.txt").write_str("existing\n").unwrap();
    ferrish_cmd()
        .current_dir(temp.path())
        .write_stdin("cat /nonexistent 2>> err.txt\n")
        .assert()
        .stderr(predicate::str::contains("/nonexistent").not());
    let contents = std::fs::read_to_string(temp.path().join("err.txt")).expect("err.txt");
    assert!(
        contents.starts_with("existing\n"),
        "original content gone: {contents}"
    );
    assert!(
        contents.contains("/nonexistent"),
        "cat's stderr not captured: {contents}"
    );
}
