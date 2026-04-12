mod harness;

use harness::ShellTest;

#[test]
fn test_exit_no_args_exits_zero() {
    let result = ShellTest::new().script("exit").run();
    assert_eq!(result.exit_code(), 0);
}

#[test]
fn test_exit_valid_code_exits_with_code() {
    let result = ShellTest::new().script("exit 42").run();
    assert_eq!(result.exit_code(), 42);
}

#[test]
fn test_exit_out_of_range_prints_error() {
    let result = ShellTest::new().script("exit 256").run();
    result.assert_error_contains("numeric argument required");
    assert_eq!(result.exit_code(), 1);
}

#[test]
fn test_exit_non_numeric_prints_error() {
    let result = ShellTest::new().script("exit abc").run();
    result.assert_error_contains("numeric argument required");
    assert_eq!(result.exit_code(), 1);
}
