use assert_cmd::Command;
use assert_cmd::cargo::cargo_bin;
use insta_cmd::StdinCommand;

#[allow(dead_code)]
pub fn ferrish_cmd() -> Command {
    #[allow(unused_mut)]
    let mut cmd = Command::cargo_bin("ferrish").unwrap();
    #[cfg(coverage)]
    if let Ok(template) = std::env::var("LLVM_PROFILE_FILE") {
        cmd.env("LLVM_PROFILE_FILE", template);
    }
    cmd
}

/// Returns a [`StdinCommand`] for use with [`insta_cmd::assert_cmd_snapshot!`].
#[allow(dead_code)]
pub fn snapshot_cmd(input: &[u8]) -> StdinCommand {
    #[allow(unused_mut)]
    let mut cmd = StdinCommand::new(cargo_bin("ferrish"), input);
    #[cfg(coverage)]
    if let Ok(template) = std::env::var("LLVM_PROFILE_FILE") {
        cmd.env("LLVM_PROFILE_FILE", template);
    }
    cmd
}
