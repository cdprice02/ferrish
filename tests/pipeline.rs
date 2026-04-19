mod harness;

use harness::ShellHarness;

#[cfg(unix)]
#[test]
fn pipeline_echo_to_cat() {
    ShellHarness::new().run("echo hello | cat").assert_stdout_contains("hello");
}

#[cfg(unix)]
#[test]
fn pipeline_echo_to_wc_l_counts_one_line() {
    let out = ShellHarness::new().run("echo hello | wc -l");
    assert!(out.stdout().contains('1'), "expected 1 line, got: {:?}", out.stdout());
}

#[cfg(unix)]
#[test]
fn pipeline_builtin_output_piped_to_cat() {
    ShellHarness::new()
        .run("echo 'piped content' | cat")
        .assert_stdout_contains("piped content");
}

#[cfg(unix)]
#[test]
fn pipeline_last_stage_redirect_to_file() {
    let out = ShellHarness::new().run("echo foo | cat > out.txt");
    out.assert_stdout_not_contains("foo");
    let contents = std::fs::read_to_string(out.home_dir().join("out.txt")).expect("out.txt");
    assert_eq!(contents.trim(), "foo");
}

#[test]
fn pipeline_quoted_pipe_is_literal() {
    ShellHarness::new()
        .run("echo 'foo | bar'")
        .assert_stdout_contains("foo | bar");
}

#[test]
fn pipeline_double_quoted_pipe_is_literal() {
    ShellHarness::new()
        .run("echo \"foo | bar\"")
        .assert_stdout_contains("foo | bar");
}

#[cfg(unix)]
#[test]
fn pipeline_pwd_piped_to_grep() {
    ShellHarness::new().run("pwd | grep /").assert_stdout_contains("/");
}

#[test]
fn pipeline_last_stage_exit_code_propagates() {
    ShellHarness::new().run("echo foo | exit 0").assert_exit_code(0);
}

// --- 3+ stage pipeline tests ---

#[cfg(unix)]
#[test]
fn three_stage_pipeline_passthrough() {
    ShellHarness::new().run("echo hello | cat | cat").assert_stdout_contains("hello");
}

#[cfg(unix)]
#[test]
fn four_stage_pipeline_passthrough() {
    ShellHarness::new()
        .run("echo hello | cat | cat | cat")
        .assert_stdout_contains("hello");
}

#[cfg(unix)]
#[test]
fn three_stage_pipeline_transforms_data() {
    ShellHarness::new()
        .run("echo hello | tr h H | cat")
        .assert_stdout_contains("Hello");
}

#[cfg(unix)]
#[test]
fn three_stage_pipeline_echo_cat_wc_c() {
    let out = ShellHarness::new().run("echo hello | cat | wc -c");
    let count: usize = out.stdout()
        .split_whitespace()
        .find_map(|t| t.parse().ok())
        .unwrap_or_else(|| panic!("expected byte count in: {:?}", out.stdout()));
    assert_eq!(count, 6, "wc -c should report 6 bytes for 'hello\\n'");
}

#[cfg(unix)]
#[test]
fn three_stage_pipeline_with_final_redirect() {
    let out = ShellHarness::new().run("echo foo | cat | cat > out.txt");
    out.assert_stdout_not_contains("foo");
    let contents = std::fs::read_to_string(out.home_dir().join("out.txt")).expect("out.txt");
    assert_eq!(contents.trim(), "foo");
}

// --- Builtins at any position in a pipeline ---

#[cfg(unix)]
#[test]
fn pipeline_builtin_in_middle_position() {
    let out = ShellHarness::new().run("echo ignored | pwd | grep /");
    out.assert_stdout_contains("/")
        .assert_stdout_not_contains("ignored");
}

#[cfg(unix)]
#[test]
fn pipeline_builtin_receives_piped_stdin() {
    ShellHarness::new()
        .run("echo upstream | echo downstream")
        .assert_stdout_contains("downstream")
        .assert_stdout_not_contains("upstream");
}

// --- Pipeline fail-fast / exit code propagation ---

#[cfg(unix)]
#[test]
fn pipeline_aborts_on_first_stage_failure() {
    ShellHarness::new()
        .run("false | echo should_not_run")
        .assert_stdout_not_contains("should_not_run");
}

#[cfg(unix)]
#[test]
fn pipeline_aborts_on_middle_stage_failure() {
    ShellHarness::new()
        .run("echo a | false | echo should_not_run")
        .assert_stdout_not_contains("should_not_run");
}

#[cfg(unix)]
#[test]
fn pipeline_last_stage_nonzero_surfaces_error() {
    ShellHarness::new()
        .run("echo ok | false")
        .assert_stderr_contains("exited with status");
}

#[cfg(unix)]
#[test]
fn pipeline_all_stages_succeed_no_error() {
    ShellHarness::new()
        .run("echo hello | cat")
        .assert_stdout_contains("hello")
        .assert_stderr_empty();
}
