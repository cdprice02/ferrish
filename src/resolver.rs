use std::str::FromStr;

use is_executable::IsExecutable;

use crate::arg::{Arg, Args};
use crate::command::builtin::{BuiltInCommand, BuiltInName};
use crate::command::executable::ExecutableCommand;
use crate::command::CommandKind;
use crate::env::get_path_dirs;
use crate::error::{ShellError, ShellResult};
use crate::parser::{Parser, UnresolvedStage};
use crate::redirect::{StderrRedirection, StdoutRedirection};

/// A resolved command: its kind (built-in or executable) plus the original word arg.
#[derive(Debug)]
pub struct ResolvedCommand {
    /// The resolved command kind.
    pub kind: CommandKind,
    /// The original command word as it appeared in the input.
    pub word: Arg,
}

/// One stage of a pipeline after command resolution.
#[derive(Debug)]
pub struct ResolvedStage {
    /// The resolved command.
    pub command: ResolvedCommand,
    /// Arguments to pass to the command.
    pub args: Args,
    /// Optional stdout redirection for this stage.
    pub stdout_redirect: Option<StdoutRedirection>,
    /// Optional stderr redirection for this stage.
    pub stderr_redirect: Option<StderrRedirection>,
}

/// Lazy resolver iterator produced by [`resolve`].
///
/// Wraps a [`Parser`] and resolves each [`UnresolvedStage`] to a
/// [`ResolvedStage`] on demand. Errors from parsing or resolution are
/// forwarded as `Err` items; the first error terminates the iterator.
pub struct Resolver<'a> {
    parser: Parser<'a>,
}

impl<'a> Iterator for Resolver<'a> {
    type Item = ShellResult<ResolvedStage>;

    fn next(&mut self) -> Option<Self::Item> {
        self.parser.next().map(|r| r.and_then(resolve_stage))
    }
}

/// Wrap `parser` in a lazy resolver that resolves each stage on demand.
pub fn resolve(parser: Parser<'_>) -> Resolver<'_> {
    Resolver { parser }
}

fn resolve_stage(stage: UnresolvedStage) -> ShellResult<ResolvedStage> {
    let word = stage.command.word;
    let kind = resolve_command(&word)?;
    Ok(ResolvedStage {
        command: ResolvedCommand { kind, word },
        args: stage.args,
        stdout_redirect: stage.stdout_redirect,
        stderr_redirect: stage.stderr_redirect,
    })
}

/// Look up `word` as a command without failing on not-found.
///
/// Returns `Some(kind)` if the word resolves to a built-in or PATH executable,
/// `None` if it is empty, non-ASCII, or not found anywhere on PATH.
pub fn lookup(word: &Arg) -> Option<CommandKind> {
    let bytes = word.as_bytes();

    if bytes.is_empty() || !bytes.is_ascii() {
        return None;
    }

    let name = std::str::from_utf8(bytes).expect("checked ASCII above");

    if let Ok(builtin_name) = BuiltInName::from_str(name) {
        return Some(CommandKind::BuiltIn(BuiltInCommand::new(builtin_name)));
    }

    #[cfg(windows)]
    let extensions = if name.contains('.') {
        // Name already has an extension — probe only the bare name, matching
        // cmd.exe behavior (PATHEXT iteration is skipped when an extension is present).
        vec![]
    } else {
        path_extensions()
    };

    for dir in get_path_dirs() {
        let candidate = dir.join(name);
        if candidate.is_executable() {
            return Some(CommandKind::Executable(ExecutableCommand::new(candidate)));
        }
        #[cfg(windows)]
        for ext in &extensions {
            let candidate_ext = dir.join(format!("{name}{ext}"));
            if candidate_ext.is_executable() {
                return Some(CommandKind::Executable(ExecutableCommand::new(
                    candidate_ext,
                )));
            }
        }
    }

    None
}

/// Returns the executable extensions to probe on Windows.
///
/// Reads `PATHEXT` from the environment; falls back to the standard set if unset.
#[cfg(windows)]
fn path_extensions() -> Vec<String> {
    let raw = std::env::var("PATHEXT").unwrap_or_else(|_| ".COM;.EXE;.BAT;.CMD".to_string());
    parse_pathext(&raw)
}

/// Parses a `PATHEXT`-style semicolon-delimited string into a lowercased extension list.
#[cfg(windows)]
fn parse_pathext(raw: &str) -> Vec<String> {
    raw.split(';')
        .filter(|e| !e.is_empty())
        .map(|e| e.to_ascii_lowercase())
        .collect()
}

fn resolve_command(word: &Arg) -> ShellResult<CommandKind> {
    lookup(word).ok_or_else(|| ShellError::CommandNotFound {
        name: String::from_utf8_lossy(word.as_bytes()).into_owned(),
        span: word.span(),
        src: word.src(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::Input;
    use crate::parser;

    fn resolve_raw(raw: &[u8]) -> ShellResult<Vec<ResolvedStage>> {
        let input = Input::new(raw);
        resolve(parser::parse(&input)).collect()
    }

    fn resolve_single(raw: &[u8]) -> ResolvedStage {
        resolve_raw(raw).unwrap().remove(0)
    }

    #[test]
    fn unknown_command_returns_not_found() {
        let err = resolve_raw(b"some_non_existent_command_xyz").unwrap_err();
        assert!(
            matches!(err, ShellError::CommandNotFound { .. }),
            "got: {err:?}"
        );
    }

    #[test]
    fn non_ascii_command_returns_not_found() {
        let err = resolve_raw(b"\xff").unwrap_err();
        assert!(matches!(err, ShellError::CommandNotFound { .. }));
    }

    #[test]
    fn builtin_echo_resolves() {
        let stage = resolve_single(b"echo hello");
        assert!(matches!(stage.command.kind, CommandKind::BuiltIn(_)));
    }

    #[test]
    fn command_not_found_name_and_span() {
        let err = resolve_raw(b"nonexistent_cmd_xyz").unwrap_err();
        if let ShellError::CommandNotFound { name, span, .. } = err {
            assert_eq!(name, "nonexistent_cmd_xyz");
            assert_eq!(span.offset(), 0);
            assert_eq!(span.len(), 19);
        } else {
            panic!("expected CommandNotFound");
        }
    }

    #[test]
    fn fails_fast_on_first_unknown_in_pipeline() {
        // Second stage is unknown; error names it, not the first stage.
        let err = resolve_raw(b"echo foo | unknown_cmd_xyz").unwrap_err();
        if let ShellError::CommandNotFound { name, .. } = err {
            assert_eq!(name, "unknown_cmd_xyz");
        } else {
            panic!("expected CommandNotFound");
        }
    }

    #[cfg(windows)]
    #[test]
    fn parse_pathext_fallback_extensions() {
        let exts = parse_pathext(".COM;.EXE;.BAT;.CMD");
        for expected in &[".com", ".exe", ".bat", ".cmd"] {
            assert!(
                exts.iter().any(|e| e == expected),
                "missing {expected}: {exts:?}"
            );
        }
    }

    #[cfg(windows)]
    #[test]
    fn parse_pathext_lowercases_extensions() {
        assert_eq!(parse_pathext(".EXE;.PS1"), vec![".exe", ".ps1"]);
    }

    #[cfg(windows)]
    #[test]
    fn parse_pathext_filters_empty_segments() {
        assert_eq!(parse_pathext(".EXE;;.BAT;"), vec![".exe", ".bat"]);
    }
}
