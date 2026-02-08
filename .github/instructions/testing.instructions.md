# Ferrish Testing Patterns

This file supplements account-level testing guidance with ferrish-specific strategies.

See account-level guidance for general testing philosophy.

## Testing Strategy for Shells

Testing shells is uniquely challenging. We use **multi-level approach**:

### Unit Tests (in `src/**/*.rs`)
- Test individual components in isolation
- Use mocks for external dependencies
- Pattern: `#[cfg(test)] mod tests { ... }` at end of file
- Run with: `cargo test --lib`

### Integration Tests (in `tests/`)
- Test complete workflows
- One file per major feature: lexing.rs, parsing.rs, evaluation.rs, end_to_end.rs
- Spawn ferrish processes for functional tests
- Run with: `cargo test --test '*'`

### Shell-Bound Tests (critical for ferrish)
- Verify **actual shell behavior** not just component behavior
- Spawn ferrish subprocess and verify I/O, process handling, semantics
- Tests: I/O redirection, pipes, signal handling, exit codes

Example:
```rust
#[test]
fn test_variable_assignment_and_expansion() {
    let output = Command::new("target/debug/ferrish")
        .arg("-c")
        .arg("x=42; echo $x")
        .output()
        .expect("Failed to run ferrish");
    
    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "42");
}

#[test]
fn test_pipe_chains_correctly() {
    let output = Command::new("target/debug/ferrish")
        .arg("-c")
        .arg("echo hello | wc -w")
        .output()
        .expect("Failed to run ferrish");
    
    assert!(output.status.success());
    assert!(String::from_utf8_lossy(&output.stdout).contains("1"));
}
```

### Benchmarks (in `benches/`)
- Use `criterion` crate for statistical rigor
- Focus on: lexing large files, parsing complex expressions, REPL responsiveness
- Run with: `cargo bench`

## When to Test

- **Always**: New features, bug fixes, refactoring
- **Never**: Trivial changes (variable renames, comments)
- **Maybe**: Third-party library calls (if wrapping, test the wrapper)

## Test Organization

### File naming
- Test file per module: `tests/lexing.rs`, `tests/parsing.rs`
- Use descriptive names: `test_pipes_chain_correctly()`

### Test structure
```rust
#[test]
fn test_descriptive_behavior() {
    // Arrange: set up test state
    let input = "echo hello";
    
    // Act: perform operation
    let tokens = lex(input).unwrap();
    
    // Assert: verify result
    assert_eq!(tokens.len(), 2);
}
```

## Debugging Failing Tests

Enable output:
```bash
cargo test -- --nocapture
```

Run specific test:
```bash
cargo test test_name -- --nocapture
```

Backtrace:
```bash
RUST_BACKTRACE=1 cargo test
```

## Shell-Specific Challenges

### I/O Handling
- Test stdin/stdout/stderr redirection
- Verify pipe communication
- Test both success and error paths

### Process Management
- Always `.expect()` on Command results
- Use `.output()` to capture stdout/stderr
- Check `.status.success()` for exit code
- Convert bytes: `String::from_utf8_lossy()`

### State Management
- Test variable scoping
- Verify state isolation between commands
- Test environment variable inheritance

### Semantics
- Test quoting (single, double, escapes)
- Test parameter expansion ($var, ${var}, etc.)
- Test command substitution
- Test operators and precedence

## Coverage

- Use `cargo tarpaulin` or `cargo llvm-cov`
- Aim for >80% coverage on core modules
- Acceptable to skip: unreachable error paths, OS-specific signals
