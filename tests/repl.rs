use expectrl::{Expect, spawn};
use ferrish::Shell;

#[test]
fn test_repl() {}

// #[test]
// fn test_repl_empty_input() {
//     const NUM_EMPTY_LINES: usize = 3;
//     let mut lines = vec![""; NUM_EMPTY_LINES];
//     lines.push("exit");
//     let io = io::MockIo::from_lines(&lines);
//     let mut shell = Shell::builder().with_io(io);
//     let result = shell.run();
//     assert!(result.is_ok());
//     let io = shell.io();
//     let output = io.output();
//     let expected_output = shell::Shell::prefix().repeat(NUM_EMPTY_LINES + 1); // +1 for exit
//     assert_eq!(output.to_vec(), expected_output);
// }
