use std::borrow::Cow;
use std::io::BufRead;
use std::sync::{Arc, Mutex};

use miette::IntoDiagnostic as _;
use reedline::{
    FileBackedHistory, Prompt, PromptEditMode, PromptHistorySearch, Reedline, Signal,
    ValidationResult, Validator,
};

use crate::ctx::ShellConfig;
use crate::scanner::Scanner;
use crate::tokenizer::Tokenizer;

/// A single logical input unit returned by a line-reading source.
///
/// Both [`InteractiveReader`] and [`ScriptReader`] accumulate physical lines
/// internally and only return once a complete shell command is ready.
pub enum LineInput {
    /// A finalized [`Tokenizer`] containing one complete logical command.
    Command(Tokenizer),
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

/// Shared scanner cache between [`CachingValidator`] and [`InteractiveReader`].
///
/// reedline calls `validate()` on every Enter key press before signaling
/// `Success`. The cache stores the [`Scanner`] built during validation so
/// `read_line` can retrieve it rather than building a second one.
type ScannerCache = Arc<Mutex<Option<Scanner>>>;

/// reedline [`Validator`] that also caches the [`Scanner`] it builds.
///
/// Because `validate()` receives exactly the same buffer string that
/// reedline will pass to `Signal::Success`, the cached [`Scanner`] is
/// already populated with the right bytes when `read_line` drains it.
struct CachingValidator {
    cache: ScannerCache,
}

impl Validator for CachingValidator {
    fn validate(&self, line: &str) -> ValidationResult {
        let mut sc = Scanner::new();
        sc.push(line.as_bytes());
        let needs_more = sc.needs_continuation();
        *self.cache.lock().unwrap() = Some(sc);
        if needs_more {
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
/// reedline accumulates physical lines (with [`CachingValidator`] gating
/// multiline continuation). When the validator signals complete input, the
/// cached [`Scanner`] is retrieved and finalized — no second byte scan.
pub struct InteractiveReader {
    editor: Reedline,
    prompt: ShellPrompt,
    cache: ScannerCache,
}

impl InteractiveReader {
    /// Create a new interactive reader using prompts and config from `config`.
    pub fn new(config: &ShellConfig) -> miette::Result<Self> {
        let cache: ScannerCache = Arc::new(Mutex::new(None));
        let validator = CachingValidator {
            cache: Arc::clone(&cache),
        };
        let mut editor = Reedline::create().with_validator(Box::new(validator));
        if let Some(ref path) = config.history_path {
            let history =
                FileBackedHistory::with_file(config.max_history, path.clone()).into_diagnostic()?;
            editor = editor.with_history(Box::new(history));
        }
        Ok(Self {
            editor,
            prompt: ShellPrompt {
                prompt: config.prompt.clone(),
                cont_prompt: config.continuation_prompt.clone(),
            },
            cache,
        })
    }
}

impl LineReader for InteractiveReader {
    fn read_line(&mut self) -> miette::Result<LineInput> {
        match self.editor.read_line(&self.prompt).into_diagnostic()? {
            Signal::Success(_) => {
                // Drain the scanner the validator already built — zero second scan.
                let sc = self
                    .cache
                    .lock()
                    .unwrap()
                    .take()
                    .expect("validator always populates cache before Success");
                Ok(LineInput::Command(sc.finalize()))
            }
            Signal::CtrlC => Ok(LineInput::Interrupted),
            Signal::CtrlD => Ok(LineInput::Eof),
            _ => Ok(LineInput::Interrupted),
        }
    }
}

/// Non-interactive line reader backed by any [`BufRead`] source.
///
/// Feeds raw bytes into a [`Scanner`] line by line. Backslash-newline
/// continuation and quote continuation are both handled by the scanner
/// itself via [`Scanner::needs_continuation`].
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
        let mut sc = Scanner::new();

        loop {
            let mut raw = Vec::new();
            if self.reader.read_until(b'\n', &mut raw).into_diagnostic()? == 0 {
                if sc.raw_bytes().iter().all(|b| b.is_ascii_whitespace()) {
                    return Ok(LineInput::Eof);
                }
                return Ok(LineInput::Command(sc.finalize()));
            }

            if raw.ends_with(b"\n") {
                raw.pop();
            }
            if raw.ends_with(b"\r") {
                raw.pop();
            }

            let trailing_backslash = raw.ends_with(b"\\");

            sc.push(&raw);
            sc.push(b"\n");

            if trailing_backslash && !sc.needs_continuation() {
                // Unquoted \<LF> continuation: scanner consumed both chars,
                // needs_continuation() is now false — but we must read another
                // line to complete the logical command.
                continue;
            }

            if !sc.needs_continuation() {
                return Ok(LineInput::Command(sc.finalize()));
            }
            // Inside an open quote — read more physical lines.
        }
    }
}

#[cfg(test)]
mod tests {
    use reedline::ValidationResult;

    use super::*;

    // --- CachingValidator ---

    #[test]
    fn validator_complete_for_simple_command() {
        let cache = Arc::new(Mutex::new(None));
        let v = CachingValidator {
            cache: Arc::clone(&cache),
        };
        assert!(matches!(
            v.validate("echo hello"),
            ValidationResult::Complete
        ));
        assert!(
            cache.lock().unwrap().is_some(),
            "cache populated on validate"
        );
    }

    #[test]
    fn validator_incomplete_for_unclosed_single_quote() {
        let cache = Arc::new(Mutex::new(None));
        let v = CachingValidator { cache };
        assert!(matches!(
            v.validate("echo 'hello"),
            ValidationResult::Incomplete
        ));
    }

    #[test]
    fn validator_incomplete_for_unclosed_double_quote() {
        let cache = Arc::new(Mutex::new(None));
        let v = CachingValidator { cache };
        assert!(matches!(
            v.validate("echo \"hello"),
            ValidationResult::Incomplete
        ));
    }

    #[test]
    fn validator_complete_after_multiline_single_quote() {
        let cache = Arc::new(Mutex::new(None));
        let v = CachingValidator { cache };
        assert!(matches!(
            v.validate("echo 'hello\nworld'"),
            ValidationResult::Complete
        ));
    }

    #[test]
    fn validator_incomplete_for_trailing_backslash() {
        let cache = Arc::new(Mutex::new(None));
        let v = CachingValidator { cache };
        assert!(matches!(
            v.validate("echo foo\\"),
            ValidationResult::Incomplete
        ));
    }

    #[test]
    fn validator_complete_after_backslash_continuation() {
        let cache = Arc::new(Mutex::new(None));
        let v = CachingValidator { cache };
        assert!(matches!(
            v.validate("echo foo\\\nbar"),
            ValidationResult::Complete
        ));
    }
}
