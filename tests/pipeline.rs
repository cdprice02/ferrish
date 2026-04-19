mod harness;

use harness::ShellTest;

#[cfg(unix)]
#[test]
fn test_basic_pipeline_echo_cat() {
    let result = ShellTest::new()
        .script("echo hello | cat")
        .run();
    result.assert_output_contains("hello");
}

#[cfg(unix)]
#[test]
fn test_pipeline_with_wc_l() {
    // `echo` produces one line; `wc -l` should report 1.
    let result = ShellTest::new()
        .script("echo hello | wc -l")
        .run();
    let output = result.output().trim().to_string();
    // wc -l output may include leading whitespace on some platforms
    assert!(
        output.contains('1'),
        "expected wc -l to report 1 line, got: {output:?}"
    );
}

#[cfg(unix)]
#[test]
fn test_pipeline_builtin_output_piped_to_cat() {
    let result = ShellTest::new()
        .script("echo 'piped content' | cat")
        .run();
    result.assert_output_contains("piped content");
}

#[cfg(unix)]
#[test]
fn test_pipeline_last_stage_redirect() {
    let result = ShellTest::new()
        .with_isolated_home()
        .script("echo foo | cat > out.txt")
        .run();

    let home = result.home_dir().expect("isolated home");
    let contents = std::fs::read_to_string(home.join("out.txt"))
        .expect("redirect file should exist");
    assert_eq!(contents.trim(), "foo", "pipeline output should be in the file");
    // Terminal stdout may contain the prompt but must not contain the piped data.
    assert!(
        !result.output().contains("foo"),
        "piped data must not appear on terminal stdout when redirected: {:?}",
        result.output()
    );
}

#[test]
fn test_pipeline_quoted_pipe_is_literal() {
    let result = ShellTest::new()
        .script("echo 'foo | bar'")
        .run();
    result.assert_output_contains("foo | bar");
}

#[test]
fn test_pipeline_double_quoted_pipe_is_literal() {
    let result = ShellTest::new()
        .script("echo \"foo | bar\"")
        .run();
    result.assert_output_contains("foo | bar");
}

#[cfg(unix)]
#[test]
fn test_pipeline_pwd_piped_to_grep() {
    let result = ShellTest::new()
        .with_isolated_home()
        .script("pwd | grep /")
        .run();
    result.assert_output_contains("/");
}

#[test]
fn test_pipeline_last_stage_exit_code_propagates() {
    // `exit 0` as the last stage of a pipeline must cause the shell to exit.
    let result = ShellTest::new()
        .script("echo foo | exit 0")
        .run();
    assert_eq!(result.exit_code(), 0);
}

// --- 3+ command pipeline tests (issue #28) ---

#[cfg(unix)]
#[test]
fn test_three_stage_pipeline_passthrough() {
    let result = ShellTest::new()
        .script("echo hello | cat | cat")
        .run();
    result.assert_output_contains("hello");
}

#[cfg(unix)]
#[test]
fn test_four_stage_pipeline_passthrough() {
    let result = ShellTest::new()
        .script("echo hello | cat | cat | cat")
        .run();
    result.assert_output_contains("hello");
}

#[cfg(unix)]
#[test]
fn test_three_stage_pipeline_transforms_data() {
    // tr translates 'h' → 'H'; cat passes it through
    let result = ShellTest::new()
        .script("echo hello | tr h H | cat")
        .run();
    result.assert_output_contains("Hello");
}

#[cfg(unix)]
#[test]
fn test_three_stage_pipeline_echo_cat_wc() {
    // "hello\n" is 6 bytes; wc -c should report 6
    let result = ShellTest::new()
        .script("echo hello | cat | wc -c")
        .run();
    let output = result.output();
    // wc -c output may be padded and appears alongside shell prompts in captured output;
    // scan tokens for the first that parses as an integer byte count.
    let count: usize = output
        .split_whitespace()
        .find_map(|t| t.parse().ok())
        .unwrap_or_else(|| panic!("expected wc -c output to contain a byte count, got: {output:?}"));
    assert_eq!(count, 6, "wc -c should report 6 bytes for 'hello\\n', got: {output:?}");
}

#[cfg(unix)]
#[test]
fn test_three_stage_pipeline_with_final_redirect() {
    let result = ShellTest::new()
        .with_isolated_home()
        .script("echo foo | cat | cat > out.txt")
        .run();

    let home = result.home_dir().expect("isolated home");
    let contents = std::fs::read_to_string(home.join("out.txt"))
        .expect("redirect file should exist");
    assert_eq!(contents.trim(), "foo");
    assert!(
        !result.output().contains("foo"),
        "piped data must not appear on terminal stdout when redirected"
    );
}

// --- Builtins at any position in a pipeline (issue #29) ---

#[cfg(unix)]
#[test]
fn test_pipeline_builtin_in_middle() {
    // pwd is a builtin in the middle stage; it ignores piped stdin and emits cwd.
    // grep filters for '/' which is present in any absolute path.
    let result = ShellTest::new()
        .with_isolated_home()
        .script("echo ignored | pwd | grep /")
        .run();
    let output = result.output();
    assert!(
        output.contains('/'),
        "expected piped pwd output to contain '/', got: {output:?}"
    );
    assert!(
        !output.contains("ignored"),
        "upstream builtin output must be piped, not written to terminal stdout: {output:?}"
    );
}

#[cfg(unix)]
#[test]
fn test_pipeline_builtin_receives_piped_stdin() {
    // A builtin at the receiving end of a pipe must not stall or error even
    // though none of the current builtins consume stdin.  echo ignores its
    // piped stdin and still emits its own arguments.
    let result = ShellTest::new()
        .script("echo upstream | echo downstream")
        .run();
    result.assert_output_contains("downstream");
    assert!(
        !result.output().contains("upstream"),
        "upstream builtin output must be routed into the pipe, not leaked to terminal stdout"
    );
}

// --- Pipeline fail-fast / exit code propagation (issue #30) ---

#[cfg(unix)]
#[test]
fn test_pipeline_aborts_on_first_stage_failure() {
    // `false` exits 1; the pipeline must abort before `echo` runs.
    let result = ShellTest::new()
        .script("false | echo should_not_run")
        .run();
    assert!(
        !result.output().contains("should_not_run"),
        "pipeline must abort at the first failing stage, got: {:?}",
        result.output()
    );
}

#[cfg(unix)]
#[test]
fn test_pipeline_aborts_on_middle_stage_failure() {
    // `echo a` succeeds, then `false` fails; `echo should_not_run` must not execute.
    let result = ShellTest::new()
        .script("echo a | false | echo should_not_run")
        .run();
    assert!(
        !result.output().contains("should_not_run"),
        "pipeline must abort at the failing middle stage, got: {:?}",
        result.output()
    );
}

#[cfg(unix)]
#[test]
fn test_pipeline_last_stage_nonzero_surfaces_error() {
    // When the last stage fails, the error is reported to the user.
    let result = ShellTest::new()
        .script("echo ok | false")
        .run();
    assert!(
        result.error().contains("exited with status") || result.error().contains("false"),
        "expected exit failure in stderr, got: {:?}",
        result.error()
    );
}

#[cfg(unix)]
#[test]
fn test_pipeline_all_succeed_no_error() {
    let result = ShellTest::new()
        .script("echo hello | cat")
        .run();
    result.assert_output_contains("hello");
    assert!(
        result.error().is_empty() || !result.error().contains("exited with status"),
        "no error expected for successful pipeline, got: {:?}",
        result.error()
    );
}
