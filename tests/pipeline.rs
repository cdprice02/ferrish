mod common;

use assert_fs::TempDir;
use assert_fs::prelude::*;
use predicates::prelude::*;

use common::ferrish_cmd;

#[cfg(unix)]
#[test]
fn pipeline_echo_to_cat() {
    ferrish_cmd()
        .write_stdin("echo hello | cat\n")
        .assert()
        .success()
        .stdout(predicate::str::contains("hello"));
}

#[cfg(unix)]
#[test]
fn pipeline_echo_to_wc_l_counts_one_line() {
    let output = ferrish_cmd()
        .write_stdin("echo hello | wc -l\n")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "expected success, got: {:?}",
        output.status
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains('1'), "expected 1 line, got: {:?}", stdout);
}

#[cfg(unix)]
#[test]
fn pipeline_builtin_output_piped_to_cat() {
    ferrish_cmd()
        .write_stdin("echo 'piped content' | cat\n")
        .assert()
        .success()
        .stdout(predicate::str::contains("piped content"));
}

#[cfg(unix)]
#[test]
fn pipeline_last_stage_redirect_to_file() {
    let temp = TempDir::new().unwrap();
    ferrish_cmd()
        .current_dir(temp.path())
        .write_stdin("echo foo | cat > out.txt\n")
        .assert()
        .success()
        .stdout(predicate::str::contains("foo").not());
    let contents = std::fs::read_to_string(temp.path().join("out.txt")).expect("out.txt");
    assert_eq!(contents.trim(), "foo");
}

#[test]
fn pipeline_quoted_pipe_is_literal() {
    ferrish_cmd()
        .write_stdin("echo 'foo | bar'\n")
        .assert()
        .success()
        .stdout(predicate::str::contains("foo | bar"));
}

#[test]
fn pipeline_double_quoted_pipe_is_literal() {
    ferrish_cmd()
        .write_stdin("echo \"foo | bar\"\n")
        .assert()
        .success()
        .stdout(predicate::str::contains("foo | bar"));
}

#[cfg(unix)]
#[test]
fn pipeline_pwd_piped_to_grep() {
    ferrish_cmd()
        .write_stdin("pwd | grep /\n")
        .assert()
        .success()
        .stdout(predicate::str::contains("/"));
}

#[test]
fn pipeline_last_stage_exit_code_propagates() {
    ferrish_cmd()
        .write_stdin("echo foo | exit 0\n")
        .assert()
        .code(0);
}

// --- 3+ stage pipeline tests ---

#[cfg(unix)]
#[test]
fn three_stage_pipeline_passthrough() {
    ferrish_cmd()
        .write_stdin("echo hello | cat | cat\n")
        .assert()
        .success()
        .stdout(predicate::str::contains("hello"));
}

#[cfg(unix)]
#[test]
fn four_stage_pipeline_passthrough() {
    ferrish_cmd()
        .write_stdin("echo hello | cat | cat | cat\n")
        .assert()
        .success()
        .stdout(predicate::str::contains("hello"));
}

#[cfg(unix)]
#[test]
fn three_stage_pipeline_transforms_data() {
    ferrish_cmd()
        .write_stdin("echo hello | tr h H | cat\n")
        .assert()
        .success()
        .stdout(predicate::str::contains("Hello"));
}

#[cfg(unix)]
#[test]
fn three_stage_pipeline_echo_cat_wc_c() {
    let output = ferrish_cmd()
        .write_stdin("echo hello | cat | wc -c\n")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "expected success, got: {:?}",
        output.status
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    let count: usize = stdout
        .split_whitespace()
        .find_map(|t| t.parse().ok())
        .unwrap_or_else(|| panic!("expected byte count in: {:?}", stdout));
    assert_eq!(count, 6, "wc -c should report 6 bytes for 'hello\\n'");
}

#[cfg(unix)]
#[test]
fn three_stage_pipeline_with_final_redirect() {
    let temp = TempDir::new().unwrap();
    ferrish_cmd()
        .current_dir(temp.path())
        .write_stdin("echo foo | cat | cat > out.txt\n")
        .assert()
        .success()
        .stdout(predicate::str::contains("foo").not());
    let contents = std::fs::read_to_string(temp.path().join("out.txt")).expect("out.txt");
    assert_eq!(contents.trim(), "foo");
}

// --- Builtins at any position in a pipeline ---

#[cfg(unix)]
#[test]
fn pipeline_builtin_in_middle_position() {
    ferrish_cmd()
        .write_stdin("echo ignored | pwd | grep /\n")
        .assert()
        .success()
        .stdout(predicate::str::contains("/"))
        .stdout(predicate::str::contains("ignored").not());
}

#[test]
fn pipeline_builtin_receives_piped_stdin() {
    ferrish_cmd()
        .write_stdin("echo upstream | echo downstream\n")
        .assert()
        .success()
        .stdout(predicate::str::contains("downstream"))
        .stdout(predicate::str::contains("upstream").not());
}

// --- Pipeline fail-fast / exit code propagation ---

#[cfg(unix)]
#[test]
fn pipeline_first_stage_failure_surfaces_error() {
    ferrish_cmd()
        .write_stdin("false | echo should_not_run\n")
        .assert()
        .stderr(predicate::str::contains("exited with"));
}

#[cfg(unix)]
#[test]
fn pipeline_middle_stage_failure_surfaces_error() {
    ferrish_cmd()
        .write_stdin("echo a | false | echo should_not_run\n")
        .assert()
        .stderr(predicate::str::contains("exited with"));
}

#[cfg(unix)]
#[test]
fn pipeline_last_stage_nonzero_surfaces_error() {
    ferrish_cmd()
        .write_stdin("echo ok | false\n")
        .assert()
        .stderr(predicate::str::contains("exited with status"));
}

#[cfg(unix)]
#[test]
fn pipeline_all_stages_succeed_no_error() {
    ferrish_cmd()
        .write_stdin("echo hello | cat\n")
        .assert()
        .success()
        .stdout(predicate::str::contains("hello"))
        .stderr(predicate::str::is_empty());
}

// --- Malformed pipeline segment errors ---

#[test]
fn trailing_pipe_reports_error_and_continues() {
    ferrish_cmd()
        .write_stdin("echo hello |\necho survived\n")
        .assert()
        .stderr(predicate::str::contains("|"))
        .stdout(predicate::str::contains("survived"))
        .stdout(predicate::str::contains("hello").not())
        .code(0);
}

#[test]
fn leading_pipe_reports_error_and_continues() {
    ferrish_cmd()
        .write_stdin("| cat\necho survived\n")
        .assert()
        .stderr(predicate::str::contains("|"))
        .stdout(predicate::str::contains("survived"))
        .code(0);
}

#[test]
fn empty_middle_segment_reports_error_and_continues() {
    ferrish_cmd()
        .write_stdin("echo a | | cat\necho survived\n")
        .assert()
        .stderr(predicate::str::contains("|"))
        .stdout(predicate::str::contains("survived"))
        .stdout(predicate::str::contains("a\n").not())
        .code(0);
}

// --- OS-level concurrent pipe correctness ---

#[cfg(unix)]
#[test]
fn pipeline_large_data_no_deadlock() {
    let output = ferrish_cmd()
        .write_stdin("yes | head -c 131072 | wc -c\n")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "expected success, got: {:?}",
        output.status
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    let count: usize = stdout
        .split_whitespace()
        .find_map(|t| t.parse().ok())
        .unwrap_or_else(|| panic!("expected byte count in: {:?}", stdout));
    assert_eq!(count, 131072, "expected 131072 bytes through pipeline");
}

#[cfg(unix)]
#[test]
fn pipeline_builtin_cd_in_middle_does_not_change_shell_cwd() {
    let temp = TempDir::new().unwrap();
    temp.child("marker").create_dir_all().unwrap();
    ferrish_cmd()
        .current_dir(temp.path())
        .write_stdin("echo x | cd / | cat\ncd marker\necho ok\n")
        .assert()
        .success()
        .stdout(predicate::str::contains("ok"))
        .stderr(predicate::str::is_empty());
}
