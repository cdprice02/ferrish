//! Common utilities for integration tests

use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};

/// Helper to run shell commands in an isolated environment
pub struct ShellTestSession {
    temp_dir: tempfile::TempDir,
    temp_path: PathBuf,
    working_dir: PathBuf,
    home_dir: PathBuf,
}

impl ShellTestSession {
    pub fn new() -> Self {
        let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");
        let temp_path = temp_dir.path().to_path_buf();
        let working_dir = temp_path.clone();
        let home_dir = temp_path.join("home");
        std::fs::create_dir(&home_dir).expect("Failed to create home dir");

        Self {
            temp_dir,
            temp_path: temp_path.clone(),
            working_dir,
            home_dir,
        }
    }

    /// Get the temporary directory path
    pub fn temp_path(&self) -> &PathBuf {
        &self.temp_path
    }

    /// Get the working directory
    pub fn working_dir(&self) -> &PathBuf {
        &self.working_dir
    }

    /// Get the home directory
    pub fn home_dir(&self) -> &PathBuf {
        &self.home_dir
    }

    /// Run shell commands and return the result
    pub fn run(&self, commands: &[&str]) -> TestResult {
        let bin_path = env!("CARGO_BIN_EXE_ferrish");

        let input = commands.join("\n") + "\nexit\n";

        let mut cmd = Command::new(bin_path);
        cmd.stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .env("HOME", &self.home_dir)
            .env("USERPROFILE", &self.home_dir)
            .current_dir(&self.working_dir);

        let mut child = cmd.spawn().expect("Failed to spawn shell");

        if let Some(mut stdin) = child.stdin.take() {
            stdin
                .write_all(input.as_bytes())
                .expect("Failed to write to stdin");
        }

        let output = child.wait_with_output().expect("Failed to wait for shell");

        TestResult {
            output: String::from_utf8_lossy(&output.stdout).to_string(),
            error: String::from_utf8_lossy(&output.stderr).to_string(),
            exit_code: output.status.code(),
        }
    }
}

pub struct TestResult {
    pub output: String,
    pub error: String,
    pub exit_code: Option<i32>,
}

impl TestResult {
    pub fn output_contains(&self, s: &str) -> bool {
        self.output.contains(s)
    }

    pub fn error_contains(&self, s: &str) -> bool {
        self.error.contains(s)
    }

    pub fn assert_output_contains(&self, s: &str) {
        assert!(
            self.output.contains(s),
            "Expected `{}` in `{}`",
            s,
            self.output,
        );
    }

    pub fn assert_error_contains(&self, s: &str) {
        assert!(
            self.error.contains(s),
            "Expected `{}` in `{}",
            s,
            self.error
        );
    }
}
