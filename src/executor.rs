use std::io::Write;

use crate::{
    Arg, Command,
    arg::Args,
    command::builtin::{BuiltInCommand, BuiltInName},
    env,
    error::{ShellError, ShellResult},
    fs,
};

pub fn execute<W: Write>(command: Command, args: Args, out_writer: &mut W) -> ShellResult<bool> {
    // TODO: create ExitCode type instead of bool
    match command {
        Command::BuiltIn(builtin) => execute_builtin(builtin, args, out_writer),
        Command::Executable(executable) => execute_executable(executable, args),
        Command::Unrecognized(_) => Err(ShellError::CommandNotFound),
    }
}

fn execute_builtin<W: Write>(
    builtin: BuiltInCommand,
    args: Args,
    out_writer: &mut W,
) -> ShellResult<bool> {
    let name = builtin.name();
    match name {
        BuiltInName::Exit => Ok(false),
        BuiltInName::Cd => execute_cd(args),
        BuiltInName::Echo => execute_echo(args, out_writer),
        BuiltInName::Type => execute_type(args, out_writer),
        BuiltInName::Pwd => execute_pwd(out_writer),
    }
}

fn execute_cd(args: Args) -> ShellResult<bool> {
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

    Ok(true)
}

fn execute_echo<W: Write>(args: Args, out_writer: &mut W) -> ShellResult<bool> {
    writeln!(
        out_writer,
        "{}",
        args.iter()
            .map(|a| a.to_string())
            .collect::<Vec<_>>()
            .join(" ")
    )?;
    Ok(true)
}

fn execute_type<W: Write>(args: Args, out_writer: &mut W) -> ShellResult<bool> {
    if args.is_empty() {
        return Err(ShellError::MissingOperand {
            builtin: BuiltInName::Type,
        });
    }

    // TODO: get type without fully parsing the arg
    match Command::from(args.first().expect("at least one arg")) {
        Command::BuiltIn(builtin) => writeln!(out_writer, "{} is a shell builtin", builtin)?,
        Command::Executable(executable) => writeln!(
            out_writer,
            "{} is {}",
            executable,
            executable.file_path().display()
        )?,
        Command::Unrecognized(_) => return Err(ShellError::CommandNotFound),
    }

    Ok(true)
}

fn execute_pwd<W: Write>(out_writer: &mut W) -> ShellResult<bool> {
    writeln!(out_writer, "{}", env::current_dir()?.display())?;
    Ok(true)
}

// TODO: handle custom I/O (at the moment, this inherits from the parent process which will break tests)
fn execute_executable(
    executable: crate::command::executable::ExecutableCommand,
    args: Args,
) -> ShellResult<bool> {
    let args = args.iter().map(|a| a.to_string()).collect::<Vec<_>>();
    let mut child = std::process::Command::new(executable.file_path())
        .args(args)
        .spawn()?;
    let status = child.wait()?;

    if !status.success() {
        return Err(ShellError::NonZeroExit(status));
    }

    Ok(true)
}
