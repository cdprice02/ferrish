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
    let output = result.output().trim().to_string();
    assert!(
        output.contains('6'),
        "expected wc -c to report 6 bytes, got: {output:?}"
    );
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
