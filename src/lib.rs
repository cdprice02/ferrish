#![deny(missing_docs)]
//! ferrish — an early-stage shell implementation in Rust.

/// Shell command argument types.
pub mod arg;
/// Shell command variants (built-in, executable, unrecognized).
pub mod command;
/// Shell context and configuration.
pub mod ctx;
/// Environment variable and filesystem path helpers.
pub mod env;
/// Error types for shell execution.
pub mod error;
/// Command dispatch and execution logic.
pub mod executor;
/// Process exit code type.
pub mod exit;
/// Filesystem path utilities.
pub mod fs;
/// I/O abstraction layer for the shell.
pub mod io;
/// Input parsing: splits raw bytes into a command and its arguments.
pub mod parser;
/// Stdout redirection descriptors produced by the parser.
pub mod redirect;
/// The interactive REPL shell.
pub mod shell;

pub use arg::Arg;
pub use command::Command;
pub use ctx::ShellConfig;
pub use shell::Shell;

/// Run the ferrish shell with standard I/O
///
/// This is the primary way to start ferrish. It sets up stdin/stdout/stderr
/// and runs the interactive REPL.
///
/// # Example
/// ```no_run
/// let result = ferrish::run();
/// ```
///
/// For testing or custom I/O, use [`Shell::builder()`] instead:
/// ```no_run
/// use ferrish::Shell;
/// use ferrish::io::MockIo;
///
/// let io = MockIo::from_lines(&["echo test", "exit"]);
/// let mut shell = Shell::builder().with_io(io);
/// let exit_code = shell.run()?;
/// # Ok::<(), anyhow::Error>(())
/// ```
pub fn run() -> anyhow::Result<exit::ExitCode> {
    Shell::<crate::io::StandardIo>::builder().with_std_io().run()
}
