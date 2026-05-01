use std::io::{BufRead, Write};
use std::path::PathBuf;

use miette::IntoDiagnostic as _;
use rustyline::error::ReadlineError;
use rustyline::DefaultEditor;

use crate::{
    ctx::{ShellConfig, ShellCtx},
    executor,
    exit::ExitCode,
    input::Input,
    lexer, parser, resolver,
};

/// A single physical line returned by a line-reading source.
enum LineInput {
    /// A line of bytes, trailing `\n` stripped — both sources normalize to this.
    Line(Vec<u8>),
    /// End of input (EOF or Ctrl+D).
    Eof,
    /// User interrupted the current input (Ctrl+C); only rustyline yields this.
    Interrupted,
}

/// Collect one logical input unit from `next_line`, handling quote and
/// backslash-newline continuation.
///
/// `next_line` is called with the prompt to display before each physical line.
/// The first call receives `prompt`; subsequent continuation reads receive
/// `cont_prompt`. Quote continuation is checked before backslash continuation
/// so that a `\` inside a quoted string is literal and does not trigger line
/// joining.
///
/// Returns `None` on EOF before any input is accumulated, or `Some(Input)`
/// once a complete logical line is available.
fn collect_logical_input(
    mut next_line: impl FnMut(&str) -> miette::Result<LineInput>,
    prompt: &str,
    cont_prompt: &str,
) -> miette::Result<Option<Input>> {
    let mut acc: Vec<u8> = Vec::new();
    let mut lex_state = lexer::LexerState::default();

    loop {
        let current = if acc.is_empty() { prompt } else { cont_prompt };

        let line = match next_line(current)? {
            LineInput::Eof => {
                if !acc.is_empty() && !acc.ends_with(b"\n") {
                    acc.push(b'\n');
                }
                return Ok(if acc.is_empty() {
                    None
                } else {
                    Some(Input::from_vec(acc))
                });
            }
            LineInput::Interrupted => {
                acc.clear();
                lex_state = lexer::LexerState::default();
                continue;
            }
            LineInput::Line(l) => l,
        };

        // Scan new bytes into the quote-state tracker — O(n) for this line only,
        // not the full accumulation. Semantics mirror LexerState::scan_bytes;
        // see lexer.rs for the escape and quote rules.
        lex_state.scan_bytes(&line);
        lex_state.scan_bytes(b"\n");

        if lex_state.needs_continuation() {
            acc.extend_from_slice(&line);
            acc.push(b'\n');
            continue;
        }

        // Backslash-newline: join this line to the next, stripping the `\`.
        if line.ends_with(b"\\") {
            acc.extend_from_slice(&line[..line.len() - 1]);
            continue;
        }

        acc.extend_from_slice(&line);
        acc.push(b'\n');
        return Ok(Some(Input::from_vec(acc)));
    }
}

/// The ferrish shell REPL.
pub struct Shell {
    ctx: ShellCtx,
}

impl Shell {
    /// Create a shell builder.
    pub fn builder() -> ShellBuilder {
        ShellBuilder::default()
    }

    /// Run the interactive REPL using rustyline for line editing.
    ///
    /// Reads from the terminal until `exit` is called or the user signals EOF
    /// (Ctrl+D). Ctrl+C abandons the current input and returns to the prompt.
    /// Diagnostics go to the process's real stderr.
    pub fn run_interactive(&mut self) -> miette::Result<ExitCode> {
        let mut editor = DefaultEditor::new().into_diagnostic()?;
        let mut err = std::io::stderr();
        loop {
            let input = match collect_logical_input(
                |prompt| match editor.readline(prompt) {
                    Ok(l) => Ok(LineInput::Line(l.into_bytes())),
                    Err(ReadlineError::Eof) => Ok(LineInput::Eof),
                    Err(ReadlineError::Interrupted) => Ok(LineInput::Interrupted),
                    Err(e) => Err(e).into_diagnostic(),
                },
                &self.ctx.config.prompt,
                &self.ctx.config.continuation_prompt,
            )? {
                None => return Ok(ExitCode::SUCCESS),
                Some(input) => input,
            };

            if input.is_effectively_empty() {
                continue;
            }

            // Add the complete logical command to history (strip trailing newline).
            let raw = input.raw_bytes();
            let entry = String::from_utf8_lossy(raw.strip_suffix(b"\n").unwrap_or(raw));
            let _ = editor.add_history_entry(entry.as_ref());

            if let Some(exit_code) = self.step(input, &mut err)? {
                return Ok(exit_code);
            }
        }
    }

    /// Run the shell loop from a [`BufRead`] source.
    ///
    /// This is the entry point for script / batch mode; interactive use goes
    /// through [`Shell::run_interactive`]. Prompts and diagnostics go to the
    /// process's real stdout/stderr.
    pub fn run_script(&mut self, reader: &mut dyn BufRead) -> miette::Result<ExitCode> {
        let mut out = std::io::stdout();
        let mut err = std::io::stderr();
        loop {
            let input = match collect_logical_input(
                |prompt| {
                    out.write_all(prompt.as_bytes()).into_diagnostic()?;
                    out.flush().into_diagnostic()?;
                    let mut line = Vec::new();
                    if reader.read_until(b'\n', &mut line).into_diagnostic()? == 0 {
                        return Ok(LineInput::Eof);
                    }
                    if line.ends_with(b"\n") {
                        line.pop();
                    }
                    Ok(LineInput::Line(line))
                },
                &self.ctx.config.prompt,
                &self.ctx.config.continuation_prompt,
            )? {
                None => return Ok(ExitCode::SUCCESS),
                Some(input) => input,
            };

            if input.is_effectively_empty() {
                continue;
            }

            if let Some(exit_code) = self.step(input, &mut err)? {
                return Ok(exit_code);
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
}
