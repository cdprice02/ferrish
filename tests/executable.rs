mod harness;

use harness::ShellHarness;

#[test]
fn command_not_found_reports_error() {
    let out = ShellHarness::new().run("nonexistentcommandthatdefinitelydoesnotexist123");
    let combined = format!("{}{}", out.stdout(), out.stderr());
    assert!(combined.contains("not found"), "stdout={:?} stderr={:?}", out.stdout(), out.stderr());
}

#[test]
fn external_command_stdout_is_captured() {
    #[cfg(unix)]
    let (script, expected) = ("which sh", "sh");
    #[cfg(windows)]
    let (script, expected) = ("where cmd", "cmd");

    ShellHarness::new()
        .run(script)
        .assert_stderr_empty()
        .assert_stdout_contains(expected);
}

#[cfg(unix)]
#[test]
fn external_command_stderr_is_captured() {
    ShellHarness::new()
        .run("sh -c 'echo ferrish_stderr_test >&2'")
        .assert_stderr_contains("ferrish_stderr_test");
}

#[cfg(unix)]
#[test]
fn external_command_both_streams_captured() {
    ShellHarness::new()
        .run("which sh\nsh -c 'echo ferrish_err_both >&2'")
        .assert_stdout_contains("sh")
        .assert_stderr_contains("ferrish_err_both");
}
