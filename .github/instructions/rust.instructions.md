# Ferrish-Specific Rust Guidance

This file supplements account-level guidance (`$HOME/.copilot/copilot-instructions-rust.md`) with ferrish-specific patterns.

## Module Organization

Ferrish uses **file-based modules** (modern Rust pattern):
- Single-file modules: `src/lexer.rs`, `src/parser.rs`, etc.
- Avoid `mod.rs` files where a single file suffices
- Benefits: Flatter structure, faster navigation, clear API boundaries

## Error Handling

### Error Types
- Use **thiserror** for module-specific types
- Use **anyhow** for error context
- Use **miette** for source span errors (lexer, parser)

### Error Philosophy
- Be specific: "Expected '}' at line 5" not "Parse error"
- Suggest solutions: "Did you mean 'cd'?" for unknown commands
- No panic in library code (user input shouldn't panic)

## Testing for Shells

Shell testing requires multiple levels:

### Unit Tests
- Test components in isolation
- Use mocks for external dependencies
- Pattern: `#[cfg(test)] mod tests { ... }` at end of file

### Integration Tests  
- Test workflows together (lexing.rs, parsing.rs, evaluation.rs, end_to_end.rs)
- Spawn ferrish as subprocess for functional verification

### Shell-Bound Tests
- **Critical for ferrish**: Verify actual shell behavior
- Spawn ferrish subprocess, verify I/O, process management, semantics

Example:
```rust
#[test]
fn test_pipe_chains_commands() {
    let output = Command::new("target/debug/ferrish")
        .arg("-c")
        .arg("echo hello | wc -w")
        .output()
        .expect("Failed to run ferrish");
    
    assert!(output.status.success());
    assert!(String::from_utf8_lossy(&output.stdout).contains("1"));
}
```

## Documentation

### Public API (rustdoc)
- Every public item must have `///` doc comment
- Include examples for non-trivial items
- Document errors and panics
- Example:
  ```rust
  /// Tokenize the input string into tokens.
  ///
  /// # Examples
  /// ```
  /// let tokens = lex("echo hello")?;
  /// assert_eq!(tokens.len(), 2);
  /// ```
  pub fn lex(input: &str) -> Result<Vec<Token>, LexError> { ... }
  ```

## Safety

- **No unsafe code** without explicit justification
- Validate all user input thoroughly
- Be explicit about allowed operations
- Document platform-specific behaviors

## Performance

- Lexing/parsing should be fast (<100ms for typical input)
- Optimization is secondary to correctness
- Benchmark critical paths: `cargo bench`
- Profile with: `cargo flamegraph`

## Dependencies

Minimize dependencies; prefer standard library. Current:
- **clap** - CLI argument parsing
- **anyhow** - Error context
- **thiserror** - Custom error types
- **miette** - Error reporting with source spans
- **criterion** - Benchmarking (dev)

New dependencies require discussion before adding.

## Commit Messages

Use conventional commits:
- `feat: add lexer module`
- `fix: handle escaped quotes`
- `refactor: simplify parsing`
- `docs: document API`
- `test: add integration tests`

Reference issues: `Closes #42`, `Relates to #21`
