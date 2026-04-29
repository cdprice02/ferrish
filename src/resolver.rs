use std::str::FromStr;

use is_executable::IsExecutable;

use crate::arg::{Arg, Args};
use crate::command::builtin::{BuiltInCommand, BuiltInName};
use crate::command::executable::ExecutableCommand;
use crate::command::CommandKind;
use crate::env::get_path_dirs;
use crate::error::{ShellError, ShellResult};
use crate::input::InputSource;
use crate::parser::{UnresolvedPipeline, UnresolvedStage};
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

/// A pipeline with all commands resolved to built-ins or executables.
#[derive(Debug)]
pub struct ResolvedPipeline {
    /// The stages of the pipeline, one per `|`-separated segment.
    pub stages: Vec<ResolvedStage>,
    /// The raw input source this pipeline was parsed from.
    pub src: InputSource,
}

/// Resolve all commands in `pipeline` to [`CommandKind::BuiltIn`] or [`CommandKind::Executable`].
///
/// Fails fast on the first unrecognised command with [`ShellError::CommandNotFound`].
pub fn resolve(pipeline: UnresolvedPipeline) -> ShellResult<ResolvedPipeline> {
    let stages = pipeline
        .stages
        .into_iter()
        .map(resolve_stage)
        .collect::<ShellResult<Vec<_>>>()?;

    Ok(ResolvedPipeline {
        stages,
        src: pipeline.src,
    })
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
    let extensions = path_extensions();

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

/// Returns the list of executable extensions to probe on Windows.
///
/// Reads `PATHEXT` from the environment; falls back to the standard set if unset.
/// Extensions are lowercased to keep extension handling consistent.
#[cfg(windows)]
fn path_extensions() -> Vec<String> {
    std::env::var("PATHEXT")
        .unwrap_or_else(|_| ".COM;.EXE;.BAT;.CMD".to_string())
        .split(';')
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

    fn resolve_raw(raw: &[u8]) -> ShellResult<ResolvedPipeline> {
        let input = Input::new(raw);
        let pipeline = parser::parse(&input)?;
        resolve(pipeline)
    }

    fn resolve_single(raw: &[u8]) -> ResolvedStage {
        resolve_raw(raw).unwrap().stages.remove(0)
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

    // Serializes PATHEXT mutations so tests don't race each other under `cargo test`'s
    // parallel runner. Poisoned locks are recovered so a panicking test doesn't block
    // subsequent ones.
    #[cfg(windows)]
    static PATHEXT_MUTEX: std::sync::Mutex<()> = std::sync::Mutex::new(());

    #[cfg(windows)]
    fn with_pathext<F: FnOnce()>(set_to: Option<&str>, f: F) {
        let _guard = PATHEXT_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        let saved = std::env::var("PATHEXT").ok();
        match set_to {
            Some(v) => std::env::set_var("PATHEXT", v),
            None => std::env::remove_var("PATHEXT"),
        }
        f();
        match saved {
            Some(v) => std::env::set_var("PATHEXT", v),
            None => std::env::remove_var("PATHEXT"),
        }
    }

    #[cfg(windows)]
    #[test]
    fn path_extensions_fallback_when_unset() {
        with_pathext(None, || {
            let exts = path_extensions();
            for expected in &[".com", ".exe", ".bat", ".cmd"] {
                assert!(
                    exts.iter().any(|e| e == expected),
                    "missing {expected} in fallback PATHEXT: {exts:?}"
                );
            }
        });
    }

    #[cfg(windows)]
    #[test]
    fn path_extensions_reads_env() {
        with_pathext(Some(".EXE;.PS1"), || {
            assert_eq!(path_extensions(), vec![".exe", ".ps1"]);
        });
    }

    #[cfg(windows)]
    #[test]
    fn path_extensions_filters_empty_segments() {
        with_pathext(Some(".EXE;;.BAT;"), || {
            assert_eq!(path_extensions(), vec![".exe", ".bat"]);
        });
    }
}
