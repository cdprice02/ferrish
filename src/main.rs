fn main() -> miette::Result<std::process::ExitCode> {
    let code = ferrish::run()?;
    Ok(code.into())
}
