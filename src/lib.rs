#![deny(missing_docs)]
//! ferrish — an early-stage shell implementation in Rust.

/// Shell command argument types.
pub mod arg;
/// CLI argument parsing (--help, --version).
pub mod cli;
/// Shell command variants (built-in, executable).
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
/// Raw input line with trimmed view and leading-offset pre-computed.
pub mod input;
/// Lexer: tokenizes raw input into [`lexer::Token`] values.
pub mod lexer;
/// Input parsing: groups tokens into an unresolved pipeline AST.
pub mod parser;
/// I/O redirection descriptors produced by the parser.
pub mod redirect;
/// Resolver: maps unresolved commands to [`CommandKind`] variants.
pub mod resolver;
/// The interactive REPL shell.
pub mod shell;

pub use arg::Arg;
pub use command::CommandKind;
pub use ctx::ShellConfig;
pub use shell::Shell;

/// Run the ferrish shell with standard I/O.
///
/// # Example
/// ```no_run
/// let result = ferrish::run();
/// ```
pub fn run() -> miette::Result<exit::ExitCode> {
    use clap::Parser;
    match cli::Cli::try_parse() {
        Ok(_) => Shell::builder().build().run_interactive(),
        Err(e) => e.exit(),
    }
}
