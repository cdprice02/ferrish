# Ferrish Copilot Instructions

This file provides project-specific guidance for the **ferrish** shell project. It supplements the account-level instructions found at `$HOME/.copilot/`.

## Using These Instructions

**Account-level guidance** (reusable across all your projects):
- Start here: `$HOME/.copilot/copilot-instructions.md` - Entry point
- `$HOME/.copilot/copilot-instructions-general.md` - Cross-project standards
- `$HOME/.copilot/copilot-instructions-rust.md` - Rust best practices
- `$HOME/.copilot/copilot-instructions-python.md` - Python best practices

**Project-specific guidance** (this directory):
- `.github/copilot-instructions.md` - Project entry point (this file)
- `.github/instructions/rust.instructions.md` - Ferrish-specific Rust guidance
- `.github/instructions/testing.instructions.md` - Ferrish-specific testing patterns
- `.github/instructions/shell.instructions.md` - Shell-specific development notes

---

## Project Overview

**ferrish** is a modern, Rust-powered shell focused on **safety, performance, and clarity**. It is not intended as a drop-in replacement for bash/zsh, but rather an experiment in better shell foundations built with modern principles.

### Core Principles

1. **Safety by default** - Avoid footguns, undefined behavior, and surprising side effects
2. **Explicit over implicit** - Favor clear, readable behavior instead of clever but opaque magic
3. **Predictable semantics** - The same input should always produce the same result
4. **Composable, but understandable** - Pipelining and composition remain powerful without becoming unreadable
5. **Fast enough, then correct** - Performance matters, but never at the cost of correctness

---

## Project Structure

### Modules (file-based, not mod.rs)
- `src/lexer.rs` - Tokenization
- `src/parser.rs` - AST construction
- `src/evaluator.rs` - Execution
- `src/repl.rs` - Interactive shell loop
- `src/builtins.rs` - Built-in commands
- `src/shell_io.rs` - I/O management

See account-level `copilot-instructions-rust.md` for details on modern module organization.

---

## Reference

- **Account-level instructions**: `$HOME/.copilot/copilot-instructions.md`
- **Rust guidance**: `$HOME/.copilot/copilot-instructions-rust.md`
- **Project README**: [README.md](../../README.md)
