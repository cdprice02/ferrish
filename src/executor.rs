use crate::{
    Arg, Command,
    arg::Args,
    command::builtin::{BuiltInCommand, BuiltInName},
    env,
    error::{ShellError, ShellResult},
    exit::ExitCode,
    fs,
    io::ShellIo,
};

pub fn execute(
    command: Command,
    args: Args,
    io: &mut impl ShellIo,
) -> ShellResult<Option<ExitCode>> {
    match command {
        Command::BuiltIn(builtin) => execute_builtin(builtin, args, io),
        Command::Executable(executable) => execute_executable(executable, args, io),
        Command::Unrecognized(_) => Err(ShellError::CommandNotFound),
    }
}

fn execute_builtin(
    builtin: BuiltInCommand,
    args: Args,
    io: &mut impl ShellIo,
) -> ShellResult<Option<ExitCode>> {
    let execute = match builtin.name() {
        BuiltInName::Exit => {
            // TODO: parse exit code argument
            return Ok(Some(ExitCode::SUCCESS));
        }
        BuiltInName::Cd => execute_cd,
        BuiltInName::Echo => execute_echo,
        BuiltInName::Type => execute_type,
        BuiltInName::Pwd => execute_pwd,
    };
    execute(args, io)?;

    Ok(None)
}

fn execute_cd(args: Args, _io: &mut impl ShellIo) -> ShellResult<()> {
    let default_target = Arg::from(b"~".as_slice());
    let target = args.first().unwrap_or(&default_target);
    let new_dir = fs::resolve_path(&target.into())?;

    if !new_dir.exists() {
        return Err(ShellError::FileNotFound {
            arg: target.clone(),
        });
    } else if !new_dir.is_dir() {
        return Err(ShellError::NotADirectory {
            arg: target.clone(),
        });
    }

    env::set_current_dir(&new_dir)?;

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

fn execute_pwd(_args: Args, io: &mut impl ShellIo) -> ShellResult<()> {
    writeln!(io.out_writer(), "{}", env::current_dir()?.display())?;

    Ok(())
}

// TODO: handle custom I/O (at the moment, this inherits from the parent process which will break tests)
fn execute_executable(
    executable: crate::command::executable::ExecutableCommand,
    args: Args,
    _io: &mut impl ShellIo,
) -> ShellResult<Option<ExitCode>> {
    let args = args.iter().map(|a| a.to_string()).collect::<Vec<_>>();
    let mut child = std::process::Command::new(executable.file_path())
        .args(args)
        .spawn()
        .map_err(ShellError::SpawnFailed)?;
    let status = child.wait().map_err(ShellError::WaitFailed)?;

    if !status.success() {
        return Err(ShellError::NonZeroExit(status));
    }

    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::io::MockIo;

    #[test]
    fn test_execute_builtin_exit() {
        let mut io = MockIo::empty();
        let result = execute_builtin(BuiltInCommand::new(BuiltInName::Exit), vec![], &mut io);
        assert!(result.is_ok());
        let exit_code = result.unwrap();
        assert_eq!(exit_code, Some(ExitCode::SUCCESS));
    }

    #[test]
    fn test_execute_builtin_echo() {
        let args = vec![Arg::from("Hello"), Arg::from("World")];
        let mut io = MockIo::empty();
        let result = execute_builtin(BuiltInCommand::new(BuiltInName::Echo), args, &mut io);
        assert!(result.is_ok());
        let exit_code = result.unwrap();
        assert_eq!(exit_code, None);
        let output = io.output_as_string();
        assert_eq!(output, "Hello World\n");
    }

    #[test]
    fn test_execute_echo_no_args() {
        let args: Vec<Arg> = vec![];
        let mut io = MockIo::empty();
        execute_echo(args, &mut io).unwrap();
        let output = io.output_as_string();
        assert_eq!(output, "\n");
    }

    #[test]
    fn test_execute_type_builtin() {
        let args = vec![Arg::from("echo")];
        let mut io = MockIo::empty();
        execute_type(args, &mut io).unwrap();
        let output = io.output_as_string();
        assert_eq!(output, "echo is a shell builtin\n");
    }

    #[test]
    fn test_execute_type_executable() {
        let args = vec![Arg::from("cargo")];
        let mut io = MockIo::empty();
        execute_type(args, &mut io).unwrap();
        let output = io.output_as_string();
        assert!(output.contains("cargo is "));
    }

    #[test]
    fn test_execute_type_unrecognized() {
        let args = vec![Arg::from("nonexistentcommand")];
        let mut io = MockIo::empty();
        let result = execute_type(args, &mut io);
        assert!(result.is_err());
        let err = result.err().unwrap();
        assert_eq!(
            err,
            ShellError::CommandNotFound,
            "Expected CommandNotFound error"
        );
    }
}
