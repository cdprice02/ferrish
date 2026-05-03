use std::io::BufRead;

use miette::IntoDiagnostic as _;
use rustyline::error::ReadlineError;
use rustyline::DefaultEditor;

/// A single physical line returned by a line-reading source.
pub enum LineInput {
    /// A line of bytes, trailing `\n` stripped.
    Line(Vec<u8>),
    /// End of input (EOF or Ctrl+D).
    Eof,
    /// User interrupted the current input (Ctrl+C); only interactive readers yield this.
    Interrupted,
}

/// A source of physical input lines, one per call.
///
/// The prompt string passed to [`read_line`] is advisory — interactive
/// implementations display it; non-interactive ones ignore it.
pub trait LineReader {
    /// Read the next physical line, displaying `prompt` if appropriate.
    fn read_line(&mut self, prompt: &str) -> miette::Result<LineInput>;

    /// Record a completed logical command in history. No-op by default.
    fn add_history(&mut self, _entry: &str) {}
}

/// Interactive line reader backed by rustyline.
///
/// Displays prompts, supports line editing and history, and yields
/// [`LineInput::Interrupted`] on Ctrl+C.
pub struct InteractiveReader {
    editor: DefaultEditor,
}

impl InteractiveReader {
    /// Create a new interactive reader backed by a rustyline editor.
    pub fn new() -> miette::Result<Self> {
        Ok(Self {
            editor: DefaultEditor::new().into_diagnostic()?,
        })
    }
}

impl LineReader for InteractiveReader {
    fn read_line(&mut self, prompt: &str) -> miette::Result<LineInput> {
        match self.editor.readline(prompt) {
            Ok(l) => Ok(LineInput::Line(l.into_bytes())),
            Err(ReadlineError::Eof) => Ok(LineInput::Eof),
            Err(ReadlineError::Interrupted) => Ok(LineInput::Interrupted),
            Err(e) => Err(e).into_diagnostic(),
        }
    }

    fn add_history(&mut self, entry: &str) {
        let _ = self.editor.add_history_entry(entry);
    }
}

/// Non-interactive line reader backed by any [`BufRead`] source.
///
/// Prompts are suppressed. Never yields [`LineInput::Interrupted`].
pub struct ScriptReader<'a> {
    reader: &'a mut dyn BufRead,
}

impl<'a> ScriptReader<'a> {
    /// Create a new script reader wrapping `reader`.
    pub fn new(reader: &'a mut dyn BufRead) -> Self {
        Self { reader }
    }
}

impl LineReader for ScriptReader<'_> {
    fn read_line(&mut self, _prompt: &str) -> miette::Result<LineInput> {
        let mut line = Vec::new();
        if self.reader.read_until(b'\n', &mut line).into_diagnostic()? == 0 {
            return Ok(LineInput::Eof);
        }
        if line.ends_with(b"\n") {
            line.pop();
        }
        Ok(LineInput::Line(line))
    }
}
