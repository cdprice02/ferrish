use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;

use miette::IntoDiagnostic as _;

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
        let cont_prompt = self.ctx.config.continuation_prompt.clone();
        loop {
            out.write_all(self.ctx.config.prompt.as_bytes())
                .into_diagnostic()?;
            out.flush().into_diagnostic()?;

            let raw = match collect_input(reader, &mut out, &cont_prompt)? {
                None => return Ok(ExitCode::SUCCESS),
                Some(raw) => raw,
            };

            let input = Input::from_vec(raw);
            if input.is_effectively_empty() {
                continue;
            }

            let unresolved = match parser::parse(&input) {
                Ok(r) => r,
                Err(e) => {
                    writeln!(err, "{:?}", miette::Report::new(e)).into_diagnostic()?;
                    continue;
                }
            };
            let pipeline = match resolver::resolve(unresolved) {
                Ok(r) => r,
                Err(e) => {
                    writeln!(err, "{:?}", miette::Report::new(e)).into_diagnostic()?;
                    continue;
                }
            };
            match executor::execute_pipeline(pipeline, &mut self.ctx) {
                Ok(Some(exit_code)) => return Ok(exit_code),
                Ok(None) => {}
                Err(e) => {
                    let fatal = e.is_fatal();
                    writeln!(err, "{:?}", miette::Report::new(e)).into_diagnostic()?;
                    if fatal {
                        return Ok(ExitCode::FAILURE);
                    }
                }
            }
        }
    }
}

/// Read one logical input line from `reader`, handling line continuations.
///
/// A physical line ending in `\` is joined to the next line (the backslash and
/// newline are both dropped). A physical line whose accumulated content has an
/// unclosed quote signals that more input is needed; the newline is kept in the
/// accumulator so the quoted string receives its literal newline character.
///
/// In both cases a `cont_prompt` is written to `out` and another physical line
/// is read. Returns `None` on EOF before any input is accumulated.
fn collect_input(
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

        let without_newline = line.strip_suffix(b"\n").unwrap_or(&line);
        if without_newline.ends_with(b"\\") {
            acc.extend_from_slice(&without_newline[..without_newline.len() - 1]);
            out.write_all(cont_prompt.as_bytes()).into_diagnostic()?;
            out.flush().into_diagnostic()?;
            continue;
        }

        acc.extend_from_slice(&line);

        if let Err(ShellError::UnclosedQuote { .. }) = lexer::lex(&Input::new(&acc)) {
            out.write_all(cont_prompt.as_bytes()).into_diagnostic()?;
            out.flush().into_diagnostic()?;
            continue;
        }

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
