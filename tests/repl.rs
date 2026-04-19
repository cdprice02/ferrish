mod harness;

use harness::ShellHarness;

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
    ShellHarness::new().run("\n\necho test").assert_stdout_contains("test");
}

#[test]
fn exit_nonzero_code_propagates() {
    ShellHarness::new().run("exit 1").assert_exit_code(1);
}

#[cfg(unix)]
#[test]
fn nonzero_exit_from_external_command_reports_error() {
    let out = ShellHarness::new().run("false");
    assert!(
        out.stderr().contains("exited with status") || out.stderr().contains("exited"),
        "expected exit status error, got: {:?}", out.stderr()
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

#[test]
fn unclosed_double_quote_reports_error_and_continues() {
    let out = ShellHarness::new().run("echo \"hello\necho survived");
    let err = out.stderr();
    assert!(err.contains("unclosed") && err.contains("quote"), "got: {err}");
    out.assert_stdout_contains("survived");
}

#[test]
fn unclosed_single_quote_reports_error_and_continues() {
    let out = ShellHarness::new().run("echo 'hello\necho survived");
    let err = out.stderr();
    assert!(err.contains("unclosed") && err.contains("quote"), "got: {err}");
    out.assert_stdout_contains("survived");
}

#[test]
fn unknown_command_error_goes_to_stderr_not_stdout() {
    let out = ShellHarness::new().run("commandthatdoesnotexist_xyz");
    assert!(!out.stderr().is_empty(), "expected error output on stderr for unknown command");
    out.assert_stdout_not_contains("not found");
}
