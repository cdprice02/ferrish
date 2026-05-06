use std::borrow::Cow;
use std::io::BufRead;

use miette::IntoDiagnostic as _;
use reedline::{
    Prompt, PromptEditMode, PromptHistorySearch, Reedline, Signal, ValidationResult, Validator,
};

use crate::ctx::ShellConfig;
use crate::lexer::{self, LexerState};

/// A single logical input unit returned by a line-reading source.
///
/// Both [`InteractiveReader`] and [`ScriptReader`] accumulate physical lines
/// internally and only return once a complete shell command is ready.
pub enum LineInput {
    /// A complete logical command, trailing `\n` included.
    Line(Vec<u8>),
    /// End of input (EOF or Ctrl+D).
    Eof,
    /// User interrupted the current input (Ctrl+C); only interactive readers yield this.
    Interrupted,
}

/// A source of complete logical shell commands, one per call.
///
/// Implementations handle their own line accumulation and continuation logic
/// internally. Callers receive complete, ready-to-parse commands.
pub trait LineReader {
    /// Read the next complete logical command.
    fn read_line(&mut self) -> miette::Result<LineInput>;
}

/// Determines whether accumulated input forms a complete shell command.
///
/// Implements [`reedline::Validator`] so it can be attached to [`Reedline`] for
/// interactive multiline support, and is used directly by [`ScriptReader`] for
/// the same completion checks. Both paths share the same grammar logic through
/// this struct rather than maintaining parallel implementations.
///
/// This is also the intended composition point for future features that need
/// to understand shell-grammar completeness: the syntax highlighter, the
/// language server, and other reedline plugins will all delegate here.
pub struct ShellValidator;

impl Validator for ShellValidator {
    fn validate(&self, line: &str) -> ValidationResult {
        let bytes = line.as_bytes();
        if lexer::needs_continuation(bytes) {
            return ValidationResult::Incomplete;
        }
        // Trailing backslash (outside quotes, since needs_continuation would
        // have fired for unclosed quotes) means backslash-newline continuation.
        if bytes.trim_ascii_end().ends_with(b"\\") {
            return ValidationResult::Incomplete;
        }
        ValidationResult::Complete
    }
}

/// Stores the prompt strings displayed by [`InteractiveReader`] and implements
/// reedline's [`Prompt`] trait so both the primary prompt and the continuation
/// prompt are drawn from the shell's configuration.
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
/// Accumulates physical lines internally via [`ShellValidator`] and returns
/// complete logical commands. Backslash-newline continuations are joined before
/// the command is returned. History is managed by reedline and saved
/// automatically on each successful line.
///
/// This struct is the composition point for reedline plugins — future
/// `Highlighter`, `Completer`, and `Hinter` implementations wire in here via
/// builder methods on [`Reedline::create`]. All such plugins should delegate
/// into the shared lexer/parser rather than forking grammar logic.
pub struct InteractiveReader {
    editor: Reedline,
    prompt: ShellPrompt,
}

impl InteractiveReader {
    /// Create a new interactive reader using prompts and config from `config`.
    pub fn new(config: &ShellConfig) -> miette::Result<Self> {
        let editor = Reedline::create().with_validator(Box::new(ShellValidator));
        // Future plugin hooks (all share the same lexer/parser as the REPL):
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
                let mut bytes = join_backslash_newlines(s.into_bytes());
                bytes.push(b'\n');
                Ok(LineInput::Line(bytes))
            }
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
/// Accumulates physical lines using the same quote-state and continuation
/// logic as [`ShellValidator`], returning complete logical commands. Prompts
/// are suppressed. Never yields [`LineInput::Interrupted`].
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
        let mut acc: Vec<u8> = Vec::new();
        let mut lex_state = LexerState::default();

        loop {
            let mut line = Vec::new();
            if self.reader.read_until(b'\n', &mut line).into_diagnostic()? == 0 {
                if acc.is_empty() {
                    return Ok(LineInput::Eof);
                }
                if !acc.ends_with(b"\n") {
                    acc.push(b'\n');
                }
                return Ok(LineInput::Line(acc));
            }
            if line.ends_with(b"\n") {
                line.pop();
            }

            lex_state.scan_bytes(&line);
            lex_state.scan_bytes(b"\n");

            if lex_state.needs_continuation() {
                acc.extend_from_slice(&line);
                acc.push(b'\n');
                continue;
            }

            if line.ends_with(b"\\") {
                acc.extend_from_slice(&line[..line.len() - 1]);
                continue;
            }

            acc.extend_from_slice(&line);
            acc.push(b'\n');
            return Ok(LineInput::Line(acc));
        }
    }
}

/// Strip backslash-newline pairs from `bytes`, joining continuation lines.
///
/// Operates on the raw buffer returned by reedline after [`ShellValidator`]
/// has confirmed the input is complete. At that point any remaining `\<LF>`
/// pairs are outside unclosed quotes (the validator would have returned
/// `Incomplete` otherwise), so a simple byte scan is correct.
fn join_backslash_newlines(bytes: Vec<u8>) -> Vec<u8> {
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'\\' && i + 1 < bytes.len() && bytes[i + 1] == b'\n' {
            i += 2;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    out
}
