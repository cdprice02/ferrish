use std::io::{BufRead, Write};
use std::path::PathBuf;

use miette::IntoDiagnostic as _;
use rustyline::DefaultEditor;
use rustyline::error::ReadlineError;

use crate::{
    ctx::{ShellConfig, ShellCtx},
    error::ShellError,
    executor,
    exit::ExitCode,
    input::Input,
    lexer, parser, resolver,
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

    /// Run the interactive REPL using rustyline for line editing.
    ///
    /// Reads from the terminal until `exit` is called or the user signals EOF
    /// (Ctrl+D). Ctrl+C abandons the current input and returns to the prompt.
    /// Diagnostics go to the process's real stderr.
    pub fn run(&mut self) -> miette::Result<ExitCode> {
        let mut rl = DefaultEditor::new().into_diagnostic()?;
        let mut err = std::io::stderr();
        loop {
            let raw = match self.collect_input(&mut rl)? {
                None => return Ok(ExitCode::SUCCESS),
                Some(raw) => raw,
            };
            if let Some(exit_code) = self.step(raw, &mut err)? {
                return Ok(exit_code);
            }
        }
    }

    /// Run the shell loop from a [`BufRead`] source.
    ///
    /// This is the entry point for future script / batch mode; interactive use
    /// goes through [`Shell::run`]. Prompts and diagnostics go to the process's
    /// real stdout/stderr.
    pub fn run_repl(&mut self, reader: &mut dyn BufRead) -> miette::Result<ExitCode> {
        let mut out = std::io::stdout();
        let mut err = std::io::stderr();
        let prompt = self.ctx.config.prompt.clone();
        let cont_prompt = self.ctx.config.continuation_prompt.clone();
        loop {
            out.write_all(prompt.as_bytes()).into_diagnostic()?;
            out.flush().into_diagnostic()?;

            let raw = match collect_input_from_reader(reader, &mut out, &cont_prompt)? {
                None => return Ok(ExitCode::SUCCESS),
                Some(raw) => raw,
            };
            if let Some(exit_code) = self.step(raw, &mut err)? {
                return Ok(exit_code);
            }
        }
    }

    /// Collect one logical input line from rustyline, handling quote and
    /// backslash-newline continuation.
    fn collect_input(&self, rl: &mut DefaultEditor) -> miette::Result<Option<Vec<u8>>> {
        let prompt = &self.ctx.config.prompt;
        let cont_prompt = &self.ctx.config.continuation_prompt;
        let mut acc: Vec<u8> = Vec::new();

        loop {
            let current = if acc.is_empty() {
                prompt.as_str()
            } else {
                cont_prompt.as_str()
            };

            let line = match rl.readline(current) {
                Ok(l) => l,
                Err(ReadlineError::Eof) => {
                    return Ok(if acc.is_empty() { None } else { Some(acc) });
                }
                Err(ReadlineError::Interrupted) => {
                    // Ctrl+C: abandon current input and restart prompt.
                    acc.clear();
                    continue;
                }
                Err(e) => return Err(miette::miette!("{e}")),
            };

            let line_bytes = line.as_bytes();
            let mut candidate = acc.clone();
            candidate.extend_from_slice(line_bytes);
            candidate.push(b'\n');

            // Check quote state before checking backslash continuation — a `\`
            // inside a quoted string is literal and must not trigger line joining.
            if let Err(ShellError::UnclosedQuote { .. }) = lexer::lex(&Input::new(&candidate)) {
                acc = candidate;
                continue;
            }

            // Backslash-newline: join this line to the next, stripping the `\`.
            if line_bytes.ends_with(b"\\") {
                acc.extend_from_slice(&line_bytes[..line_bytes.len() - 1]);
                continue;
            }

            let _ = rl.add_history_entry(line.as_str());
            acc.extend_from_slice(line_bytes);
            acc.push(b'\n');
            return Ok(Some(acc));
        }
    }

    /// Parse and execute one logical input unit. Returns `Some(code)` when the
    /// shell should exit, `None` to continue the REPL loop.
    fn step(&mut self, raw: Vec<u8>, err: &mut dyn Write) -> miette::Result<Option<ExitCode>> {
        let input = Input::from_vec(raw);
        if input.is_effectively_empty() {
            return Ok(None);
        }

        let unresolved = match parser::parse(&input) {
            Ok(r) => r,
            Err(e) => {
                writeln!(err, "{:?}", miette::Report::new(e)).into_diagnostic()?;
                return Ok(None);
            }
        };
        let pipeline = match resolver::resolve(unresolved) {
            Ok(r) => r,
            Err(e) => {
                writeln!(err, "{:?}", miette::Report::new(e)).into_diagnostic()?;
                return Ok(None);
            }
        };
        match executor::execute_pipeline(pipeline, &mut self.ctx) {
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

/// Read one logical input line from `reader`, handling quote and
/// backslash-newline continuation.
///
/// Quote continuation is checked before backslash continuation so that a `\`
/// inside a quoted string (where it is literal) does not trigger line joining.
/// Backslash-newline joining is only applied when a real `\n` delimiter was
/// read; an EOF-terminated chunk ending in `\` is kept as-is.
///
/// Returns `None` on EOF before any input is accumulated.
fn collect_input_from_reader(
    reader: &mut dyn BufRead,
    out: &mut dyn Write,
    cont_prompt: &str,
) -> miette::Result<Option<Vec<u8>>> {
    let mut acc: Vec<u8> = Vec::new();

    loop {
        let mut line = Vec::new();
        let n = reader.read_until(b'\n', &mut line).into_diagnostic()?;
        if n == 0 {
            return Ok(if acc.is_empty() { None } else { Some(acc) });
        }

        let mut candidate = acc.clone();
        candidate.extend_from_slice(&line);
        if let Err(ShellError::UnclosedQuote { .. }) = lexer::lex(&Input::new(&candidate)) {
            acc = candidate;
            out.write_all(cont_prompt.as_bytes()).into_diagnostic()?;
            out.flush().into_diagnostic()?;
            continue;
        }

        if let Some(without_newline) = line.strip_suffix(b"\n")
            && without_newline.ends_with(b"\\")
        {
            acc.extend_from_slice(&without_newline[..without_newline.len() - 1]);
            out.write_all(cont_prompt.as_bytes()).into_diagnostic()?;
            out.flush().into_diagnostic()?;
            continue;
        }

        acc.extend_from_slice(&line);
        return Ok(Some(acc));
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
