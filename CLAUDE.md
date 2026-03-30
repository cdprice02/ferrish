# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project

**ferrish** is an early-stage shell implementation in Rust. Correctness over performance; when a trade-off exists between the two, correctness wins.

## Commands

```bash
# Build
cargo build
cargo build --release

# Test (all)
cargo test

# Test (single integration test by name)
cargo test --test builtin test_name

# Test (single unit test)
cargo test -p ferrish test_name

# Lint (CI enforces no warnings)
cargo clippy --all-targets --all-features -- -D warnings
```

## Architecture

Input flows through: **Shell** → **Parser** → **Executor** → **BuiltIn | Executable**

- `src/shell.rs` — REPL loop; reads input, calls parser, calls executor, handles fatal vs. recoverable errors
- `src/parser.rs` — Splits input into command + args; resolves whether the command is a built-in, a PATH executable, or unrecognized
- `src/executor.rs` — Dispatches to built-in handlers or spawns external processes
- `src/command/builtin.rs` — Implementations of `exit`, `cd`, `echo`, `pwd`, `type`
- `src/command/executable.rs` — Wraps an external binary found on PATH
- `src/error.rs` — `ShellError` enum; distinguishes fatal errors (process spawn/wait failures) from recoverable ones (command not found, bad path)
- `src/io.rs` — `ShellIo` trait with `StandardIo` (real I/O) and `MockIo` (testing); this abstraction is what makes unit tests possible without spawning a process

## Testing Strategy

Two layers:

1. **Unit tests** — embedded in each module via `#[cfg(test)]`, use `MockIo` for I/O
2. **Integration tests** (`tests/`) — spawn the compiled `ferrish` binary as a subprocess via the `ShellTest` harness in `tests/harness.rs`

The `ShellTest` builder in `harness.rs` is the primary integration test interface. It creates an isolated `HOME` via `tempfile` and captures stdout/stderr. Prefer integration tests for anything user-visible.

## Key Conventions

- **File-based modules** — no `mod.rs`; each module is a standalone `.rs` file
- **Error reporting** — miette for user-facing errors with span context; `thiserror` for internal error types
- **REPL responsiveness** — lex/parse must complete in <100ms for typical input
- **State** — keep shell state (env vars, working dir) centralized and test scoping/isolation explicitly
