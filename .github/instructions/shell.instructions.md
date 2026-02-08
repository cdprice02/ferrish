# Shell-Specific Development Notes

This file documents challenges and best practices unique to shell development.

## Challenges Unique to Shells

### 1. Complex I/O
Shells manage multiple streams:
- **stdin**: User or piped input
- **stdout**: Normal output
- **stderr**: Error output
- **Pipes**: Output → input chaining
- **Redirections**: Changing where output goes (>, >>, &>, etc.)

**Best practices**:
- Test I/O redirection separately
- Use shell-bound tests for actual I/O verification
- Document stream assumptions

### 2. Process Management
Shells spawn and manage subprocesses:
- Spawning external commands
- Foreground/background processes
- Signal propagation (Ctrl+C, SIGTERM)
- Exit code handling
- Job control

**Best practices**:
- Abstract process spawning
- Test signal handling explicitly
- Document platform-specific behavior
- Consider timeout handling

### 3. State Management
Shells maintain complex state:
- **Variables**: User and environment
- **Functions**: User-defined functions
- **Environment**: Current directory, env vars
- **History**: Command history (if implementing)
- **Options**: Configuration flags

**Best practices**:
- Keep state management centralized
- Test variable scoping thoroughly
- Document persistence across commands
- Test state isolation

### 4. User Interaction
Shells are deeply interactive:
- **Prompts**: Visual cues for input
- **Line editing**: Editing current input
- **History**: Accessing previous commands
- **Interrupts**: Handling Ctrl+C gracefully
- **Multi-line**: Commands spanning multiple lines

**Best practices**:
- Separate command execution from interaction
- Test REPL with shell-bound tests
- Document interrupt handling
- Consider accessibility

### 5. Semantics Complexity
Shell syntax has intricate semantics:
- **Quoting**: Single, double quotes, backticks
- **Globbing**: Filename expansion with *, ?, [...]
- **Parameter expansion**: $var, ${var}, ${var:-default}
- **Command substitution**: Command output in expressions
- **Operators**: Precedence, associativity, special behavior

**Best practices**:
- Document semantics explicitly
- Use comprehensive test cases
- Consider edge cases (empty strings, special chars)
- Reference core principle: "Explicit over implicit"

## Testing Shell Behavior

### Integration Tests Are Critical
Multiple levels required:
- **Unit tests**: Individual components
- **Integration tests**: Components together
- **Shell-bound tests**: Actual user-facing behavior (essential)

### Spawning ferrish
Shell-bound tests run ferrish as subprocess:

```rust
use std::process::Command;

#[test]
fn test_shell_executes_command() {
    let output = Command::new("target/debug/ferrish")
        .arg("-c")
        .arg("echo hello")
        .output()
        .expect("Failed to run ferrish");
    
    assert!(output.status.success());
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim(),
        "hello"
    );
}
```

**Important**:
- Always `.expect()` on Command result
- Use `.output()` to capture stdout/stderr
- Check `.status.success()` for exit code
- Convert bytes: `String::from_utf8_lossy()`

### Capturing Output
Different strategies:

```rust
// Capture all output
let output = Command::new("ferrish")
    .arg("-c")
    .arg("command")
    .output()?;

// Use specific streams
let mut child = Command::new("ferrish")
    .arg("-c")
    .arg("command")
    .spawn()?;

let exit_status = child.wait()?;
```

## Performance Considerations

### REPL Responsiveness
Users expect immediate feedback:
- Lexing/parsing: <100ms for typical inputs
- Large scripts (thousands of lines) still usable
- Benchmark before and after optimization

### Memory Usage
Shells run for long sessions:
- Watch for memory leaks
- Test with long-running REPL sessions
- Consider streaming for large inputs

### Bottleneck Identification
```bash
cargo build --release
cargo flamegraph -- -c "your test"
```

## Safety

### No Unsafe Code Without Justification
```rust
// BAD: Unjustified
unsafe { std::mem::transmute(x) }

// GOOD: Justified with documentation
unsafe { libc::kill(pid as i32, signal) }
```

### Input Validation
User input is hostile:
- Validate command arguments
- Check string lengths
- Whitelist allowed characters
- Document security assumptions

### Process Spawning Safety
External command execution is security boundary:
- Validate command paths
- Consider restricting executables
- Document security assumptions
- Test with known-safe commands first

## Platform-Specific Behaviors

Shells differ between Unix and Windows - document all platform-specific behavior.
