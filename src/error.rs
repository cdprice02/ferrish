use std::process::ExitStatus;

use crate::arg::{Arg, QuoteStyle};
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
    },

    /// A required operand was not provided.
    #[error("missing operand")]
    #[diagnostic(code(ferrish::missing_operand))]
    MissingOperand,

    // --- File system ---
    /// The path does not exist on the filesystem.
    #[error("no such file or directory: {arg}")]
    #[diagnostic(code(ferrish::fs::not_found))]
    FileNotFound {
        /// The argument that referred to the missing path.
        arg: Arg,
    },

    /// The path exists but is a directory where a regular file was expected.
    #[error("is a directory: {arg}")]
    #[diagnostic(code(ferrish::fs::is_a_directory))]
    IsADirectory {
        /// The argument that referred to the directory.
        arg: Arg,
    },

    /// The path exists but is a regular file where a directory was expected.
    #[error("not a directory: {arg}")]
    #[diagnostic(code(ferrish::fs::not_a_directory))]
    NotADirectory {
        /// The argument that referred to the non-directory path.
        arg: Arg,
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

    // --- I/O ---
    /// An I/O error propagated from the underlying stream.
    #[error(transparent)]
    #[diagnostic(code(ferrish::io))]
    Io(#[from] std::io::Error),

    // --- Parse ---
    /// An opening quote was never closed before end of input.
    #[error("unclosed {style:?} quote")]
    #[diagnostic(
        code(ferrish::parse::unclosed_quote),
        help("add the matching quote to close this token")
    )]
    UnclosedQuote {
        /// The kind of quote that was opened but never closed.
        style: QuoteStyle,
        /// Byte offset of the opening quote character in the trimmed input line passed to the parser.
        #[label("quote opened here")]
        span: SourceSpan,
    },
}

impl PartialEq for ShellError {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::CommandNotFound { name: l }, Self::CommandNotFound { name: r }) => l == r,
            (Self::FileNotFound { arg: l_arg }, Self::FileNotFound { arg: r_arg }) => {
                l_arg == r_arg
            }
            (Self::IsADirectory { arg: l_arg }, Self::IsADirectory { arg: r_arg }) => {
                l_arg == r_arg
            }
            (Self::NotADirectory { arg: l_arg }, Self::NotADirectory { arg: r_arg }) => {
                l_arg == r_arg
            }
            (Self::NonZeroExit(l0), Self::NonZeroExit(r0)) => l0 == r0,
            (Self::ExecutionFailed(l0), Self::ExecutionFailed(r0)) => {
                l0.raw_os_error() == r0.raw_os_error()
            }
            (Self::Io(l0), Self::Io(r0)) => l0.raw_os_error() == r0.raw_os_error(),
            (
                Self::UnclosedQuote {
                    style: l_style,
                    span: l_span,
                },
                Self::UnclosedQuote {
                    style: r_style,
                    span: r_span,
                },
            ) => l_style == r_style && l_span == r_span,
            _ => core::mem::discriminant(self) == core::mem::discriminant(other),
        }
    }
}

impl ShellError {
    /// Check if this error is fatal (should stop execution)
    pub fn is_fatal(&self) -> bool {
        matches!(self, ShellError::ExecutionFailed(_))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_display_command_not_found() {
        let err = ShellError::CommandNotFound { name: "foo".to_string() };
        assert_eq!(err.to_string(), "foo: command not found");
    }

    #[test]
    fn test_display_missing_operand() {
        let err = ShellError::MissingOperand;
        assert_eq!(err.to_string(), "missing operand");
    }

    #[test]
    fn test_display_file_not_found() {
        let err = ShellError::FileNotFound {
            arg: Arg::from("/path/to/file"),
        };
        assert_eq!(err.to_string(), "no such file or directory: /path/to/file");
    }

    #[test]
    fn test_display_is_a_directory() {
        let err = ShellError::IsADirectory {
            arg: Arg::from("/some/dir"),
        };
        assert_eq!(err.to_string(), "is a directory: /some/dir");
    }

    #[test]
    fn test_display_not_a_directory() {
        let err = ShellError::NotADirectory {
            arg: Arg::from("/some/file"),
        };
        assert_eq!(err.to_string(), "not a directory: /some/file");
    }

    #[test]
    fn test_display_unclosed_quote_single() {
        let err = ShellError::UnclosedQuote {
            style: QuoteStyle::Single,
            span: SourceSpan::from((0, 1)),
        };
        assert_eq!(err.to_string(), "unclosed Single quote");
    }

    #[test]
    fn test_display_unclosed_quote_double() {
        let err = ShellError::UnclosedQuote {
            style: QuoteStyle::Double,
            span: SourceSpan::from((5, 1)),
        };
        assert_eq!(err.to_string(), "unclosed Double quote");
    }

    #[test]
    fn test_is_fatal_execution_failed() {
        let err = ShellError::ExecutionFailed(std::io::Error::last_os_error());
        assert!(err.is_fatal());
    }

    #[test]
    fn test_is_fatal_command_not_found() {
        let err = ShellError::CommandNotFound { name: "foo".to_string() };
        assert!(!err.is_fatal());
    }

    #[test]
    fn test_is_fatal_file_not_found() {
        let err = ShellError::FileNotFound {
            arg: Arg::from("test"),
        };
        assert!(!err.is_fatal());
    }

    #[test]
    fn test_is_fatal_unclosed_quote() {
        let err = ShellError::UnclosedQuote {
            style: QuoteStyle::Double,
            span: SourceSpan::from((0, 1)),
        };
        assert!(!err.is_fatal());
    }

    #[test]
    fn test_equality_file_not_found() {
        let err1 = ShellError::FileNotFound {
            arg: Arg::from("file1"),
        };
        let err2 = ShellError::FileNotFound {
            arg: Arg::from("file1"),
        };
        assert_eq!(err1, err2);
    }

    #[test]
    fn test_inequality_file_not_found() {
        let err1 = ShellError::FileNotFound {
            arg: Arg::from("file1"),
        };
        let err2 = ShellError::FileNotFound {
            arg: Arg::from("file2"),
        };
        assert_ne!(err1, err2);
    }

    #[test]
    fn test_equality_different_variant() {
        let err1 = ShellError::CommandNotFound { name: "foo".to_string() };
        let err2 = ShellError::MissingOperand;
        assert_ne!(err1, err2);
    }

    #[test]
    fn test_equality_nonzero_exit() {
        #[cfg(unix)]
        {
            use std::os::unix::process::ExitStatusExt;
            let status1 = ExitStatusExt::from_raw(42 << 8);
            let status2 = ExitStatusExt::from_raw(42 << 8);
            let e1 = ShellError::NonZeroExit(status1);
            let e2 = ShellError::NonZeroExit(status2);
            assert_eq!(e1, e2);
        }
        #[cfg(windows)]
        {
            use std::os::windows::process::ExitStatusExt;
            let status1 = ExitStatusExt::from_raw(42);
            let status2 = ExitStatusExt::from_raw(42);
            let e1 = ShellError::NonZeroExit(status1);
            let e2 = ShellError::NonZeroExit(status2);
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
            style: QuoteStyle::Single,
            span: SourceSpan::from((3, 1)),
        };
        let e2 = ShellError::UnclosedQuote {
            style: QuoteStyle::Single,
            span: SourceSpan::from((3, 1)),
        };
        assert_eq!(e1, e2);
    }

    #[test]
    fn test_inequality_unclosed_quote_different_style() {
        let e1 = ShellError::UnclosedQuote {
            style: QuoteStyle::Single,
            span: SourceSpan::from((3, 1)),
        };
        let e2 = ShellError::UnclosedQuote {
            style: QuoteStyle::Double,
            span: SourceSpan::from((3, 1)),
        };
        assert_ne!(e1, e2);
    }
}
