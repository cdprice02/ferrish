mod common;

use common::ShellTestSession;

#[test]
fn test_cd_to_home_with_no_args() {
    let shell = ShellTestSession::new();

    let subdir = shell.home_dir().join("subdir");
    std::fs::create_dir(&subdir).expect("Failed to create subdir");

    let result = shell.run(&["cd subdir", "pwd", "cd", "pwd"]);
    eprintln!("Output:\n{}", result.output);
    eprintln!("Error:\n{}", result.error);
}

#[test]
fn test_cd_tilde() {}

#[test]
fn test_cd_tilde_subdirectory() {}

#[test]
fn test_home_directory_isolation() {}

#[test]
fn test_cd_to_temp_directory() {}

#[test]
fn test_cd_with_created_subdirectory() {}

#[test]
fn test_cd_with_relative_paths() {}
