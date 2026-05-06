use std::io::{BufRead, Write};
use std::path::PathBuf;

use miette::IntoDiagnostic as _;

use crate::{
    ctx::{ShellConfig, ShellCtx},
    executor,
    exit::ExitCode,
    input::Input,
    line_reader::{InteractiveReader, LineInput, LineReader, ScriptReader},
    parser, resolver,
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

    /// Run the interactive REPL using reedline for line editing.
    ///
    /// Reads from the terminal until `exit` is called or the user signals EOF
    /// (Ctrl+D). Ctrl+C abandons the current input and returns to the prompt.
    /// Diagnostics go to the process's real stderr.
    pub fn run_interactive(&mut self) -> miette::Result<ExitCode> {
        let mut reader = InteractiveReader::new(&self.ctx.config)?;
        self.run_loop(&mut reader)
    }

    /// Run the shell loop from a [`BufRead`] source.
    ///
    /// This is the entry point for script / batch mode; interactive use goes
    /// through [`Shell::run_interactive`]. Prompts are suppressed; diagnostics
    /// go to the process's real stderr.
    pub fn run_script(&mut self, source: &mut dyn BufRead) -> miette::Result<ExitCode> {
        let mut reader = ScriptReader::new(source);
        self.run_loop(&mut reader)
    }

    /// Shared REPL loop driven by any [`LineReader`].
    ///
    /// Each call to [`LineReader::read_line`] returns a complete logical
    /// command — accumulation and continuation are handled inside the reader.
    fn run_loop(&mut self, reader: &mut dyn LineReader) -> miette::Result<ExitCode> {
        let mut err = std::io::stderr();
        loop {
            match reader.read_line()? {
                LineInput::Eof => return Ok(ExitCode::SUCCESS),
                LineInput::Interrupted => continue,
                LineInput::Line(bytes) => {
                    let input = Input::from_vec(bytes);
                    if input.is_effectively_empty() {
                        continue;
                    }
                    if let Some(exit_code) = self.step(input, &mut err)? {
                        return Ok(exit_code);
                    }
                }
            }
        }
    }

    /// Parse and execute one logical input unit. Returns `Some(code)` when the
    /// shell should exit, `None` to continue the REPL loop.
    ///
    /// Callers guarantee `input` is not effectively empty.
    fn step(&mut self, input: Input, err: &mut dyn Write) -> miette::Result<Option<ExitCode>> {
        let stages = resolver::resolve(parser::parse(&input));
        match executor::execute_pipeline(stages, &mut self.ctx) {
            Ok(Some(exit_code)) => return Ok(Some(exit_code)),
            Ok(None) => {}
            Err(e) => {
                let fatal = e.is_fatal();
                writeln!(err, "{:?}", miette::Report::new(e)).into_diagnostic()?;
                if fatal {
                    return Ok(Some(ExitCode::FAILURE));
                }
            }
        }
        Ok(None)
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
        let home = self.home_dir.or(base.home_dir);
        let mut config = self.config.unwrap_or_default();
        if config.history_path.is_none()
            && let Some(ref h) = home
        {
            config.history_path = Some(h.join(".ferrish_history"));
        }
        let ctx = ShellCtx::with_config(home, self.cwd.unwrap_or(base.cwd), config);
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
        let config = ShellConfig {
            prompt: "CFG> ".to_string(),
            ..Default::default()
        };
        let shell = Shell::builder().with_config(config).build();
        assert_eq!(shell.ctx.config.prompt, "CFG> ");
    }

    #[test]
    fn with_prompt_overrides_with_config_prompt() {
        let config = ShellConfig {
            prompt: "CONFIG> ".to_string(),
            ..Default::default()
        };
        let shell = Shell::builder()
            .with_config(config)
            .with_prompt("OVERRIDE> ".to_string())
            .build();
        assert_eq!(shell.ctx.config.prompt, "OVERRIDE> ");
    }

    #[test]
    fn build_derives_history_path_from_home_dir() {
        use std::path::PathBuf;
        let home = PathBuf::from("/tmp/test-home");
        let shell = Shell::builder().with_home_dir(home.clone()).build();
        assert_eq!(
            shell.ctx.config.history_path,
            Some(home.join(".ferrish_history"))
        );
    }

    #[test]
    fn explicit_history_path_not_overridden() {
        use std::path::PathBuf;
        let custom = PathBuf::from("/tmp/my_history");
        let config = ShellConfig {
            history_path: Some(custom.clone()),
            ..Default::default()
        };
        let shell = Shell::builder()
            .with_home_dir(PathBuf::from("/tmp/home"))
            .with_config(config)
            .build();
        assert_eq!(shell.ctx.config.history_path, Some(custom));
    }
}
