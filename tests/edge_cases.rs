mod harness;

use harness::ShellHarness;

#[test]
fn whitespace_only_lines_produce_no_errors() {
    ShellHarness::new()
        .run("   \n\t\n  \t  \necho alive")
        .assert_stderr_empty()
        .assert_stdout_contains("alive");
}
