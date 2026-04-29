#[allow(dead_code)]
mod common;

use common::ShellHarness;

#[test]
fn repl_shows_prompt_at_startup() {
    let out = ShellHarness::new().run("");
    let stdout = out.stdout();
    assert!(
        stdout.contains('>') || stdout.contains('❯') || stdout.contains('$'),
        "expected prompt character in output, got: {stdout:?}"
    );
}

#[test]
fn repl_ignores_empty_lines() {
    ShellHarness::new()
        .run("\n\necho test")
        .assert_stdout_contains("test");
}

#[test]
fn whitespace_only_lines_produce_no_errors() {
    ShellHarness::new()
        .run("   \n\t\n  \t  \necho alive")
        .assert_stderr_empty()
        .assert_stdout_contains("alive");
}

#[cfg(unix)]
#[test]
fn nonzero_exit_from_external_command_reports_error() {
    let out = ShellHarness::new().run("false");
    assert!(
        out.stderr().contains("exited with status") || out.stderr().contains("exited"),
        "expected exit status error, got: {:?}",
        out.stderr()
    );
}

#[cfg(unix)]
#[test]
fn nonfatal_error_does_not_stop_subsequent_commands() {
    ShellHarness::new()
        .run("false\necho survived")
        .assert_stderr_contains("exited")
        .assert_stdout_contains("survived");
}

// When an unclosed quote reaches EOF without being closed, the accumulated
// input is reported as an UnclosedQuote error. The text after the newline is
// absorbed into the open quote rather than running as a separate command.
#[test]
fn unclosed_double_quote_at_eof_reports_error() {
    let out = ShellHarness::new().run("echo \"hello\necho inside_quote");
    let err = out.stderr();
    assert!(
        err.contains("unclosed") && err.contains("quote"),
        "got: {err}"
    );
    out.assert_stdout_not_contains("inside_quote");
}

#[test]
fn unclosed_single_quote_at_eof_reports_error() {
    let out = ShellHarness::new().run("echo 'hello\necho inside_quote");
    let err = out.stderr();
    assert!(
        err.contains("unclosed") && err.contains("quote"),
        "got: {err}"
    );
    out.assert_stdout_not_contains("inside_quote");
}

// A double-quoted string that spans two physical lines is accepted as a single
// command once the closing quote arrives.
#[test]
fn multiline_double_quoted_string_continues_until_closed() {
    ShellHarness::new()
        .run("echo \"line one\nline two\"\necho done")
        .assert_stderr_empty()
        .assert_stdout_contains("line one")
        .assert_stdout_contains("line two")
        .assert_stdout_contains("done");
}

#[test]
fn multiline_single_quoted_string_continues_until_closed() {
    ShellHarness::new()
        .run("echo 'line one\nline two'\necho done")
        .assert_stderr_empty()
        .assert_stdout_contains("line one")
        .assert_stdout_contains("line two")
        .assert_stdout_contains("done");
}

// A trailing backslash joins the next physical line, removing the backslash
// and newline from the input.
#[test]
fn trailing_backslash_joins_next_line() {
    ShellHarness::new()
        .run("echo hello \\\nworld\necho done")
        .assert_stderr_empty()
        .assert_stdout_contains("hello")
        .assert_stdout_contains("world")
        .assert_stdout_contains("done");
}

#[test]
fn unknown_command_error_goes_to_stderr_not_stdout() {
    let out = ShellHarness::new().run("commandthatdoesnotexist_xyz");
    assert!(
        !out.stderr().is_empty(),
        "expected error output on stderr for unknown command"
    );
    out.assert_stdout_not_contains("not found");
}
