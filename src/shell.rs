use std::io::{BufRead, Write};

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

impl Default for Shell {
    fn default() -> Self {
        Self::new()
    }
}

impl Shell {
    /// Create a shell initialized from the current process environment.
    ///
    /// Sets `history_path` to `~/.ferrish_history` when the home directory is
    /// available; leaves it `None` otherwise.
    pub fn new() -> Self {
        let ctx = ShellCtx::from_env();
        let config = ShellConfig {
            history_path: ctx.home_dir.as_deref().map(|h| h.join(".ferrish_history")),
            ..ctx.config
        };
        Shell {
            ctx: ShellCtx::with_config(ctx.home_dir, ctx.cwd, config),
        }
    }

    /// Create a shell with a custom configuration; env supplies home dir and cwd.
    pub fn with_config(config: ShellConfig) -> Self {
        let base = ShellCtx::from_env();
        Shell {
            ctx: ShellCtx::with_config(base.home_dir, base.cwd, config),
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn with_config_sets_prompt() {
        let config = ShellConfig {
            prompt: "CFG> ".to_string(),
            ..Default::default()
        };
        let shell = Shell::with_config(config);
        assert_eq!(shell.ctx.config.prompt, "CFG> ");
    }

    #[test]
    fn with_config_sets_continuation_prompt() {
        let config = ShellConfig {
            continuation_prompt: "... ".to_string(),
            ..Default::default()
        };
        let shell = Shell::with_config(config);
        assert_eq!(shell.ctx.config.continuation_prompt, "... ");
    }
}
