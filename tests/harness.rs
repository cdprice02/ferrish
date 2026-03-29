//! Lean custom test harness for ferrish shell integration tests

use std::io::{Read, Write};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};

/// Lightweight builder for configuring and running shell tests
pub struct ShellTest {
    home_dir: Option<PathBuf>,
    temp_dir: Option<tempfile::TempDir>,
    script: Option<String>,
}

impl Default for ShellTest {
    fn default() -> Self {
        Self::new()
    }
}

impl ShellTest {
    /// Create a new shell test builder
    pub fn new() -> Self {
        Self {
            home_dir: None,
            temp_dir: None,
            script: None,
        }
    }

    /// Set script mode with commands to run
    pub fn script(mut self, commands: &str) -> Self {
        self.script = Some(commands.to_string());
        self
    }

    /// Create an isolated HOME directory
    #[allow(dead_code)]
    pub fn with_isolated_home(mut self) -> Self {
        let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");
        let home = temp_dir.path().to_path_buf();
        self.home_dir = Some(home);
        self.temp_dir = Some(temp_dir);
        self
    }

    /// Run the shell test and return the result
    pub fn run(self) -> TestResult {
        let bin_path = env!("CARGO_BIN_EXE_ferrish");

        let mut cmd = Command::new(bin_path);
        cmd.stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        if let Some(home) = &self.home_dir {
            cmd.env("HOME", home);
            cmd.env("USERPROFILE", home);
        }

        let mut child = cmd.spawn().expect("Failed to spawn shell");

        let script = self.script.unwrap_or_default();
        run_script_mode(&mut child, &script)
    }
}

/// Run script mode: write all commands at once, read all output
fn run_script_mode(child: &mut Child, script: &str) -> TestResult {
    let mut input = script.to_string();
    if !input.ends_with('\n') && !input.is_empty() {
        input.push('\n');
    }
    input.push_str("exit\n");

    if let Some(mut stdin) = child.stdin.take() {
        let _ = stdin.write_all(input.as_bytes());
    }

    let mut output = Vec::new();
    if let Some(mut stdout) = child.stdout.take() {
        let _ = stdout.read_to_end(&mut output);
    }

    let mut error = Vec::new();
    if let Some(mut stderr) = child.stderr.take() {
        let _ = stderr.read_to_end(&mut error);
    }

    let exit_code = child.wait().ok().and_then(|status| status.code());

    TestResult {
        output: String::from_utf8_lossy(&output).to_string(),
        error: String::from_utf8_lossy(&error).to_string(),
        exit_code,
    }
}

/// Result of running a shell test
pub struct TestResult {
    output: String,
    #[allow(dead_code)]
    error: String,
    #[allow(dead_code)]
    exit_code: Option<i32>,
}

impl TestResult {
    /// Get the standard output
    pub fn output(&self) -> &str {
        &self.output
    }

    /// Get the standard error
    #[allow(dead_code)]
    pub fn error(&self) -> &str {
        &self.error
    }

    /// Check if output contains a string (non-panicking)
    #[allow(dead_code)]
    pub fn output_contains(&self, s: &str) -> bool {
        self.output.contains(s)
    }

    /// Assert that output contains a string
    #[allow(dead_code)]
    pub fn assert_output_contains(&self, s: &str) {
        assert!(
            self.output.contains(s),
            "Expected `{}` in output:\n{}",
            s,
            self.output
        );
    }

    /// Assert that error contains a string
    #[allow(dead_code)]
    pub fn assert_error_contains(&self, s: &str) {
        assert!(
            self.error.contains(s),
            "Expected `{}` in error:\n{}",
            s,
            self.error
        );
    }
}
