fn main() -> anyhow::Result<std::process::ExitCode> {
    let code = ferrish::run()?;
    Ok(code.into())
}
