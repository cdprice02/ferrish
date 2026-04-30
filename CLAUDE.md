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

- `src/shell.rs` — REPL loop; `Shell::run()` drives the interactive loop via rustyline; `Shell::run_repl()` is the future script-mode entry point (BufRead-based); `step()` is the shared parse-and-execute helper
- `src/parser.rs` — Splits input into command + args; resolves whether the command is a built-in, a PATH executable, or unrecognized
- `src/executor.rs` — Dispatches to built-in handlers or spawns external processes
- `src/command/builtin.rs` — Implementations of `exit`, `cd`, `echo`, `pwd`, `type`
- `src/command/executable.rs` — Wraps an external binary found on PATH
- `src/error.rs` — `ShellError` enum; distinguishes fatal errors (process spawn/wait failures) from recoverable ones (command not found, bad path)
- `src/ctx.rs` — `ShellCtx` (runtime state: cwd, home dir, config) and `ShellConfig` (prompt, history settings)
- `src/arg.rs` — `Arg` enum representing a parsed command argument (currently a `Literal` byte-sequence variant)
- `src/env.rs` — PATH resolution, home directory lookup, and current-directory helpers
- `src/exit.rs` — `ExitCode` newtype wrapping `u8`; constants `SUCCESS`/`FAILURE`, conversions to `i32` and `std::process::ExitCode`
- `src/fs.rs` — path resolution: expands `~`, resolves relative paths against cwd, soft-canonicalizes without requiring existence

## Testing Strategy

Two layers:

1. **Unit tests** — embedded in each module via `#[cfg(test)]`
2. **Integration tests** (`tests/`) — spawn the `ferrish` binary as a subprocess via `ShellHarness` in `tests/common/mod.rs`, feed commands via stdin, and capture stdout/stderr/exit code

`ShellHarness` is the primary integration test interface. It runs in an isolated `tempfile` directory. Prefer integration tests for anything user-visible.

Note: integration tests pipe stdin to the binary (non-TTY). The interactive rustyline path (`Shell::run`) suppresses the prompt in non-TTY mode, which is correct shell behavior — tests should not assert on prompt output.

## Key Conventions

- **File-based modules** — no `mod.rs`; each module is a standalone `.rs` file
- **Error reporting** — `thiserror` derives `Error` for `ShellError` in `src/error.rs`; `anyhow` for `Shell::run()` return type and error context
- **REPL responsiveness** — lex/parse must complete in <100ms for typical input
- **State** — keep shell state (env vars, working dir) centralized and test scoping/isolation explicitly
