//! Integration test harness — runs commands through the ferrish library directly
//! using MockIo, so tarpaulin captures coverage from all integration tests.

use std::io::Write as _;
use std::path::PathBuf;

use ferrish::io::MockIo;
use ferrish::Shell;

pub struct ShellTest {
    home_dir: Option<PathBuf>,
    temp_dir: Option<tempfile::TempDir>,
    script: Option<String>,
    setup_dirs: Vec<String>,
    setup_files: Vec<(String, String)>,
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
            setup_dirs: Vec::new(),
            setup_files: Vec::new(),
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

    /// Pre-create a directory relative to the isolated home before running the shell.
    /// Requires `with_isolated_home()` to be called first.
    #[allow(dead_code)]
    pub fn with_dir(mut self, path: &str) -> Self {
        self.setup_dirs.push(path.to_string());
        self
    }

    /// Pre-create a file with the given contents relative to the isolated home.
    /// Requires `with_isolated_home()` to be called first.
    #[allow(dead_code)]
    pub fn with_file(mut self, path: &str, contents: &str) -> Self {
        self.setup_files.push((path.to_string(), contents.to_string()));
        self
    }

    pub fn run(self) -> TestResult {
        let home = self.home_dir.clone();

        // Apply filesystem setup inside the temp dir.
        if let (Some(home), Some(_temp_dir)) = (&home, &self.temp_dir) {
            for dir in &self.setup_dirs {
                std::fs::create_dir_all(home.join(dir)).expect("setup: create dir");
            }
            for (path, contents) in &self.setup_files {
                let full = home.join(path);
                if let Some(parent) = full.parent() {
                    std::fs::create_dir_all(parent).expect("setup: create parent dir");
                }
                let mut f = std::fs::File::create(&full).expect("setup: create file");
                f.write_all(contents.as_bytes()).expect("setup: write file");
            }
        }

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

        let mut builder = Shell::builder();
        if let Some(h) = home {
            builder = builder.with_home_dir(h.clone()).with_cwd(h);
        }
        let mut shell = builder.with_io(io);
        let run_result = shell.run();

        let output = String::from_utf8_lossy(shell.io().output()).to_string();
        let error = String::from_utf8_lossy(shell.io().error()).to_string();

        let exit_code = run_result
            .unwrap_or_else(|err| {
                panic!("shell.run() failed: {err}\nstdout:\n{output}\nstderr:\n{error}");
            })
            .0;

        TestResult { output, error, home_dir: self.home_dir, exit_code }
        // self drops here — temp_dir is cleaned up
    }
}

pub struct TestResult {
    output: String,
    #[allow(dead_code)]
    error: String,
    #[allow(dead_code)]
    home_dir: Option<PathBuf>,
    #[allow(dead_code)]
    exit_code: u8,
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
    pub fn exit_code(&self) -> u8 {
        self.exit_code
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
    pub fn home_dir(&self) -> Option<&std::path::Path> {
        self.home_dir.as_deref()
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
