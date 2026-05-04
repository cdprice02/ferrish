use assert_cmd::Command;

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
