//! Integration tests for `Shell::run_script()` and `ShellBuilder::with_config()`/`with_prompt()`.
//!
//! These tests intentionally bypass the `ShellTest` harness (which calls `shell.run()`) so that
//! `run_script()` and the builder methods under test are exercised directly.

use ferrish::Shell;
use ferrish::ctx::ShellConfig;
use ferrish::exit::ExitCode;
use ferrish::io::MockIo;

// ============================================================================
// run_script() tests
// ============================================================================

/// Basic command execution: a single `echo` line produces the expected output.
#[test]
fn test_run_script_basic_echo() {
    let io = MockIo::empty();
    let mut shell = Shell::builder().with_io(io);
    let exit_code = shell.run_script(&["echo hello"]).unwrap();
    assert_eq!(exit_code, ExitCode::SUCCESS);
    let output = String::from_utf8_lossy(shell.io().output()).into_owned();
    assert_eq!(output.trim(), "hello");
}

/// Early exit: once `exit` is encountered subsequent lines must not execute.
#[test]
fn test_run_script_early_exit_stops_execution() {
    let io = MockIo::empty();
    let mut shell = Shell::builder().with_io(io);
    let exit_code = shell.run_script(&["echo a", "exit", "echo b"]).unwrap();
    assert_eq!(exit_code, ExitCode::SUCCESS);
    let output = String::from_utf8_lossy(shell.io().output()).into_owned();
    assert!(output.contains("a"), "Expected 'a' in output, got: {output}");
    assert!(!output.contains("b"), "Expected 'b' to be absent after exit, got: {output}");
}

/// Empty and whitespace-only lines are skipped without error.
#[test]
fn test_run_script_empty_lines_are_skipped() {
    let io = MockIo::empty();
    let mut shell = Shell::builder().with_io(io);
    let exit_code = shell.run_script(&["", "  ", "echo ok"]).unwrap();
    assert_eq!(exit_code, ExitCode::SUCCESS);
    let output = String::from_utf8_lossy(shell.io().output()).into_owned();
    assert!(output.contains("ok"), "Expected 'ok' in output, got: {output}");
}

/// A script with no `exit` call returns `ExitCode::SUCCESS`.
#[test]
fn test_run_script_returns_success_on_completion() {
    let io = MockIo::empty();
    let mut shell = Shell::builder().with_io(io);
    let exit_code = shell
        .run_script(&["echo first", "echo second"])
        .unwrap();
    assert_eq!(exit_code, ExitCode::SUCCESS);
}

/// `exit <n>` propagates the numeric code through `run_script()`.
#[test]
fn test_run_script_exit_code_propagated() {
    let io = MockIo::empty();
    let mut shell = Shell::builder().with_io(io);
    let exit_code = shell.run_script(&["exit 42"]).unwrap();
    assert_eq!(exit_code.0, 42);
}

/// An empty script slice returns `ExitCode::SUCCESS` immediately.
#[test]
fn test_run_script_empty_slice() {
    let io = MockIo::empty();
    let mut shell = Shell::builder().with_io(io);
    let exit_code = shell.run_script(&[]).unwrap();
    assert_eq!(exit_code, ExitCode::SUCCESS);
}

// ============================================================================
// ShellBuilder tests
// ============================================================================

/// `with_prompt` — the provided prompt string appears in `run()` output once per command.
///
/// Two commands are fed (`echo hi` + `exit`) so the REPL loop shows the prompt twice,
/// verifying that the prompt is printed before *each* line and not just at startup.
#[test]
fn test_builder_with_prompt_appears_in_output() {
    let custom_prompt = "$ ".to_string();
    // Feed two commands so the REPL prints the prompt twice.
    let io = MockIo::from_lines(&["echo hi", "exit"]);
    let mut shell = Shell::builder()
        .with_prompt(custom_prompt.clone())
        .with_io(io);
    let _exit_code = shell.run().unwrap();
    let output = String::from_utf8_lossy(shell.io().output()).into_owned();
    let prompt_count = output.matches(custom_prompt.as_str()).count();
    assert_eq!(
        prompt_count, 2,
        "Expected prompt '{}' to appear 2 times (once per command), got {prompt_count} in: {output}",
        custom_prompt
    );
}

/// `with_config` — a `ShellConfig` with a custom prompt is applied correctly.
#[test]
fn test_builder_with_config_applies_prompt() {
    let config = ShellConfig {
        prompt: ">> ".to_string(),
        ..Default::default()
    };
    let io = MockIo::from_lines(&["exit"]);
    let mut shell = Shell::builder().with_config(config).with_io(io);
    let _exit_code = shell.run().unwrap();
    let output = String::from_utf8_lossy(shell.io().output()).into_owned();
    assert!(
        output.contains(">> "),
        "Expected prompt '>> ' in output, got: {output}"
    );
}

/// `with_prompt` overrides the default prompt; default crab prompt must be absent.
#[test]
fn test_builder_with_prompt_overrides_default() {
    let io = MockIo::from_lines(&["exit"]);
    let mut shell = Shell::builder()
        .with_prompt("MYPROMPT ".to_string())
        .with_io(io);
    let _exit_code = shell.run().unwrap();
    let output = String::from_utf8_lossy(shell.io().output()).into_owned();
    assert!(
        output.contains("MYPROMPT "),
        "Expected custom prompt in output, got: {output}"
    );
    // Default prompt emoji should be replaced
    assert!(
        !output.contains("🦀"),
        "Default crab prompt should be replaced, got: {output}"
    );
}

/// `with_config` followed by `with_prompt` — last call wins for the prompt.
#[test]
fn test_builder_with_prompt_after_with_config() {
    let config = ShellConfig {
        prompt: "config_prompt ".to_string(),
        ..Default::default()
    };
    let io = MockIo::from_lines(&["exit"]);
    let mut shell = Shell::builder()
        .with_config(config)
        .with_prompt("final_prompt ".to_string())
        .with_io(io);
    let _exit_code = shell.run().unwrap();
    let output = String::from_utf8_lossy(shell.io().output()).into_owned();
    assert!(
        output.contains("final_prompt "),
        "Expected 'final_prompt' in output, got: {output}"
    );
    assert!(
        !output.contains("config_prompt"),
        "Expected 'config_prompt' to be overridden, got: {output}"
    );
}
