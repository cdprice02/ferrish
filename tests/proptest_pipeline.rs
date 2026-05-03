mod common;

use common::ferrish_cmd;
use proptest::prelude::*;
use proptest::test_runner::Config;

proptest! {
    #![proptest_config(Config::with_cases(25))]

    /// echo <args> | cat produces the same output as echo <args>
    #[test]
    fn echo_pipe_cat_passthrough(
        args in proptest::collection::vec("[a-z0-9]{1,8}", 1..=5)
    ) {
        let cmd_str = format!("echo {}", args.join(" "));
        let piped_str = format!("echo {} | cat", args.join(" "));

        let direct = ferrish_cmd()
            .arg("-c").arg(&cmd_str)
            .output().unwrap();
        let piped = ferrish_cmd()
            .arg("-c").arg(&piped_str)
            .output().unwrap();

        prop_assert_eq!(direct.stdout, piped.stdout,
            "echo | cat mismatch for args: {:?}", args);
    }

    /// -c mode and stdin mode produce the same stdout for single builtin commands
    #[test]
    fn c_mode_equals_stdin_mode(word in "[a-z]{1,10}") {
        let cmd_str = format!("echo {}", word);

        let via_c = ferrish_cmd()
            .arg("-c").arg(&cmd_str)
            .output().unwrap();
        let via_stdin = ferrish_cmd()
            .write_stdin(format!("{}\n", cmd_str))
            .output().unwrap();

        prop_assert_eq!(via_c.stdout, via_stdin.stdout,
            "stdout differs between -c and stdin mode for: {:?}", cmd_str);
    }
}

#[cfg(unix)]
proptest! {
    #![proptest_config(Config::with_cases(25))]

    /// Redirecting stdout to /dev/null leaves terminal stdout empty
    #[test]
    fn redirect_to_devnull_leaves_stdout_empty(word in "[a-z]{1,10}") {
        let output = ferrish_cmd()
            .arg("-c").arg(format!("echo {} > /dev/null", word))
            .output().unwrap();

        prop_assert!(output.stdout.is_empty(),
            "stdout not empty after redirect to /dev/null: {:?}", word);
    }

    /// echo with N args, piped to wc -w, reports N words
    #[test]
    fn echo_arg_count_matches_wc(
        words in proptest::collection::vec("[a-z]{1,8}", 1..=10)
    ) {
        let cmd_str = format!("echo {} | wc -w", words.join(" "));
        let output = ferrish_cmd()
            .arg("-c").arg(&cmd_str)
            .output().unwrap();

        prop_assert!(output.status.success(), "command failed: {}", cmd_str);
        let stdout = String::from_utf8_lossy(&output.stdout);
        let count: usize = stdout.trim().parse().expect("wc -w output not an integer");
        prop_assert_eq!(count, words.len(),
            "wc reported {} but expected {} for: {:?}", count, words.len(), words);
    }
}
