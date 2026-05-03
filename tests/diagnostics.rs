mod common;

use assert_cmd::cargo::cargo_bin;
use assert_fs::prelude::*;
use assert_fs::TempDir;
use insta_cmd::{assert_cmd_snapshot, StdinCommand};

fn ferrish_stdin(temp: &TempDir, input: &'static str) -> StdinCommand {
    let mut cmd = StdinCommand::new(cargo_bin("ferrish"), input.as_bytes());
    cmd.current_dir(temp.path())
        .env("HOME", temp.path())
        .env("USERPROFILE", temp.path());
    cmd
}

#[test]
fn command_not_found_diagnostic() {
    let temp = TempDir::new().unwrap();
    let temp_path = temp.path().to_str().unwrap().to_string();
    insta::with_settings!({
        filters => vec![(temp_path.as_str(), "[TMPDIR]")]
    }, {
        assert_cmd_snapshot!(ferrish_stdin(&temp, "foobar arg1\n"));
    });
}

#[test]
fn cd_file_not_found_diagnostic() {
    let temp = TempDir::new().unwrap();
    let temp_path = temp.path().to_str().unwrap().to_string();
    insta::with_settings!({
        filters => vec![(temp_path.as_str(), "[TMPDIR]")]
    }, {
        assert_cmd_snapshot!(ferrish_stdin(&temp, "cd /this/path/does/not/exist\n"));
    });
}

#[test]
fn cd_not_a_directory_diagnostic() {
    let temp = TempDir::new().unwrap();
    temp.child("plain.txt").write_str("").unwrap();
    let temp_path = temp.path().to_str().unwrap().to_string();
    insta::with_settings!({
        filters => vec![(temp_path.as_str(), "[TMPDIR]")]
    }, {
        assert_cmd_snapshot!(ferrish_stdin(&temp, "cd plain.txt\n"));
    });
}

#[test]
fn unclosed_quote_diagnostic() {
    let temp = TempDir::new().unwrap();
    let temp_path = temp.path().to_str().unwrap().to_string();
    insta::with_settings!({
        filters => vec![(temp_path.as_str(), "[TMPDIR]")]
    }, {
        assert_cmd_snapshot!(ferrish_stdin(&temp, "echo 'hello\n"));
    });
}
