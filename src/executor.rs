use std::io::Write;

use crate::{
    Arg, Command,
    arg::Args,
    command::builtin::{BuiltInCommand, BuiltInName},
    env, fs,
};

pub fn execute<WO: Write, WE: Write>(
    command: Command,
    args: Args,
    out_writer: &mut WO,
    err_writer: &mut WE,
) -> anyhow::Result<bool> {
    // TODO: create ExitCode type instead of bool
    match command {
        Command::BuiltIn(builtin) => execute_builtin(builtin, args, out_writer, err_writer),
        Command::Executable(executable) => {
            execute_executable(executable, args, out_writer, err_writer)
        }
        Command::Unrecognized(name) => {
            writeln!(
                err_writer,
                "{}: command not found",
                String::from_utf8_lossy(&name)
            )?;
            Ok(true)
        }
    }
}

fn execute_builtin<WO: Write, WE: Write>(
    builtin: BuiltInCommand,
    args: Args,
    out_writer: &mut WO,
    err_writer: &mut WE,
) -> anyhow::Result<bool> {
    let name = builtin.name();
    match name {
        BuiltInName::Exit => return Ok(false),
        BuiltInName::Echo => writeln!(
            out_writer,
            "{}",
            args.iter()
                .map(|a| a.to_string())
                .collect::<Vec<_>>()
                .join(" ")
        )?,
        BuiltInName::Type => {
            if args.is_empty() {
                writeln!(err_writer, "{}: missing operand", name)?;
            } else {
                match Command::from(args.first().expect("at least one arg")) {
                    Command::BuiltIn(builtin) => {
                        writeln!(out_writer, "{} is a shell builtin", builtin)?
                    }
                    Command::Executable(executable) => writeln!(
                        out_writer,
                        "{} is {}",
                        executable,
                        executable.file_path().display()
                    )?,
                    Command::Unrecognized(name) => {
                        writeln!(err_writer, "{}: not found", String::from_utf8_lossy(&name))?
                    }
                }
            }
        }
        BuiltInName::Pwd => writeln!(
            out_writer,
            "{}",
            env::current_dir().unwrap_or_default().display()
        )?,
        BuiltInName::Cd => {
            let default_target = Arg::from(b"~".as_slice());
            let target = args.first().unwrap_or(&default_target);
            let new_dir = fs::resolve_path(&target.into())?;

            if !new_dir.exists() {
                writeln!(
                    err_writer,
                    "{}: no such file or directory: {}",
                    name, target
                )?;
            } else if !new_dir.is_dir() {
                writeln!(err_writer, "{}: not a directory: {}", name, target)?;
            } else {
                match env::set_current_dir(&new_dir) {
                    Ok(()) => {}
                    Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => {
                        writeln!(err_writer, "{}: permission denied: {}", name, target)?;
                    }
                    Err(e) => {
                        writeln!(err_writer, "{}: {}", name, e)?;
                    }
                }
            }
        }
    }

    Ok(true)
}

fn execute_executable<WO: Write, WE: Write>(
    executable: crate::command::executable::ExecutableCommand,
    args: Args,
    out_writer: &mut WO,
    _err_writer: &mut WE,
) -> anyhow::Result<bool> {
    let args = args.iter().map(|a| a.to_string()).collect::<Vec<_>>();
    let mut child = std::process::Command::new(executable.file_path())
        .args(args)
        .spawn()?;
    let status = child.wait()?;

    if !status.success() {
        writeln!(out_writer, "{}: exited with status {}", executable, status)?;
    }

    Ok(true)
}
