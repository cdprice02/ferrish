use std::borrow::Cow;
use std::io::BufRead;

use miette::IntoDiagnostic as _;
use reedline::{
    FileBackedHistory, Prompt, PromptEditMode, PromptHistorySearch, Reedline, Signal,
    ValidationResult, Validator,
};

use crate::ctx::ShellConfig;
use crate::lexer::Lexer;

/// A single logical input unit returned by a line-reading source.
///
/// Both [`InteractiveReader`] and [`ScriptReader`] accumulate physical lines
/// internally and only return once a complete shell command is ready.
pub enum LineInput {
    /// A finalized [`Lexer`] containing one complete logical command.
    Lexer(Lexer),
    /// End of input (EOF or Ctrl+D).
    Eof,
    /// User interrupted the current input (Ctrl+C); only interactive readers yield this.
    Interrupted,
}

/// A source of complete logical shell commands, one per call.
pub trait LineReader {
    /// Read the next complete logical command.
    fn read_line(&mut self) -> miette::Result<LineInput>;
}

/// Determines whether accumulated input forms a complete shell command.
///
/// Implements [`reedline::Validator`] so it can be attached to [`Reedline`]
/// for interactive multiline support. Delegates entirely to [`Lexer::needs_continuation`],
/// which covers both unclosed quotes and trailing unquoted backslashes.
pub struct ShellValidator;

impl Validator for ShellValidator {
    fn validate(&self, line: &str) -> ValidationResult {
        let mut lex = Lexer::new();
        lex.push(line.as_bytes());
        if lex.needs_continuation() {
            ValidationResult::Incomplete
        } else {
            ValidationResult::Complete
        }
    }
}

/// Stores the prompt strings displayed by [`InteractiveReader`].
struct ShellPrompt {
    prompt: String,
    cont_prompt: String,
}

impl Prompt for ShellPrompt {
    fn render_prompt_left(&self) -> Cow<'_, str> {
        Cow::Borrowed("")
    }

    fn render_prompt_right(&self) -> Cow<'_, str> {
        Cow::Borrowed("")
    }

    fn render_prompt_indicator(&self, _: PromptEditMode) -> Cow<'_, str> {
        Cow::Borrowed(&self.prompt)
    }

    fn render_prompt_multiline_indicator(&self) -> Cow<'_, str> {
        Cow::Borrowed(&self.cont_prompt)
    }

    fn render_prompt_history_search_indicator(&self, _: PromptHistorySearch) -> Cow<'_, str> {
        Cow::Borrowed("search: ")
    }
}

/// Interactive line reader backed by reedline.
///
/// reedline accumulates physical lines (with [`ShellValidator`] gating
/// multiline continuation). When the validator signals complete input, the
/// raw bytes are pushed directly into a [`Lexer`] — backslash-newline
/// continuation and quote handling are resolved by the Lexer itself.
pub struct InteractiveReader {
    editor: Reedline,
    prompt: ShellPrompt,
}

impl InteractiveReader {
    /// Create a new interactive reader using prompts and config from `config`.
    pub fn new(config: &ShellConfig) -> miette::Result<Self> {
        let mut editor = Reedline::create().with_validator(Box::new(ShellValidator));
        if let Some(ref path) = config.history_path {
            let history =
                FileBackedHistory::with_file(config.max_history, path.clone()).into_diagnostic()?;
            editor = editor.with_history(Box::new(history));
        }
        // Future plugin hooks:
        //   .with_highlighter(Box::new(ShellHighlighter))
        //   .with_completer(Box::new(ShellCompleter))
        //   .with_hinter(Box::new(DefaultHinter::default()))
        Ok(Self {
            editor,
            prompt: ShellPrompt {
                prompt: config.prompt.clone(),
                cont_prompt: config.continuation_prompt.clone(),
            },
        })
    }
}

impl LineReader for InteractiveReader {
    fn read_line(&mut self) -> miette::Result<LineInput> {
        match self.editor.read_line(&self.prompt).into_diagnostic()? {
            Signal::Success(s) => {
                let mut lexer = Lexer::new();
                lexer.push(s.as_bytes());
                lexer.push(b"\n");
                lexer.finalize();
                Ok(LineInput::Lexer(lexer))
            }
            Signal::CtrlC => Ok(LineInput::Interrupted),
            Signal::CtrlD => Ok(LineInput::Eof),
            _ => Ok(LineInput::Interrupted),
        }
    }
}

/// Non-interactive line reader backed by any [`BufRead`] source.
///
/// Feeds raw bytes into a [`Lexer`] line by line. Backslash-newline
/// continuation is handled here (the backslash is stripped and no newline is
/// pushed, so the lexer sees joined content). Quote continuation is handled
/// by the lexer itself via [`Lexer::needs_continuation`].
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
    fn read_line(&mut self) -> miette::Result<LineInput> {
        let mut lexer = Lexer::new();

        loop {
            let mut raw = Vec::new();
            if self.reader.read_until(b'\n', &mut raw).into_diagnostic()? == 0 {
                if lexer.raw_bytes().iter().all(|b| b.is_ascii_whitespace()) {
                    return Ok(LineInput::Eof);
                }
                lexer.finalize();
                return Ok(LineInput::Lexer(lexer));
            }

            if raw.ends_with(b"\n") {
                raw.pop();
            }
            if raw.ends_with(b"\r") {
                raw.pop();
            }

            if raw.ends_with(b"\\") && !lexer.is_in_single_quote() {
                // Backslash-newline: strip the backslash, push the rest, and
                // read the next physical line without adding a newline so the
                // lexer sees the two lines as one continuous word.
                // Guard: inside single quotes `\` is literal — POSIX says
                // single quotes suppress all special character interpretation.
                raw.pop();
                lexer.push(&raw);
                continue;
            }

            lexer.push(&raw);
            lexer.push(b"\n");

            if !lexer.needs_continuation() {
                lexer.finalize();
                return Ok(LineInput::Lexer(lexer));
            }
            // Open quote — read more physical lines.
        }
    }
}

#[cfg(test)]
mod tests {
    use reedline::ValidationResult;

    use super::*;

    // --- ShellValidator ---

    #[test]
    fn validator_complete_for_simple_command() {
        assert!(matches!(
            ShellValidator.validate("echo hello"),
            ValidationResult::Complete
        ));
    }

    #[test]
    fn validator_incomplete_for_unclosed_single_quote() {
        assert!(matches!(
            ShellValidator.validate("echo 'hello"),
            ValidationResult::Incomplete
        ));
    }

    #[test]
    fn validator_incomplete_for_unclosed_double_quote() {
        assert!(matches!(
            ShellValidator.validate("echo \"hello"),
            ValidationResult::Incomplete
        ));
    }

    #[test]
    fn validator_complete_after_multiline_single_quote() {
        assert!(matches!(
            ShellValidator.validate("echo 'hello\nworld'"),
            ValidationResult::Complete
        ));
    }

    #[test]
    fn validator_incomplete_for_trailing_backslash() {
        assert!(matches!(
            ShellValidator.validate("echo foo\\"),
            ValidationResult::Incomplete
        ));
    }

    #[test]
    fn validator_complete_after_backslash_continuation() {
        assert!(matches!(
            ShellValidator.validate("echo foo\\\nbar"),
            ValidationResult::Complete
        ));
    }
}
