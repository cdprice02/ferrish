//! Integration test harness — runs commands through the ferrish library directly
//! using MockIo, so tarpaulin captures coverage from all integration tests.

use std::path::PathBuf;
use std::sync::Mutex;

use ferrish::io::MockIo;
use ferrish::Shell;

/// Serializes tests that touch global process state (CWD, HOME).
/// All integration tests go through this to avoid races between parallel test threads.
static PROCESS_MUTEX: Mutex<()> = Mutex::new(());

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
    pub fn new() -> Self {
        Self {
            home_dir: None,
            temp_dir: None,
            script: None,
        }
    }

    pub fn script(mut self, commands: &str) -> Self {
        self.script = Some(commands.to_string());
        self
    }

    pub fn with_isolated_home(mut self) -> Self {
        let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");
        let home = temp_dir.path().to_path_buf();
        self.home_dir = Some(home);
        self.temp_dir = Some(temp_dir);
        self
    }

    pub fn run(self) -> TestResult {
        // Serialize all tests — CWD and HOME are process-global state.
        let _guard = PROCESS_MUTEX.lock().unwrap_or_else(|e| e.into_inner());

        let original_cwd = std::env::current_dir().ok();
        let original_home = std::env::var("HOME").ok();
        let original_userprofile = std::env::var("USERPROFILE").ok();

        if let Some(home) = &self.home_dir {
            // SAFETY: this function holds PROCESS_MUTEX, serializing all
            // env-var mutations across test threads.
            unsafe {
                std::env::set_var("HOME", home);
                std::env::set_var("USERPROFILE", home);
            }
            let _ = std::env::set_current_dir(home);
        }

        // Build MockIo input from the script lines, appending "exit" so Shell::run()
        // terminates cleanly instead of looping on EOF.
        let mut lines: Vec<String> = self
            .script
            .as_deref()
            .unwrap_or("")
            .lines()
            .map(str::to_string)
            .collect();
        lines.push("exit".to_string());

        let line_refs: Vec<&str> = lines.iter().map(String::as_str).collect();
        let io = MockIo::from_lines(&line_refs);
        let mut shell = Shell::builder().with_io(io);
        let _ = shell.run();

        let output = String::from_utf8_lossy(shell.io().output()).to_string();
        let error = String::from_utf8_lossy(shell.io().error()).to_string();

        // Restore process state before self.temp_dir is dropped.
        if let Some(cwd) = original_cwd {
            let _ = std::env::set_current_dir(&cwd);
        }
        // SAFETY: PROCESS_MUTEX is held for the duration of this function.
        unsafe {
            restore_env("HOME", original_home);
            restore_env("USERPROFILE", original_userprofile);
        }

        TestResult { output, error }
        // self drops here (temp_dir cleaned up), then _guard (mutex released)
    }
}

/// # Safety
/// Callers must hold `PROCESS_MUTEX` to serialize env-var mutations.
unsafe fn restore_env(key: &str, original: Option<String>) {
    // SAFETY: caller holds PROCESS_MUTEX.
    unsafe {
        match original {
            Some(val) => std::env::set_var(key, val),
            None => std::env::remove_var(key),
        }
    }
}

pub struct TestResult {
    output: String,
    #[allow(dead_code)]
    error: String,
}

impl TestResult {
    pub fn output(&self) -> &str {
        &self.output
    }

    #[allow(dead_code)]
    pub fn error(&self) -> &str {
        &self.error
    }

    #[allow(dead_code)]
    pub fn output_contains(&self, s: &str) -> bool {
        self.output.contains(s)
    }

    #[allow(dead_code)]
    pub fn assert_output_contains(&self, s: &str) {
        assert!(
            self.output.contains(s),
            "Expected `{}` in output:\n{}",
            s,
            self.output
        );
    }

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
