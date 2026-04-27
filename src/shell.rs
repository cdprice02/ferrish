use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;

use miette::IntoDiagnostic as _;

use crate::{
    ctx::{ShellConfig, ShellCtx},
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

    /// Run the interactive REPL loop, reading from stdin until `exit` is called
    /// or stdin is exhausted. Prompts and diagnostics go to the process's real
    /// stdout/stderr.
    pub fn run(&mut self) -> miette::Result<ExitCode> {
        let stdin = std::io::stdin();
        let mut reader = BufReader::new(stdin.lock());
        self.run_repl(&mut reader)
    }

    /// Run the REPL loop with injectable stdin — useful for driving the shell
    /// from a script or test without spawning a subprocess. Prompts and
    /// diagnostics still go to the process's real stdout/stderr.
    pub fn run_repl(&mut self, reader: &mut dyn BufRead) -> miette::Result<ExitCode> {
        let mut out = std::io::stdout();
        let mut err = std::io::stderr();
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
            match executor::execute_pipeline(pipeline, &mut self.ctx) {
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

    #[test]
    fn with_prompt_sets_prompt_string() {
        let shell = Shell::builder().with_prompt("TEST> ".to_string()).build();
        assert_eq!(shell.ctx.config.prompt, "TEST> ");
    }

    #[test]
    fn with_config_sets_prompt() {
        let config = ShellConfig { prompt: "CFG> ".to_string(), ..Default::default() };
        let shell = Shell::builder().with_config(config).build();
        assert_eq!(shell.ctx.config.prompt, "CFG> ");
    }

    #[test]
    fn with_prompt_overrides_with_config_prompt() {
        let config = ShellConfig { prompt: "CONFIG> ".to_string(), ..Default::default() };
        let shell = Shell::builder().with_config(config).with_prompt("OVERRIDE> ".to_string()).build();
        assert_eq!(shell.ctx.config.prompt, "OVERRIDE> ");
    }
}
