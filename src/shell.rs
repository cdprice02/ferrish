use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;

use miette::IntoDiagnostic as _;

use crate::{
    ctx::{ShellConfig, ShellCtx},
    error::ShellResult,
    executor,
    exit::ExitCode,
    parser,
};

/// The ferrish shell REPL.
pub struct Shell {
    ctx: ShellCtx,
}

impl Shell {
    /// Create a shell builder.
    pub fn builder() -> ShellBuilder {
        ShellBuilder::default()
    }

    /// Run the interactive REPL loop, reading from stdin and writing to
    /// stdout/stderr until `exit` is called or stdin is exhausted.
    pub fn run(&mut self) -> miette::Result<ExitCode> {
        let stdin = std::io::stdin();
        let mut reader = BufReader::new(stdin.lock());
        let mut out = std::io::stdout();
        let mut err = std::io::stderr();
        self.run_repl(&mut reader, &mut out, &mut err)
    }

    /// Run the REPL loop with injectable I/O — useful for testing prompt
    /// behavior and other REPL properties without spawning a subprocess.
    pub fn run_repl(
        &mut self,
        reader: &mut dyn BufRead,
        out: &mut dyn Write,
        err: &mut dyn Write,
    ) -> miette::Result<ExitCode> {
        loop {
            out.write_all(self.ctx.config.prompt.as_bytes()).into_diagnostic()?;
            out.flush().into_diagnostic()?;

            let mut buffer = Vec::<u8>::new();
            let bytes = reader.read_until(b'\n', &mut buffer).into_diagnostic()?;
            if bytes == 0 {
                return Ok(ExitCode::SUCCESS);
            }

            let buffer = buffer.trim_ascii();
            if buffer.is_empty() {
                continue;
            }

            let make_src = || String::from_utf8_lossy(buffer).into_owned();
            let pipeline = match parser::parse(buffer) {
                Ok(r) => r,
                Err(e) => {
                    let report = miette::Report::new(e).with_source_code(make_src());
                    writeln!(err, "{report:?}").into_diagnostic()?;
                    continue;
                }
            };
            match executor::execute_pipeline(pipeline, out, err, &mut self.ctx) {
                Ok(Some(exit_code)) => return Ok(exit_code),
                Ok(None) => {}
                Err(e) => {
                    let fatal = e.is_fatal();
                    let report = miette::Report::new(e).with_source_code(make_src());
                    writeln!(err, "{report:?}").into_diagnostic()?;
                    if fatal {
                        return Ok(ExitCode::FAILURE);
                    }
                }
            }
        }
    }

    /// Execute a sequence of command lines as a non-interactive script,
    /// writing output to the provided `out` and `err` writers.
    pub fn run_script(
        &mut self,
        script: &[&str],
        out: &mut dyn Write,
        err: &mut dyn Write,
    ) -> miette::Result<ExitCode> {
        for line in script {
            let buffer = line.as_bytes().trim_ascii();
            if buffer.is_empty() {
                continue;
            }

            let pipeline = parser::parse(buffer).map_err(|e| {
                miette::Report::new(e)
                    .with_source_code(String::from_utf8_lossy(buffer).into_owned())
            })?;
            if let Some(exit_code) = self.execute_pipeline(pipeline, out, err)? {
                return Ok(exit_code);
            }
        }

        Ok(ExitCode::SUCCESS)
    }

    fn execute_pipeline(
        &mut self,
        pipeline: parser::Pipeline,
        out: &mut dyn Write,
        err: &mut dyn Write,
    ) -> ShellResult<Option<ExitCode>> {
        executor::execute_pipeline(pipeline, out, err, &mut self.ctx)
    }
}

/// Builder for constructing a [`Shell`] with custom configuration.
#[derive(Default)]
pub struct ShellBuilder {
    home_dir: Option<PathBuf>,
    cwd: Option<PathBuf>,
    config: Option<ShellConfig>,
}

impl ShellBuilder {
    /// Set an explicit home directory (overrides env HOME / USERPROFILE).
    pub fn with_home_dir(mut self, path: PathBuf) -> Self {
        self.home_dir = Some(path);
        self
    }

    /// Set an explicit initial working directory (overrides process CWD).
    pub fn with_cwd(mut self, path: PathBuf) -> Self {
        self.cwd = Some(path);
        self
    }

    /// Override the shell configuration.
    pub fn with_config(mut self, config: ShellConfig) -> Self {
        self.config = Some(config);
        self
    }

    /// Override only the prompt string.
    pub fn with_prompt(mut self, prompt: String) -> Self {
        let mut config = self.config.unwrap_or_default();
        config.prompt = prompt;
        self.config = Some(config);
        self
    }

    /// Build the [`Shell`].
    pub fn build(self) -> Shell {
        let base = ShellCtx::from_env();
        let ctx = ShellCtx::with_config(
            self.home_dir.or(base.home_dir),
            self.cwd.unwrap_or(base.cwd),
            self.config.unwrap_or_default(),
        );
        Shell { ctx }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn run_repl_executes_commands_and_captures_output() {
        let input = b"echo hello world\n";
        let mut reader = BufReader::new(Cursor::new(input.as_ref()));
        let mut out: Vec<u8> = Vec::new();
        let mut err: Vec<u8> = Vec::new();
        Shell::builder().build().run_repl(&mut reader, &mut out, &mut err).unwrap();
        assert!(String::from_utf8_lossy(&out).contains("hello world"));
    }

    #[test]
    fn run_repl_with_custom_prompt_writes_prompt_to_stdout() {
        let input = b"echo hi\n";
        let mut reader = BufReader::new(Cursor::new(input.as_ref()));
        let mut out: Vec<u8> = Vec::new();
        let mut err: Vec<u8> = Vec::new();
        Shell::builder()
            .with_prompt("PROMPT> ".to_string())
            .build()
            .run_repl(&mut reader, &mut out, &mut err)
            .unwrap();
        assert!(String::from_utf8_lossy(&out).contains("PROMPT> "));
    }

    #[test]
    fn run_repl_with_config_prompt_writes_prompt_to_stdout() {
        let config = ShellConfig { prompt: ">> ".to_string(), ..Default::default() };
        let input = b"echo hi\n";
        let mut reader = BufReader::new(Cursor::new(input.as_ref()));
        let mut out: Vec<u8> = Vec::new();
        let mut err: Vec<u8> = Vec::new();
        Shell::builder().with_config(config).build().run_repl(&mut reader, &mut out, &mut err).unwrap();
        assert!(String::from_utf8_lossy(&out).contains(">> "));
    }

    #[test]
    fn run_script_executes_lines_and_captures_output() {
        let mut out: Vec<u8> = Vec::new();
        let mut err: Vec<u8> = Vec::new();
        let code = Shell::builder().build().run_script(&["echo hello"], &mut out, &mut err).unwrap();
        assert_eq!(code, ExitCode::SUCCESS);
        assert_eq!(out.trim_ascii(), b"hello");
    }

    #[test]
    fn run_script_stops_at_exit_skipping_subsequent_lines() {
        let mut out: Vec<u8> = Vec::new();
        let mut err: Vec<u8> = Vec::new();
        Shell::builder().build().run_script(&["echo a", "exit", "echo b"], &mut out, &mut err).unwrap();
        let stdout = String::from_utf8_lossy(&out);
        assert!(stdout.contains("a"), "expected 'a'");
        assert!(!stdout.contains("b"), "expected 'b' not to run after exit");
    }

    #[test]
    fn run_script_skips_empty_and_whitespace_lines() {
        let mut out: Vec<u8> = Vec::new();
        let mut err: Vec<u8> = Vec::new();
        let code = Shell::builder().build().run_script(&["", "  ", "echo ok"], &mut out, &mut err).unwrap();
        assert_eq!(code, ExitCode::SUCCESS);
        assert!(String::from_utf8_lossy(&out).contains("ok"));
    }

    #[test]
    fn run_script_propagates_exit_code() {
        let mut out: Vec<u8> = Vec::new();
        let mut err: Vec<u8> = Vec::new();
        let code = Shell::builder().build().run_script(&["exit 42"], &mut out, &mut err).unwrap();
        assert_eq!(code.0, 42);
    }

    #[test]
    fn run_script_empty_slice_returns_success() {
        let mut out: Vec<u8> = Vec::new();
        let mut err: Vec<u8> = Vec::new();
        let code = Shell::builder().build().run_script(&[], &mut out, &mut err).unwrap();
        assert_eq!(code, ExitCode::SUCCESS);
    }

    #[test]
    fn builder_with_prompt_after_with_config_last_call_wins() {
        let config = ShellConfig { prompt: "config_prompt ".to_string(), ..Default::default() };
        let input = b"echo hi\n";
        let mut reader = BufReader::new(Cursor::new(input.as_ref()));
        let mut out: Vec<u8> = Vec::new();
        let mut err: Vec<u8> = Vec::new();
        Shell::builder()
            .with_config(config)
            .with_prompt("final_prompt ".to_string())
            .build()
            .run_repl(&mut reader, &mut out, &mut err)
            .unwrap();
        let stdout = String::from_utf8_lossy(&out);
        assert!(stdout.contains("final_prompt "), "got: {stdout}");
        assert!(!stdout.contains("config_prompt"), "should be overridden, got: {stdout}");
    }
}
