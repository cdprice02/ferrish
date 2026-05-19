use std::io::{BufRead, Write};

use bytes::Bytes;
use miette::IntoDiagnostic as _;

use crate::{
    ctx::{ShellConfig, ShellCtx},
    executor,
    exit::ExitCode,
    lexer::Lexer,
    line_reader::{InteractiveReader, LineInput, LineReader, ScriptReader},
    parser::Parser,
    resolver,
};

/// Byte-backed source code for miette diagnostics.
///
/// Keeps `SourceSpan` byte offsets accurate even when input contains non-UTF-8
/// bytes: `String::from_utf8_lossy` replaces each invalid byte with U+FFFD
/// (3 bytes), shifting offsets for every span that follows.
struct ByteSource {
    name: String,
    bytes: Bytes,
}

impl ByteSource {
    fn new(name: impl Into<String>, bytes: Bytes) -> Self {
        ByteSource {
            name: name.into(),
            bytes,
        }
    }
}

impl miette::SourceCode for ByteSource {
    fn read_span<'a>(
        &'a self,
        span: &miette::SourceSpan,
        context_lines_before: usize,
        context_lines_after: usize,
    ) -> Result<Box<dyn miette::SpanContents<'a> + 'a>, miette::MietteError> {
        // Delegate to &[u8]'s SourceCode impl — no UTF-8 round-trip, so
        // SourceSpan byte offsets stay accurate regardless of input encoding.
        let inner =
            self.bytes
                .as_ref()
                .read_span(span, context_lines_before, context_lines_after)?;
        Ok(Box::new(miette::MietteSpanContents::new_named(
            self.name.clone(),
            inner.data(),
            *inner.span(),
            inner.line(),
            inner.column(),
            inner.line_count(),
        )))
    }
}

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
    pub fn run_interactive(&mut self) -> miette::Result<ExitCode> {
        let mut reader = InteractiveReader::new(&self.ctx.config)?;
        self.run_loop(&mut reader)
    }

    /// Run the shell loop from a [`BufRead`] source.
    pub fn run_script(&mut self, source: &mut dyn BufRead) -> miette::Result<ExitCode> {
        let mut reader = ScriptReader::new(source);
        self.run_loop(&mut reader)
    }

    /// Shared REPL loop driven by any [`LineReader`].
    fn run_loop(&mut self, reader: &mut dyn LineReader) -> miette::Result<ExitCode> {
        let mut err = std::io::stderr();
        loop {
            match reader.read_line()? {
                LineInput::Eof => return Ok(ExitCode::SUCCESS),
                LineInput::Interrupted => continue,
                LineInput::Lexer(lexer) => {
                    if lexer.raw_bytes().iter().all(|b| b.is_ascii_whitespace()) {
                        continue;
                    }
                    if let Some(exit_code) = self.step(lexer, &mut err)? {
                        return Ok(exit_code);
                    }
                }
            }
        }
    }

    /// Parse and execute one logical input unit. Returns `Some(code)` when the
    /// shell should exit, `None` to continue the REPL loop.
    fn step(&mut self, lexer: Lexer, err: &mut dyn Write) -> miette::Result<Option<ExitCode>> {
        let raw = lexer.raw_bytes_shared();
        let stages = resolver::resolve(Parser::new(lexer));
        match executor::execute_pipeline(stages, &mut self.ctx) {
            Ok(Some(exit_code)) => return Ok(Some(exit_code)),
            Ok(None) => {}
            Err(e) => {
                let fatal = e.is_fatal();
                let src = ByteSource::new("<input>", raw);
                writeln!(err, "{:?}", miette::Report::new(e).with_source_code(src))
                    .into_diagnostic()?;
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
    fn new_derives_history_path_from_home() {
        let shell = Shell::new();
        if shell.ctx.home_dir.is_some() {
            let hp = shell
                .ctx
                .config
                .history_path
                .expect("history_path should be set");
            assert_eq!(hp.file_name().unwrap(), ".ferrish_history");
        }
    }

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
