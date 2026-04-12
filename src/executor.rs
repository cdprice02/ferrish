use std::path::PathBuf;
use std::str::FromStr;

use is_executable::IsExecutable;

use crate::{
    Arg, Command,
    arg::Args,
    command::builtin::{BuiltInCommand, BuiltInName},
    ctx::ShellCtx,
    env::get_path_dirs,
    error::{ShellError, ShellResult},
    exit::ExitCode,
    fs,
    io::ShellIo,
};

enum CommandKind {
    Builtin(BuiltInName),
    Executable(PathBuf),
    NotFound,
}

fn resolve_command_type(name: &[u8]) -> CommandKind {
    if !name.is_ascii() {
        return CommandKind::NotFound;
    }

    let name_str = std::str::from_utf8(name).expect("checked ASCII above");

    if let Ok(builtin_name) = BuiltInName::from_str(name_str) {
        return CommandKind::Builtin(builtin_name);
    }

    for dir in get_path_dirs() {
        let candidate = dir.join(name_str);
        if candidate.is_executable() {
            return CommandKind::Executable(candidate);
        }
        // On Windows executables typically carry a .exe extension
        #[cfg(windows)]
        {
            let candidate_exe = dir.join(format!("{name_str}.exe"));
            if candidate_exe.is_executable() {
                return CommandKind::Executable(candidate_exe);
            }
        }
    }

    CommandKind::NotFound
}

/// Dispatch and execute a parsed command, returning an optional exit code.
///
/// Returns `Ok(Some(code))` when the command requests shell exit, `Ok(None)` otherwise.
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
            let code = match args.first() {
                None => ExitCode::SUCCESS,
                Some(arg) => {
                    let s = arg.to_string();
                    match s.parse::<u8>() {
                        Ok(n) => ExitCode(n),
                        Err(_) => {
                            writeln!(io.err_writer(), "exit: {}: numeric argument required", s)?;
                            return Ok(Some(ExitCode::FAILURE));
                        }
                    }
                }
            };
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

    let arg = args.first().expect("at least one arg");
    let name = arg.as_bytes();

    match resolve_command_type(name) {
        CommandKind::Builtin(builtin_name) => {
            writeln!(io.out_writer(), "{} is a shell builtin", builtin_name)?
        }
        CommandKind::Executable(path) => {
            let display_name = path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("");
            writeln!(io.out_writer(), "{} is {}", display_name, path.display())?
        }
        CommandKind::NotFound => return Err(ShellError::CommandNotFound),
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
    use std::process::Stdio;
    use std::thread;

    let args = args.iter().map(|a| a.to_string()).collect::<Vec<_>>();
    let mut child = std::process::Command::new(executable.file_path())
        .args(args)
        .current_dir(&ctx.cwd)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(ShellError::ExecutionFailed)?;

    // Drain both pipes concurrently on separate threads to avoid deadlock:
    // if the child fills one pipe's buffer while we are blocked reading the
    // other, neither side can make progress.  `ChildStdout`/`ChildStderr` are
    // `Send`, so they can be moved into threads safely.  We accumulate bytes
    // into `Vec<u8>` and write them into `ShellIo` after joining — keeping
    // the `?Send` `ShellIo` exclusively on the calling thread.
    let stdout_thread = child.stdout.take().map(|mut pipe| {
        thread::spawn(move || -> std::io::Result<Vec<u8>> {
            let mut buf = Vec::new();
            std::io::copy(&mut pipe, &mut buf)?;
            Ok(buf)
        })
    });
    let stderr_thread = child.stderr.take().map(|mut pipe| {
        thread::spawn(move || -> std::io::Result<Vec<u8>> {
            let mut buf = Vec::new();
            std::io::copy(&mut pipe, &mut buf)?;
            Ok(buf)
        })
    });

    // Always wait on the child so it is reaped even when I/O copy fails.
    let status = child.wait().map_err(ShellError::ExecutionFailed)?;

    // Collect thread results; propagate the first I/O error encountered.
    if let Some(handle) = stdout_thread {
        let bytes = join_io_thread(handle, "stdout")?;
        io.out_writer().write_all(&bytes)?;
    }
    if let Some(handle) = stderr_thread {
        let bytes = join_io_thread(handle, "stderr")?;
        io.err_writer().write_all(&bytes)?;
    }

    if !status.success() {
        return Err(ShellError::NonZeroExit(status));
    }

    Ok(None)
}

/// Join an I/O drain thread and convert both join-failure and I/O errors into
/// a non-fatal [`ShellError::Io`].  `stream` names the pipe ("stdout"/"stderr")
/// and is included in the panic-message for easier debugging.
fn join_io_thread(
    handle: std::thread::JoinHandle<std::io::Result<Vec<u8>>>,
    stream: &str,
) -> ShellResult<Vec<u8>> {
    handle
        .join()
        .unwrap_or_else(|_| {
            Err(std::io::Error::other(format!(
                "{stream} I/O thread panicked"
            )))
        })
        .map_err(ShellError::Io)
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

    #[cfg(unix)]
    #[test]
    fn test_execute_type_executable() {
        use std::fs;
        use std::os::unix::fs::PermissionsExt;

        let dir = tempfile::tempdir().expect("create temp dir");
        let bin_path = dir.path().join("my_test_tool");
        fs::write(&bin_path, b"#!/bin/sh\n").expect("write fake executable");
        let mut perms = fs::metadata(&bin_path).unwrap().permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&bin_path, perms).expect("set executable bit");

        let original_path = std::env::var_os("PATH").unwrap_or_default();
        let mut new_dirs: Vec<PathBuf> = vec![dir.path().to_path_buf()];
        new_dirs.extend(std::env::split_paths(&original_path));
        let new_path = std::env::join_paths(&new_dirs).expect("join paths");
        // SAFETY: test-only, single-threaded context
        unsafe { std::env::set_var("PATH", &new_path) };

        let args = vec![Arg::from("my_test_tool")];
        let mut io = MockIo::empty();
        let result = execute_type(args, &mut io);

        // SAFETY: test-only, single-threaded context
        unsafe { std::env::set_var("PATH", &original_path) };

        result.unwrap();
        let output = String::from_utf8(io.output().to_vec()).unwrap();
        assert!(
            output.contains("my_test_tool is"),
            "unexpected output: {output}"
        );
        assert!(
            output.contains(bin_path.to_str().unwrap()),
            "path not in output: {output}"
        );
    }

    #[test]
    fn test_join_io_thread_panic_is_non_fatal_io_error() {
        // Verify that a panicking I/O thread produces a non-fatal ShellError::Io
        // that includes the stream name, rather than propagating a process panic.
        let handle = std::thread::spawn(|| -> std::io::Result<Vec<u8>> {
            panic!("simulated OOM");
        });
        // Suppress the panic output from the child thread in test output.
        let result = join_io_thread(handle, "stdout");
        let err = result.expect_err("expected Err from panicking thread");
        assert!(!err.is_fatal(), "join panic must not be fatal");
        assert!(
            matches!(err, ShellError::Io(_)),
            "expected ShellError::Io, got {err:?}"
        );
        assert!(
            err.to_string().contains("stdout"),
            "error message should name the stream: {err}"
        );
    }
}
