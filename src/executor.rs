use crate::{
    Arg, Command,
    arg::Args,
    command::builtin::{BuiltInCommand, BuiltInName},
    ctx::ShellCtx,
    error::{ShellError, ShellResult},
    exit::ExitCode,
    fs,
    io::ShellIo,
};

pub fn execute(
    command: Command,
    args: Args,
    io: &mut impl ShellIo,
    ctx: &mut ShellCtx,
) -> ShellResult<Option<ExitCode>> {
    match command {
        Command::BuiltIn(builtin) => execute_builtin(builtin, args, io, ctx),
        Command::Executable(executable) => execute_executable(executable, args, io, ctx),
        Command::Unrecognized(_) => Err(ShellError::CommandNotFound),
    }
}

fn execute_builtin(
    builtin: BuiltInCommand,
    args: Args,
    io: &mut impl ShellIo,
    ctx: &mut ShellCtx,
) -> ShellResult<Option<ExitCode>> {
    match builtin.name() {
        BuiltInName::Exit => {
            let code = args
                .first()
                .and_then(|a| a.to_string().parse::<u8>().ok())
                .map(ExitCode)
                .unwrap_or(ExitCode::SUCCESS);
            return Ok(Some(code));
        }
        BuiltInName::Cd => execute_cd(args, io, ctx)?,
        BuiltInName::Echo => execute_echo(args, io)?,
        BuiltInName::Type => execute_type(args, io)?,
        BuiltInName::Pwd => execute_pwd(args, io, ctx)?,
    }

    Ok(None)
}

fn execute_cd(args: Args, _io: &mut impl ShellIo, ctx: &mut ShellCtx) -> ShellResult<()> {
    let default_target = Arg::from(b"~".as_slice());
    let target = args.first().unwrap_or(&default_target);
    let new_dir = fs::resolve_path(&target.into(), ctx.home_dir.as_deref(), &ctx.cwd)?;

    if !new_dir.exists() {
        return Err(ShellError::FileNotFound {
            arg: target.clone(),
        });
    } else if !new_dir.is_dir() {
        return Err(ShellError::NotADirectory {
            arg: target.clone(),
        });
    }

    ctx.cwd = new_dir;

    Ok(())
}

fn execute_echo(args: Args, io: &mut impl ShellIo) -> ShellResult<()> {
    writeln!(
        io.out_writer(),
        "{}",
        args.iter()
            .map(|a| a.to_string())
            .collect::<Vec<_>>()
            .join(" ")
    )?;

    Ok(())
}

fn execute_type(args: Args, io: &mut impl ShellIo) -> ShellResult<()> {
    if args.is_empty() {
        return Err(ShellError::MissingOperand);
    }

    // TODO: get type without fully parsing the arg
    match Command::from(args.first().expect("at least one arg")) {
        Command::BuiltIn(builtin) => writeln!(io.out_writer(), "{} is a shell builtin", builtin)?,
        Command::Executable(executable) => writeln!(
            io.out_writer(),
            "{} is {}",
            executable,
            executable.file_path().display()
        )?,
        Command::Unrecognized(_) => return Err(ShellError::CommandNotFound),
    }

    Ok(())
}

fn execute_pwd(_args: Args, io: &mut impl ShellIo, ctx: &ShellCtx) -> ShellResult<()> {
    writeln!(io.out_writer(), "{}", ctx.cwd.display())?;
    Ok(())
}

fn execute_executable(
    executable: crate::command::executable::ExecutableCommand,
    args: Args,
    io: &mut impl ShellIo,
    ctx: &ShellCtx,
) -> ShellResult<Option<ExitCode>> {
    let args = args.iter().map(|a| a.to_string()).collect::<Vec<_>>();
    let output = std::process::Command::new(executable.file_path())
        .args(args)
        .current_dir(&ctx.cwd)
        .output()
        .map_err(ShellError::SpawnFailed)?;

    io.out_writer().write_all(&output.stdout)?;
    io.err_writer().write_all(&output.stderr)?;

    if !output.status.success() {
        return Err(ShellError::NonZeroExit(output.status));
    }

    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::io::MockIo;
    use std::path::PathBuf;

    fn test_ctx() -> ShellCtx {
        ShellCtx::new(None, std::env::current_dir().unwrap_or_else(|_| PathBuf::from("/")))
    }

    #[test]
    fn test_execute_builtin_exit() {
        let mut io = MockIo::empty();
        let mut ctx = test_ctx();
        let result = execute_builtin(BuiltInCommand::new(BuiltInName::Exit), vec![], &mut io, &mut ctx);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), Some(ExitCode::SUCCESS));
    }

    #[test]
    fn test_execute_builtin_exit_with_code() {
        let mut io = MockIo::empty();
        let mut ctx = test_ctx();
        let result = execute_builtin(
            BuiltInCommand::new(BuiltInName::Exit),
            vec![Arg::from("1")],
            &mut io,
            &mut ctx,
        );
        assert_eq!(result.unwrap(), Some(ExitCode(1)));
    }

    #[test]
    fn test_execute_type_builtin() {
        let args = vec![Arg::from("echo")];
        let mut io = MockIo::empty();
        execute_type(args, &mut io).unwrap();
        assert_eq!(io.output(), b"echo is a shell builtin\n");
    }

    #[test]
    fn test_execute_type_unrecognized() {
        let args = vec![Arg::from("nonexistentcommand")];
        let mut io = MockIo::empty();
        let result = execute_type(args, &mut io);
        assert!(result.is_err());
        assert_eq!(result.err().unwrap(), ShellError::CommandNotFound);
    }

    #[test]
    fn test_execute_type_exit_builtin() {
        let args = vec![Arg::from("exit")];
        let mut io = MockIo::empty();
        execute_type(args, &mut io).unwrap();
        assert_eq!(io.output(), b"exit is a shell builtin\n");
    }

    #[test]
    fn test_execute_builtin_pwd() {
        let cwd = PathBuf::from("/some/test/path");
        let ctx = ShellCtx::new(None, cwd.clone());
        let mut io = MockIo::empty();
        execute_pwd(vec![], &mut io, &ctx).unwrap();
        assert_eq!(io.output(), format!("{}\n", cwd.display()).as_bytes());
    }
}
