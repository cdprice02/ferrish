use std::borrow::Cow;
use std::io::BufRead;

use miette::IntoDiagnostic as _;
use reedline::{Prompt, PromptEditMode, PromptHistorySearch, Reedline, Signal};

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
}

/// Bridges a `&str` prompt into reedline's [`Prompt`] trait.
struct ShellPrompt<'a>(&'a str);

impl Prompt for ShellPrompt<'_> {
    fn render_prompt_left(&self) -> Cow<'_, str> {
        Cow::Borrowed("")
    }

    fn render_prompt_right(&self) -> Cow<'_, str> {
        Cow::Borrowed("")
    }

    fn render_prompt_indicator(&self, _: PromptEditMode) -> Cow<'_, str> {
        Cow::Borrowed(self.0)
    }

    fn render_prompt_multiline_indicator(&self) -> Cow<'_, str> {
        Cow::Borrowed("> ")
    }

    fn render_prompt_history_search_indicator(&self, _: PromptHistorySearch) -> Cow<'_, str> {
        Cow::Borrowed("search: ")
    }
}

/// Interactive line reader backed by reedline.
///
/// Displays prompts, supports line editing and history, and yields
/// [`LineInput::Interrupted`] on Ctrl+C. History is managed internally by
/// reedline and saved automatically on each successful line.
///
/// This struct is the composition point for reedline plugins — future
/// `Highlighter`, `Completer`, and `Hinter` implementations wire in here via
/// builder methods on [`Reedline::create`].
pub struct InteractiveReader {
    editor: Reedline,
}

impl InteractiveReader {
    /// Create a new interactive reader backed by a reedline editor.
    pub fn new() -> miette::Result<Self> {
        let editor = Reedline::create();
        // Future plugin hooks (all share the same lexer/parser as the REPL):
        //   .with_highlighter(Box::new(ShellHighlighter))
        //   .with_completer(Box::new(ShellCompleter))
        //   .with_hinter(Box::new(DefaultHinter::default()))
        Ok(Self { editor })
    }
}

impl LineReader for InteractiveReader {
    fn read_line(&mut self, prompt: &str) -> miette::Result<LineInput> {
        let p = ShellPrompt(prompt);
        match self.editor.read_line(&p).into_diagnostic()? {
            Signal::Success(s) => Ok(LineInput::Line(s.into_bytes())),
            Signal::CtrlC => Ok(LineInput::Interrupted),
            Signal::CtrlD => Ok(LineInput::Eof),
            // HostCommand and ExternalBreak are not issued in the default
            // configuration; treat them as interrupts so the REPL continues.
            _ => Ok(LineInput::Interrupted),
        }
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
