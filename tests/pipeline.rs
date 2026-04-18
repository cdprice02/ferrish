mod harness;

use harness::ShellTest;

#[test]
fn test_basic_pipeline_echo_cat() {
    let result = ShellTest::new()
        .script("echo hello | cat")
        .run();
    result.assert_output_contains("hello");
}

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

#[test]
fn test_pipeline_builtin_output_piped_to_cat() {
    let result = ShellTest::new()
        .script("echo 'piped content' | cat")
        .run();
    result.assert_output_contains("piped content");
}

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
