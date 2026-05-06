mod common;

use predicates::prelude::*;

use common::ferrish_cmd;

#[test]
fn repl_ignores_empty_lines() {
    ferrish_cmd()
        .write_stdin("\n\necho test\n")
        .assert()
        .success()
        .stdout(predicate::str::contains("test"));
}

#[test]
fn whitespace_only_lines_produce_no_errors() {
    ferrish_cmd()
        .write_stdin("   \n\t\n  \t  \necho alive\n")
        .assert()
        .success()
        .stderr(predicate::str::is_empty())
        .stdout(predicate::str::contains("alive"));
}

#[test]
fn nonzero_exit_from_external_command_reports_error() {
    let output = ferrish_cmd().write_stdin("false\n").output().unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("exited with status") || stderr.contains("exited"),
        "expected exit status error, got: {:?}",
        stderr
    );
}

#[test]
fn nonfatal_error_does_not_stop_subsequent_commands() {
    ferrish_cmd()
        .write_stdin("false\necho survived\n")
        .assert()
        .stderr(predicate::str::contains("exited"))
        .stdout(predicate::str::contains("survived"));
}

// When an unclosed quote reaches EOF without being closed, the accumulated
// input is reported as an UnclosedQuote error. The text after the newline is
// absorbed into the open quote rather than running as a separate command.
#[test]
fn unclosed_double_quote_at_eof_reports_error() {
    let output = ferrish_cmd()
        .write_stdin("echo \"hello\necho inside_quote\n")
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stderr.contains("unclosed") && stderr.contains("quote"),
        "got: {stderr}"
    );
    assert!(!stdout.contains("inside_quote"));
}

#[test]
fn unclosed_single_quote_at_eof_reports_error() {
    let output = ferrish_cmd()
        .write_stdin("echo 'hello\necho inside_quote\n")
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stderr.contains("unclosed") && stderr.contains("quote"),
        "got: {stderr}"
    );
    assert!(!stdout.contains("inside_quote"));
}

// A double-quoted string that spans two physical lines is accepted as a single
// command once the closing quote arrives.
#[test]
fn multiline_double_quoted_string_continues_until_closed() {
    ferrish_cmd()
        .write_stdin("echo \"line one\nline two\"\necho done\n")
        .assert()
        .success()
        .stderr(predicate::str::is_empty())
        .stdout(predicate::str::contains("line one"))
        .stdout(predicate::str::contains("line two"))
        .stdout(predicate::str::contains("done"));
}

#[test]
fn multiline_single_quoted_string_continues_until_closed() {
    ferrish_cmd()
        .write_stdin("echo 'line one\nline two'\necho done\n")
        .assert()
        .success()
        .stderr(predicate::str::is_empty())
        .stdout(predicate::str::contains("line one"))
        .stdout(predicate::str::contains("line two"))
        .stdout(predicate::str::contains("done"));
}

#[test]
fn trailing_backslash_joins_next_line() {
    ferrish_cmd()
        .write_stdin("echo hello \\\nworld\necho done\n")
        .assert()
        .success()
        .stderr(predicate::str::is_empty())
        .stdout(predicate::str::contains("hello"))
        .stdout(predicate::str::contains("world"))
        .stdout(predicate::str::contains("done"));
}

#[test]
fn backslash_inside_single_quote_is_not_continuation() {
    ferrish_cmd()
        .write_stdin("echo 'hello \\\nworld'\necho done\n")
        .assert()
        .success()
        .stderr(predicate::str::is_empty())
        .stdout(predicate::str::contains("hello"))
        .stdout(predicate::str::contains("world"))
        .stdout(predicate::str::contains("done"));
}

#[test]
fn unknown_command_error_goes_to_stderr_not_stdout() {
    let output = ferrish_cmd()
        .write_stdin("commandthatdoesnotexist_xyz\n")
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        !stderr.is_empty(),
        "expected error output on stderr for unknown command"
    );
    assert!(!stdout.contains("not found"));
}
