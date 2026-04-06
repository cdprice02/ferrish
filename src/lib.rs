pub mod arg;
pub mod command;
pub mod ctx;
pub mod env;
pub mod error;
pub mod executor;
pub mod exit;
pub mod fs;
pub mod io;
pub mod parser;
pub mod shell;

pub use arg::Arg;
pub use command::Command;
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
/// let result = shell.run();
/// ```
pub fn run() -> anyhow::Result<exit::ExitCode> {
    Shell::<crate::io::StandardIo>::builder().with_std_io().run()
}
