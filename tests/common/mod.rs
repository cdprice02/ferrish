use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use ferrish::fs::canonicalize_path;

const FERRISH_BIN: &str = env!("CARGO_BIN_EXE_ferrish");

/// Subprocess test harness. Always runs in an isolated temp directory so tests
/// never share filesystem state. Call `.run()` to spawn the ferrish binary,
/// feed it commands via stdin, and get back a [`ShellOutput`] to assert on.
pub struct ShellHarness {
    temp_dir: tempfile::TempDir,
    setup_dirs: Vec<String>,
    setup_files: Vec<(String, String)>,
}

impl Default for ShellHarness {
    fn default() -> Self {
        Self::new()
    }
}

impl ShellHarness {
    pub fn new() -> Self {
        Self {
            temp_dir: tempfile::tempdir().expect("create temp dir"),
            setup_dirs: Vec::new(),
            setup_files: Vec::new(),
        }
    }

    /// Pre-create a directory relative to the isolated home before running.
    pub fn with_dir(mut self, path: &str) -> Self {
        self.setup_dirs.push(path.to_string());
        self
    }

    /// Pre-create a file with the given contents relative to the isolated home.
    pub fn with_file(mut self, path: &str, contents: &str) -> Self {
        self.setup_files.push((path.to_string(), contents.to_string()));
        self
    }

    /// Spawn ferrish, feed `script` as stdin (lines separated by `\n`), and
    /// capture stdout/stderr/exit code. The process exits cleanly on stdin EOF.
    pub fn run(self, script: &str) -> ShellOutput {
        // Canonicalize to the long path form; on Windows tempfile may return
        // short 8.3 paths, but the OS resolves cwd to the long form in `pwd`.
        let home = canonicalize_path(self.temp_dir.path().to_path_buf())
            .unwrap_or_else(|_| self.temp_dir.path().to_path_buf());

        for dir in &self.setup_dirs {
            std::fs::create_dir_all(home.join(dir)).expect("setup: create dir");
        }
        for (path, contents) in &self.setup_files {
            let full = home.join(path);
            if let Some(parent) = full.parent() {
                std::fs::create_dir_all(parent).expect("setup: create parent dir");
            }
            std::fs::write(&full, contents).expect("setup: write file");
        }

        let mut cmd = Command::new(FERRISH_BIN);
        cmd.stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .env("HOME", &home)
            .current_dir(&home);

        // On Windows the home directory comes from USERPROFILE / HOMEDRIVE+HOMEPATH.
        #[cfg(windows)]
        {
            cmd.env("USERPROFILE", &home);
            if let Some(s) = home.to_str() {
                if let Some((drive, path)) = s.split_once(':') {
                    cmd.env("HOMEDRIVE", format!("{drive}:"));
                    cmd.env("HOMEPATH", if path.is_empty() { "\\" } else { path });
                }
            }
        }

        // When running under cargo-llvm-cov, propagate LLVM_PROFILE_FILE so the
        // child process writes its own profraw into the same collection directory.
        #[cfg(coverage)]
        if let Ok(template) = std::env::var("LLVM_PROFILE_FILE") {
            cmd.env("LLVM_PROFILE_FILE", template);
        }

        let mut child = cmd.spawn().expect("spawn ferrish");

        let stdin_input = script.to_string();
        let mut stdin = child.stdin.take().expect("take stdin");
        // Write stdin on a separate thread so the child can keep making progress
        // if it is blocked waiting for input or stdin EOF while the parent waits
        // for process completion and collects its output.
        let writer = std::thread::spawn(move || {
            let _ = stdin.write_all(stdin_input.as_bytes());
        });

        let output = child.wait_with_output().expect("wait for ferrish");
        let _ = writer.join();

        ShellOutput {
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
            exit_code: output.status.code().unwrap_or(-1),
            home_dir: home,
            _temp_dir: self.temp_dir,
        }
    }
}

/// Captured output from a single ferrish subprocess run.
pub struct ShellOutput {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
    home_dir: PathBuf,
    /// Keeps the temp dir alive so callers can inspect files after the run.
    _temp_dir: tempfile::TempDir,
}

impl ShellOutput {
    pub fn stdout(&self) -> &str {
        &self.stdout
    }

    pub fn stderr(&self) -> &str {
        &self.stderr
    }

    pub fn exit_code(&self) -> i32 {
        self.exit_code
    }

    /// Path to the isolated home directory used for this run.
    pub fn home_dir(&self) -> &Path {
        &self.home_dir
    }

    #[track_caller]
    pub fn assert_stdout_contains(&self, s: &str) -> &Self {
        assert!(
            self.stdout.contains(s),
            "expected stdout to contain {s:?}\nstdout:\n{}\nstderr:\n{}",
            self.stdout, self.stderr,
        );
        self
    }

    #[track_caller]
    pub fn assert_stdout_not_contains(&self, s: &str) -> &Self {
        assert!(
            !self.stdout.contains(s),
            "expected stdout NOT to contain {s:?}\nstdout:\n{}",
            self.stdout,
        );
        self
    }

    #[track_caller]
    pub fn assert_stdout_empty(&self) -> &Self {
        assert!(self.stdout.is_empty(), "expected stdout to be empty\nstdout:\n{}", self.stdout);
        self
    }

    #[track_caller]
    pub fn assert_stderr_contains(&self, s: &str) -> &Self {
        assert!(
            self.stderr.contains(s),
            "expected stderr to contain {s:?}\nstderr:\n{}\nstdout:\n{}",
            self.stderr, self.stdout,
        );
        self
    }

    #[track_caller]
    pub fn assert_stderr_not_contains(&self, s: &str) -> &Self {
        assert!(
            !self.stderr.contains(s),
            "expected stderr NOT to contain {s:?}\nstderr:\n{}",
            self.stderr,
        );
        self
    }

    #[track_caller]
    pub fn assert_stderr_empty(&self) -> &Self {
        assert!(self.stderr.is_empty(), "expected stderr to be empty\nstderr:\n{}", self.stderr);
        self
    }

    #[track_caller]
    pub fn assert_exit_success(&self) -> &Self {
        assert_eq!(
            self.exit_code, 0,
            "expected exit code 0, got {}\nstdout:\n{}\nstderr:\n{}",
            self.exit_code, self.stdout, self.stderr,
        );
        self
    }

    #[track_caller]
    pub fn assert_exit_nonzero(&self) -> &Self {
        assert_ne!(
            self.exit_code, 0,
            "expected nonzero exit code\nstdout:\n{}\nstderr:\n{}",
            self.stdout, self.stderr,
        );
        self
    }

    #[track_caller]
    pub fn assert_exit_code(&self, code: i32) -> &Self {
        assert_eq!(
            self.exit_code, code,
            "expected exit code {code}, got {}\nstdout:\n{}\nstderr:\n{}",
            self.exit_code, self.stdout, self.stderr,
        );
        self
    }
}
