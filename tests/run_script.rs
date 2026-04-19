// Script-mode integration tests exercised via the real ferrish binary.
// Builder API and prompt behavior are unit-tested in src/shell.rs.
// These tests cover script-mode interactions not already in other test files.

mod harness;

use harness::ShellHarness;

#[test]
fn exit_stops_subsequent_commands() {
    // Unique to script mode: verifies that commands after `exit` do NOT run.
    ShellHarness::new()
        .run("echo a\nexit\necho b")
        .assert_stdout_contains("a")
        .assert_stdout_not_contains("b");
}
