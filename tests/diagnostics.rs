mod common;

use assert_fs::TempDir;
use assert_fs::prelude::*;
use insta_cmd::assert_cmd_snapshot;

use common::snapshot_cmd;

#[test]
fn command_not_found_diagnostic() {
    assert_cmd_snapshot!(snapshot_cmd("foobar arg1\n".as_bytes()));
}

#[test]
fn cd_file_not_found_diagnostic() {
    assert_cmd_snapshot!(snapshot_cmd("cd /this/path/does/not/exist\n".as_bytes()));
}

#[test]
fn cd_not_a_directory_diagnostic() {
    let temp = TempDir::new().unwrap();
    temp.child("plain.txt").write_str("").unwrap();
    let mut cmd = snapshot_cmd("cd plain.txt\n".as_bytes());
    cmd.current_dir(temp.path());
    assert_cmd_snapshot!(cmd);
}

#[test]
fn unclosed_quote_diagnostic() {
    assert_cmd_snapshot!(snapshot_cmd("echo 'hello\n".as_bytes()));
}
