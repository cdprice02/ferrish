use std::process::ExitStatus;

use crate::command::CommandKind;
use crate::input::InputSource;
use crate::lexer::QuoteKind;
use miette::{Diagnostic, SourceSpan};
use thiserror::Error;

/// Convenience result type for shell execution results
pub type ShellResult<T> = Result<T, ShellError>;

/// Errors that can occur during command execution
///
/// These are domain errors that the shell can handle gracefully.
/// They're displayed to the user but don't crash the shell.
#[derive(Error, Debug, Diagnostic)]
pub enum ShellError {
    // --- Command-level ---
    /// The requested command could not be found as a built-in or on `PATH`.
    #[error("{name}: command not found")]
    #[diagnostic(code(ferrish::command_not_found))]
    CommandNotFound {
        /// The command name that was not found.
        name: String,
        /// Byte offset of the command token in the raw input line.
        #[label("command not found")]
        span: SourceSpan,
        /// The raw input line this error was produced from.
        #[source_code]
        src: InputSource,
    },

    /// A required operand was not provided.
    #[error("missing operand")]
    #[diagnostic(code(ferrish::missing_operand))]
    MissingOperand,

    /// The home directory could not be determined.
    #[error("home directory not set")]
    #[diagnostic(code(ferrish::home_not_set))]
    HomeNotSet,

    // --- File system ---
    /// The path does not exist on the filesystem.
    #[error("no such file or directory: {name}")]
    #[diagnostic(code(ferrish::fs::not_found))]
    FileNotFound {
        /// The path that was not found.
        name: String,
        /// Byte offset of the offending argument in the raw input line.
        #[label("no such file or directory")]
        span: SourceSpan,
        /// The raw input line this error was produced from.
        #[source_code]
        src: InputSource,
    },

    /// The path exists but is a directory where a regular file was expected.
    #[error("is a directory: {name}")]
    #[diagnostic(code(ferrish::fs::is_a_directory))]
    IsADirectory {
        /// The path that resolved to a directory.
        name: String,
        /// Byte offset of the offending argument in the raw input line.
        #[label("is a directory")]
        span: SourceSpan,
        /// The raw input line this error was produced from.
        #[source_code]
        src: InputSource,
    },

    /// The path exists but is a regular file where a directory was expected.
    #[error("not a directory: {name}")]
    #[diagnostic(code(ferrish::fs::not_a_directory))]
    NotADirectory {
        /// The path that was not a directory.
        name: String,
        /// Byte offset of the offending argument in the raw input line.
        #[label("not a directory")]
        span: SourceSpan,
        /// The raw input line this error was produced from.
        #[source_code]
        src: InputSource,
    },

    // --- Process execution ---
    /// The child process exited with a non-zero status code.
    #[error("exited with status {0}")]
    #[diagnostic(code(ferrish::exec::non_zero_exit))]
    NonZeroExit(ExitStatus),

    /// The shell failed to spawn or wait on the child process.
    #[error("failed to execute: {0}")]
    #[diagnostic(code(ferrish::exec::failed))]
    ExecutionFailed(#[source] std::io::Error),

    // --- Command context ---
    /// Wraps any execution error with the command that caused it.
    ///
    /// Applied at the dispatch boundary so individual error variants stay lean.
    #[error("{command}: {source}")]
    #[diagnostic(code(ferrish::exec::command_error))]
    InCommand {
        /// The command that was executing when the error occurred.
        command: CommandKind,
        /// The underlying shell error.
        #[source]
        #[diagnostic_source]
        source: Box<ShellError>,
    },

    // --- I/O ---
    /// An I/O error propagated from the underlying stream.
    #[error(transparent)]
    #[diagnostic(code(ferrish::io))]
    Io(#[from] std::io::Error),

    // --- Parse ---
    /// An opening quote was never closed before end of input.
    #[error("unclosed {style} quote")]
    #[diagnostic(
        code(ferrish::parse::unclosed_quote),
        help("add the matching quote to close this token")
    )]
    UnclosedQuote {
        /// The kind of quote that was opened but never closed.
        style: QuoteKind,
        /// Absolute byte offset of the opening quote character in the raw input line.
        #[label("quote opened here")]
        span: SourceSpan,
        /// The raw input line this error was produced from.
        #[source_code]
        src: InputSource,
    },

    /// A pipeline segment (between `|` operators) is empty.
    #[error("syntax error near unexpected token `|'")]
    #[diagnostic(
        code(ferrish::parse::empty_pipeline_segment),
        help("each `|` must be preceded and followed by a command")
    )]
    EmptyPipelineSegment {
        /// Absolute byte offset of the `|` operator that produced an empty segment.
        #[label("unexpected token")]
        span: SourceSpan,
        /// The raw input line this error was produced from.
        #[source_code]
        src: InputSource,
    },
}

impl std::borrow::Borrow<dyn miette::Diagnostic> for Box<ShellError> {
    fn borrow(&self) -> &(dyn miette::Diagnostic + 'static) {
        self.as_ref()
    }
}

impl PartialEq for ShellError {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::CommandNotFound { name: l, .. }, Self::CommandNotFound { name: r, .. }) => {
                l == r
            }
            (Self::FileNotFound { name: la, .. }, Self::FileNotFound { name: ra, .. }) => la == ra,
            (Self::IsADirectory { name: la, .. }, Self::IsADirectory { name: ra, .. }) => la == ra,
            (Self::NotADirectory { name: la, .. }, Self::NotADirectory { name: ra, .. }) => {
                la == ra
            }
            (Self::NonZeroExit(l0), Self::NonZeroExit(r0)) => l0 == r0,
            (Self::ExecutionFailed(l0), Self::ExecutionFailed(r0)) => {
                l0.raw_os_error() == r0.raw_os_error()
            }
            (Self::Io(l0), Self::Io(r0)) => l0.raw_os_error() == r0.raw_os_error(),
            (
                Self::InCommand {
                    command: lc,
                    source: ls,
                },
                Self::InCommand {
                    command: rc,
                    source: rs,
                },
            ) => lc == rc && ls == rs,
            (
                Self::UnclosedQuote {
                    style: l_style,
                    span: l_span,
                    ..
                },
                Self::UnclosedQuote {
                    style: r_style,
                    span: r_span,
                    ..
                },
            ) => l_style == r_style && l_span == r_span,
            (
                Self::EmptyPipelineSegment { span: l_span, .. },
                Self::EmptyPipelineSegment { span: r_span, .. },
            ) => l_span == r_span,
            _ => core::mem::discriminant(self) == core::mem::discriminant(other),
        }
    }
}

impl ShellError {
    /// Check if this error is fatal (should stop execution)
    pub fn is_fatal(&self) -> bool {
        match self {
            ShellError::ExecutionFailed(_) => true,
            ShellError::InCommand { source, .. } => source.is_fatal(),
            _ => false,
        }
    }

    /// Extract the [`ExitStatus`] if this error (or its wrapped source) is a
    /// [`ShellError::NonZeroExit`].  Useful for callers that need to inspect
    /// stage exit codes without pattern-matching through the `InCommand`
    /// wrapper at each call site.
    pub fn as_exit_status(&self) -> Option<ExitStatus> {
        match self {
            ShellError::NonZeroExit(s) => Some(*s),
            ShellError::InCommand { source, .. } => source.as_exit_status(),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::Input;

    fn dummy_src() -> InputSource {
        Input::new(b"").as_source()
    }

    fn dummy_span() -> SourceSpan {
        SourceSpan::from((0, 0))
    }

    #[test]
    fn test_display_command_not_found() {
        let err = ShellError::CommandNotFound {
            name: "foo".to_string(),
            span: dummy_span(),
            src: dummy_src(),
        };
        assert_eq!(err.to_string(), "foo: command not found");
    }

    #[test]
    fn test_display_missing_operand() {
        let err = ShellError::MissingOperand;
        assert_eq!(err.to_string(), "missing operand");
    }

    #[test]
    fn test_display_home_not_set() {
        let err = ShellError::HomeNotSet;
        assert_eq!(err.to_string(), "home directory not set");
    }

    #[test]
    fn test_display_file_not_found() {
        let err = ShellError::FileNotFound {
            name: "/path/to/file".to_string(),
            span: dummy_span(),
            src: dummy_src(),
        };
        assert_eq!(err.to_string(), "no such file or directory: /path/to/file");
    }

    #[test]
    fn test_display_is_a_directory() {
        let err = ShellError::IsADirectory {
            name: "/some/dir".to_string(),
            span: dummy_span(),
            src: dummy_src(),
        };
        assert_eq!(err.to_string(), "is a directory: /some/dir");
    }

    #[test]
    fn test_display_not_a_directory() {
        let err = ShellError::NotADirectory {
            name: "/some/file".to_string(),
            span: dummy_span(),
            src: dummy_src(),
        };
        assert_eq!(err.to_string(), "not a directory: /some/file");
    }

    #[test]
    fn test_in_command_prepends_command_name() {
        let inner = ShellError::FileNotFound {
            name: "/no/such".to_string(),
            span: dummy_span(),
            src: dummy_src(),
        };
        let err = ShellError::InCommand {
            command: CommandKind::BuiltIn(crate::command::builtin::BuiltInCommand::new(
                crate::command::builtin::BuiltInName::Cd,
            )),
            source: Box::new(inner),
        };
        assert_eq!(err.to_string(), "cd: no such file or directory: /no/such");
    }

    #[test]
    fn test_in_command_fatal_delegates_to_source() {
        let inner = ShellError::ExecutionFailed(std::io::Error::last_os_error());
        let err = ShellError::InCommand {
            command: CommandKind::BuiltIn(crate::command::builtin::BuiltInCommand::new(
                crate::command::builtin::BuiltInName::Echo,
            )),
            source: Box::new(inner),
        };
        assert!(err.is_fatal());
    }

    #[test]
    fn test_display_unclosed_quote_single() {
        let err = ShellError::UnclosedQuote {
            style: QuoteKind::Single,
            span: SourceSpan::from((0, 1)),
            src: dummy_src(),
        };
        assert_eq!(err.to_string(), "unclosed single quote");
    }

    #[test]
    fn test_display_unclosed_quote_double() {
        let err = ShellError::UnclosedQuote {
            style: QuoteKind::Double,
            span: SourceSpan::from((5, 1)),
            src: dummy_src(),
        };
        assert_eq!(err.to_string(), "unclosed double quote");
    }

    #[cfg(unix)]
    #[test]
    fn test_as_exit_status_non_zero_exit() {
        use std::os::unix::process::ExitStatusExt;
        let status = ExitStatus::from_raw(1 << 8);
        let err = ShellError::NonZeroExit(status);
        assert_eq!(err.as_exit_status(), Some(status));
    }

    #[cfg(unix)]
    #[test]
    fn test_as_exit_status_in_command_wraps_non_zero() {
        use std::os::unix::process::ExitStatusExt;
        let status = ExitStatus::from_raw(2 << 8);
        let inner = ShellError::NonZeroExit(status);
        let err = ShellError::InCommand {
            command: CommandKind::BuiltIn(crate::command::builtin::BuiltInCommand::new(
                crate::command::builtin::BuiltInName::Echo,
            )),
            source: Box::new(inner),
        };
        assert_eq!(err.as_exit_status(), Some(status));
    }

    #[test]
    fn test_as_exit_status_other_variants_return_none() {
        let err = ShellError::CommandNotFound {
            name: "foo".to_string(),
            span: dummy_span(),
            src: dummy_src(),
        };
        assert_eq!(err.as_exit_status(), None);
    }

    #[test]
    fn test_is_fatal_execution_failed() {
        let err = ShellError::ExecutionFailed(std::io::Error::last_os_error());
        assert!(err.is_fatal());
    }

    #[test]
    fn test_is_fatal_command_not_found() {
        let err = ShellError::CommandNotFound {
            name: "foo".to_string(),
            span: dummy_span(),
            src: dummy_src(),
        };
        assert!(!err.is_fatal());
    }

    #[test]
    fn test_is_fatal_file_not_found() {
        let err = ShellError::FileNotFound {
            name: "test".to_string(),
            span: dummy_span(),
            src: dummy_src(),
        };
        assert!(!err.is_fatal());
    }

    #[test]
    fn test_is_fatal_unclosed_quote() {
        let err = ShellError::UnclosedQuote {
            style: QuoteKind::Double,
            span: SourceSpan::from((0, 1)),
            src: dummy_src(),
        };
        assert!(!err.is_fatal());
    }

    #[test]
    fn test_equality_file_not_found() {
        let err1 = ShellError::FileNotFound {
            name: "file1".to_string(),
            span: dummy_span(),
            src: dummy_src(),
        };
        let err2 = ShellError::FileNotFound {
            name: "file1".to_string(),
            span: dummy_span(),
            src: dummy_src(),
        };
        assert_eq!(err1, err2);
    }

    #[test]
    fn test_inequality_file_not_found() {
        let err1 = ShellError::FileNotFound {
            name: "file1".to_string(),
            span: dummy_span(),
            src: dummy_src(),
        };
        let err2 = ShellError::FileNotFound {
            name: "file2".to_string(),
            span: dummy_span(),
            src: dummy_src(),
        };
        assert_ne!(err1, err2);
    }

    #[test]
    fn test_equality_different_variant() {
        let err1 = ShellError::CommandNotFound {
            name: "foo".to_string(),
            span: dummy_span(),
            src: dummy_src(),
        };
        let err2 = ShellError::MissingOperand;
        assert_ne!(err1, err2);
    }

    #[test]
    fn test_equality_nonzero_exit() {
        #[cfg(unix)]
        {
            use std::os::unix::process::ExitStatusExt;
            let e1 = ShellError::NonZeroExit(ExitStatusExt::from_raw(42 << 8));
            let e2 = ShellError::NonZeroExit(ExitStatusExt::from_raw(42 << 8));
            assert_eq!(e1, e2);
        }
        #[cfg(windows)]
        {
            use std::os::windows::process::ExitStatusExt;
            let e1 = ShellError::NonZeroExit(ExitStatusExt::from_raw(42));
            let e2 = ShellError::NonZeroExit(ExitStatusExt::from_raw(42));
            assert_eq!(e1, e2);
        }
    }

    #[test]
    fn test_equality_execution_failed_and_io() {
        let s1 = ShellError::ExecutionFailed(std::io::Error::from_raw_os_error(2));
        let s2 = ShellError::ExecutionFailed(std::io::Error::from_raw_os_error(2));
        assert_eq!(s1, s2);

        let i1 = ShellError::Io(std::io::Error::from_raw_os_error(4));
        let i2 = ShellError::Io(std::io::Error::from_raw_os_error(4));
        assert_eq!(i1, i2);
    }

    #[test]
    fn test_equality_unclosed_quote() {
        let e1 = ShellError::UnclosedQuote {
            style: QuoteKind::Single,
            span: SourceSpan::from((3, 1)),
            src: dummy_src(),
        };
        let e2 = ShellError::UnclosedQuote {
            style: QuoteKind::Single,
            span: SourceSpan::from((3, 1)),
            src: dummy_src(),
        };
        assert_eq!(e1, e2);
    }

    #[test]
    fn test_inequality_unclosed_quote_different_style() {
        let e1 = ShellError::UnclosedQuote {
            style: QuoteKind::Single,
            span: SourceSpan::from((3, 1)),
            src: dummy_src(),
        };
        let e2 = ShellError::UnclosedQuote {
            style: QuoteKind::Double,
            span: SourceSpan::from((3, 1)),
            src: dummy_src(),
        };
        assert_ne!(e1, e2);
    }
}
