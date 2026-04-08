# Ferrish Copilot Instructions

Project-specific guidance for **ferrish**, a modern Rust-powered shell focused on safety, performance, and clarity.

## Core Principles

These principles guide all development decisions:

1. **Safety by default** - Avoid footguns, undefined behavior, and surprising side effects
2. **Explicit over implicit** - Favor clear, readable behavior instead of clever but opaque magic
3. **Predictable semantics** - The same input should always produce the same result
4. **Composable, but understandable** - Pipelining and composition remain powerful without becoming unreadable
5. **Fast enough, then correct** - Performance matters, but never at the cost of correctness

**Design Philosophy**: When a trade-off exists between performance and correctness, correctness wins.

## Project Structure

Modules use **file-based organization** (not mod.rs):

- `src/shell.rs` — REPL loop; reads input, calls parser, calls executor, handles fatal vs. recoverable errors
- `src/parser.rs` — Splits input into command + args; resolves whether the command is a built-in, a PATH executable, or unrecognized
- `src/executor.rs` — Dispatches to built-in handlers or spawns external processes
- `src/command/builtin.rs` — Implementations of `exit`, `cd`, `echo`, `pwd`, `type`
- `src/command/executable.rs` — Wraps an external binary found on PATH
- `src/io.rs` — `ShellIo` trait with `StandardIo` (real I/O) and `MockIo` (testing)
- `src/ctx.rs` — Shell context and state
- `src/error.rs` — `ShellError` enum; distinguishes fatal from recoverable errors
- `src/arg.rs` — Argument parsing and representation

## Key Development Notes

**Error Reporting**: Use **miette** for user-facing errors with span context and suggestions.

**Testing**: Shell-bound tests are essential. Beyond unit and integration tests, spawn ferrish as a subprocess to verify actual behavior (I/O, pipes, semantics, signal handling).

**REPL Responsiveness**: Lexing/parsing should complete in <100ms for typical commands. Users expect immediate feedback.

**State Management**: Keep state (variables, functions, environment) centralized. Test variable scoping and isolation thoroughly.

## Reference

- **Project README**: [README.md](../README.md)
