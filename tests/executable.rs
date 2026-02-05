use ferrish::Shell;
use ferrish::io;

mod common;

// #[test]
// fn test_executable_cargo() {
//     let io = io::MockIo::from_lines(&["cargo --version", "exit"]);
//     let mut shell = Shell::builder().with_io(io);

//     let result = shell.run();
//     assert!(result.is_ok());

//     let io = shell.io();
//     let output = io.output();
//     assert!(common::find_subsequence(output, b"cargo ").is_some()); // TODO: more precise check
//     let error = io.error();
//     assert_matching_output!(error, b"");
// }
