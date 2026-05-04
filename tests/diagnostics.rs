mod common;

use assert_cmd::cargo::cargo_bin;
use assert_fs::prelude::*;
use assert_fs::TempDir;
use insta_cmd::{assert_cmd_snapshot, StdinCommand};

#[test]
fn command_not_found_diagnostic() {
    assert_cmd_snapshot!(StdinCommand::new(
        cargo_bin("ferrish"),
        "foobar arg1\n".as_bytes()
    ));
}

#[test]
fn cd_file_not_found_diagnostic() {
    assert_cmd_snapshot!(StdinCommand::new(
        cargo_bin("ferrish"),
        "cd /this/path/does/not/exist\n".as_bytes()
    ));
}

#[test]
fn cd_not_a_directory_diagnostic() {
    let temp = TempDir::new().unwrap();
    temp.child("plain.txt").write_str("").unwrap();
    let mut cmd = StdinCommand::new(cargo_bin("ferrish"), "cd plain.txt\n".as_bytes());
    cmd.current_dir(temp.path());
    assert_cmd_snapshot!(cmd);
}

#[test]
fn unclosed_quote_diagnostic() {
    assert_cmd_snapshot!(StdinCommand::new(
        cargo_bin("ferrish"),
        "echo 'hello\n".as_bytes()
    ));
}
