use std::path::PathBuf;
use std::str::FromStr;

use is_executable::IsExecutable;

use crate::{
    Arg, Command,
    arg::Args,
    command::builtin::{BuiltInCommand, BuiltInName},
    command::executable::ExecutableCommand,
    ctx::ShellCtx,
    env::get_path_dirs,
    error::{ShellError, ShellResult},
    exit::ExitCode,
    fs,
    parser::Pipeline,
    redirect::{RedirectMode, StderrRedirection, StdoutRedirection},
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

/// Open a redirect target file according to its [`RedirectMode`].
fn open_redirect_file(
    mode: &RedirectMode,
    target: &std::path::Path,
    cwd: &std::path::Path,
) -> ShellResult<std::fs::File> {
    let target_path = if target.is_absolute() {
        target.to_path_buf()
    } else {
        cwd.join(target)
    };
    match mode {
        RedirectMode::Overwrite => {
            std::fs::File::create(&target_path).map_err(ShellError::Io)
        }
        RedirectMode::Append => std::fs::OpenOptions::new()
            .append(true)
            .create(true)
            .open(&target_path)
            .map_err(ShellError::Io),
    }
}

/// Dispatch and execute a parsed command, returning an optional exit code.
///
/// When `stdout_redirect` is `Some`, the command's standard output is written
/// to the named file (overwriting or appending, per [`RedirectMode`]) instead
/// of the shell's normal stdout.  When `stderr_redirect` is `Some`, the
/// command's standard error is redirected likewise.  Any combination of
/// redirects may be present independently.
///
/// Returns `Ok(Some(code))` when the command requests shell exit, `Ok(None)` otherwise.
pub fn execute(
    command: Command,
    args: Args,
    out: &mut dyn std::io::Write,
    err: &mut dyn std::io::Write,
    ctx: &mut ShellCtx,
    stdout_redirect: Option<StdoutRedirection>,
    stderr_redirect: Option<StderrRedirection>,
) -> ShellResult<Option<ExitCode>> {
    let mut stdout_file: Option<std::fs::File> = stdout_redirect
        .as_ref()
        .map(|r| open_redirect_file(&r.mode, &r.target, &ctx.cwd))
        .transpose()?;

    let mut stderr_file: Option<std::fs::File> = stderr_redirect
        .as_ref()
        .map(|r| open_redirect_file(&r.mode, &r.target, &ctx.cwd))
        .transpose()?;

    let eff_out: &mut dyn std::io::Write = match stdout_file.as_mut() {
        Some(f) => f,
        None => out,
    };
    let eff_err: &mut dyn std::io::Write = match stderr_file.as_mut() {
        Some(f) => f,
        None => err,
    };
    execute_with_writers(command, args, eff_out, eff_err, ctx)
}

/// Execute a [`Pipeline`] (one or more `|`-connected commands).
///
/// A single-stage pipeline delegates to [`execute`] unchanged.  A multi-stage
/// pipeline runs each stage serially: the stdout of stage *i* is buffered then
/// fed as stdin to stage *i+1*.  For executable stages the data is written into
/// the child's stdin pipe; built-in stages do not currently consume piped stdin
/// data.  The last stage's stdout is written to `io` (or to a file if a
/// per-stage redirect is present).
///
/// Stdout buffering is intentional for this implementation — issue #28 will
/// replace it with OS-level streaming pipes between concurrent processes.
///
/// Returns `Ok(Some(code))` when any stage requests shell exit, `Ok(None)` otherwise.
pub fn execute_pipeline(
    pipeline: Pipeline,
    out: &mut dyn std::io::Write,
    err: &mut dyn std::io::Write,
    ctx: &mut ShellCtx,
) -> ShellResult<Option<ExitCode>> {
    // Fast path: no pipe operators.
    if pipeline.len() == 1 {
        let (cmd, args, stdout_redir, stderr_redir) = pipeline.into_iter().next().unwrap();
        return execute(cmd, args, out, err, ctx, stdout_redir, stderr_redir);
    }

    // Multi-stage: carry the previous stage's captured stdout forward.
    let mut stdin_buf: Option<Vec<u8>> = None;
    let last_idx = pipeline.len() - 1;

    for (i, (command, args, stdout_redirect, stderr_redirect)) in pipeline.into_iter().enumerate() {
        let is_last = i == last_idx;

        if is_last {
            return if let Some(buf) = stdin_buf.take() {
                execute_stage_with_stdin(
                    command, args, out, err, ctx, stdout_redirect, stderr_redirect, &buf,
                )
            } else {
                execute(command, args, out, err, ctx, stdout_redirect, stderr_redirect)
            };
        }

        let mut out_buf: Vec<u8> = Vec::new();
        let prev = stdin_buf.take();
        // Fail-fast: any non-zero exit from an intermediate stage aborts the pipeline.
        // Issue #28 tracks replacing this with concurrent OS-level pipes.
        execute_stage_capture(
            command, args, err, ctx, stdout_redirect, stderr_redirect, prev.as_deref(), &mut out_buf,
        )?;
        stdin_buf = Some(out_buf);
    }

    Ok(None)
}

/// Run one intermediate pipeline stage, feeding `stdin_data` as the command's
/// stdin.  If `stdout_redirect` is `Some`, stdout goes to that file (POSIX
/// semantics); otherwise it is captured into `out_buf` for the next stage.
/// Stderr honours `stderr_redirect` or falls back to `io`.
#[allow(clippy::too_many_arguments)]
fn execute_stage_capture(
    command: Command,
    args: Args,
    err: &mut dyn std::io::Write,
    ctx: &mut ShellCtx,
    stdout_redirect: Option<StdoutRedirection>,
    stderr_redirect: Option<StderrRedirection>,
    stdin_data: Option<&[u8]>,
    out_buf: &mut Vec<u8>,
) -> ShellResult<Option<ExitCode>> {
    let mut stdout_file: Option<std::fs::File> = stdout_redirect
        .as_ref()
        .map(|r| open_redirect_file(&r.mode, &r.target, &ctx.cwd))
        .transpose()?;

    let mut stderr_file: Option<std::fs::File> = stderr_redirect
        .as_ref()
        .map(|r| open_redirect_file(&r.mode, &r.target, &ctx.cwd))
        .transpose()?;

    let out_writer: &mut dyn std::io::Write = match stdout_file.as_mut() {
        Some(f) => f,
        None => out_buf,
    };
    let err_writer: &mut dyn std::io::Write = match stderr_file.as_mut() {
        Some(f) => f,
        None => err,
    };

    execute_stage_inner(command, args, out_writer, err_writer, ctx, stdin_data)
}

/// Run the last pipeline stage, feeding `stdin_data` and honouring redirects.
#[allow(clippy::too_many_arguments)]
fn execute_stage_with_stdin(
    command: Command,
    args: Args,
    out: &mut dyn std::io::Write,
    err: &mut dyn std::io::Write,
    ctx: &mut ShellCtx,
    stdout_redirect: Option<StdoutRedirection>,
    stderr_redirect: Option<StderrRedirection>,
    stdin_data: &[u8],
) -> ShellResult<Option<ExitCode>> {
    let mut stdout_file: Option<std::fs::File> = stdout_redirect
        .as_ref()
        .map(|r| open_redirect_file(&r.mode, &r.target, &ctx.cwd))
        .transpose()?;

    let mut stderr_file: Option<std::fs::File> = stderr_redirect
        .as_ref()
        .map(|r| open_redirect_file(&r.mode, &r.target, &ctx.cwd))
        .transpose()?;

    let out_writer: &mut dyn std::io::Write = match stdout_file.as_mut() {
        Some(f) => f,
        None => out,
    };
    let err_writer: &mut dyn std::io::Write = match stderr_file.as_mut() {
        Some(f) => f,
        None => err,
    };

    execute_stage_inner(command, args, out_writer, err_writer, ctx, Some(stdin_data))
}

/// Core dispatch for a pipeline stage: routes to builtin or executable, feeding
/// `stdin_data` (if any) as the command's standard input.
fn execute_stage_inner(
    command: Command,
    args: Args,
    out: &mut dyn std::io::Write,
    err: &mut dyn std::io::Write,
    ctx: &mut ShellCtx,
    stdin_data: Option<&[u8]>,
) -> ShellResult<Option<ExitCode>> {
    let result = match &command {
        Command::BuiltIn(builtin) => {
            // Builtins don't consume stdin from a pipe — they ignore it.
            execute_builtin(builtin, args, out, err, ctx)
        }
        Command::Executable(executable) => {
            execute_executable(executable, args, out, err, ctx, stdin_data)
        }
        Command::Unrecognized(cmd) => {
            return Err(ShellError::CommandNotFound {
                name: String::from_utf8_lossy(cmd).into_owned(),
            })
        }
    };
    result.map_err(|e| ShellError::InCommand { command, source: Box::new(e) })
}

/// Core dispatch: all output goes to the provided `out` and `err` writers.
///
/// This separation makes it possible to redirect stdout to a file while keeping
/// stderr on the terminal without changing the `ShellIo` abstraction.
fn execute_with_writers(
    command: Command,
    args: Args,
    out: &mut dyn std::io::Write,
    err: &mut dyn std::io::Write,
    ctx: &mut ShellCtx,
) -> ShellResult<Option<ExitCode>> {
    let result = match &command {
        Command::BuiltIn(builtin) => execute_builtin(builtin, args, out, err, ctx),
        Command::Executable(executable) => execute_executable(executable, args, out, err, ctx, None),
        Command::Unrecognized(cmd) => {
            return Err(ShellError::CommandNotFound {
                name: String::from_utf8_lossy(cmd).into_owned(),
            })
        }
    };
    result.map_err(|e| ShellError::InCommand { command, source: Box::new(e) })
}

fn execute_builtin(
    builtin: &BuiltInCommand,
    args: Args,
    out: &mut dyn std::io::Write,
    err: &mut dyn std::io::Write,
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
                            writeln!(err, "exit: {}: numeric argument required", s)?;
                            return Ok(Some(ExitCode::FAILURE));
                        }
                    }
                }
            };
            return Ok(Some(code));
        }
        BuiltInName::Cd => execute_cd(args, ctx)?,
        BuiltInName::Echo => execute_echo(args, out)?,
        BuiltInName::Type => execute_type(args, out)?,
        BuiltInName::Pwd => execute_pwd(args, out, ctx)?,
    }

    Ok(None)
}

fn execute_cd(args: Args, ctx: &mut ShellCtx) -> ShellResult<()> {
    let default_target = Arg::from(b"~".as_slice());
    let target = args.first().unwrap_or(&default_target);
    let new_dir = fs::resolve_path(&target.into(), ctx.home_dir.as_deref(), &ctx.cwd)?;

    if !new_dir.exists() {
        return Err(ShellError::FileNotFound { arg: target.clone() });
    } else if !new_dir.is_dir() {
        return Err(ShellError::NotADirectory { arg: target.clone() });
    }

    ctx.cwd = new_dir;

    Ok(())
}

fn execute_echo(args: Args, out: &mut dyn std::io::Write) -> ShellResult<()> {
    writeln!(
        out,
        "{}",
        args.iter()
            .map(|a| a.to_string())
            .collect::<Vec<_>>()
            .join(" ")
    )?;

    Ok(())
}

fn execute_type(args: Args, out: &mut dyn std::io::Write) -> ShellResult<()> {
    if args.is_empty() {
        return Err(ShellError::MissingOperand);
    }

    let arg = args.first().expect("at least one arg");
    let name = arg.as_bytes();

    match resolve_command_type(name) {
        CommandKind::Builtin(builtin_name) => {
            writeln!(out, "{} is a shell builtin", builtin_name)?
        }
        CommandKind::Executable(path) => {
            let display_name = path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("");
            writeln!(out, "{} is {}", display_name, path.display())?
        }
        CommandKind::NotFound => return Err(ShellError::CommandNotFound { name: arg.to_string() }),
    }

    Ok(())
}

fn execute_pwd(
    _args: Args,
    out: &mut dyn std::io::Write,
    ctx: &ShellCtx,
) -> ShellResult<()> {
    writeln!(out, "{}", ctx.cwd.display())?;
    Ok(())
}

fn execute_executable(
    executable: &ExecutableCommand,
    args: Args,
    out: &mut dyn std::io::Write,
    err: &mut dyn std::io::Write,
    ctx: &ShellCtx,
    stdin_data: Option<&[u8]>,
) -> ShellResult<Option<ExitCode>> {
    use std::io::Write as _;
    use std::process::Stdio;
    use std::thread;

    let args = args.iter().map(|a| a.to_string()).collect::<Vec<_>>();
    let mut child = std::process::Command::new(executable.file_path())
        .args(args)
        .current_dir(&ctx.cwd)
        .stdin(if stdin_data.is_some() { Stdio::piped() } else { Stdio::inherit() })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(ShellError::ExecutionFailed)?;

    // Spawn a stdin writer before draining stdout/stderr to avoid deadlock:
    // the child may fill its stdout buffer before reading all stdin, so we
    // must drain both concurrently.  `ChildStdin`/`ChildStdout`/`ChildStderr`
    // are `Send`, so they can be moved into threads safely.
    let stdin_thread = stdin_data
        .zip(child.stdin.take())
        .map(|(data, mut child_stdin)| {
            let data = data.to_vec();
            thread::spawn(move || -> std::io::Result<()> {
                // BrokenPipe means the child exited before reading all stdin — treat
                // as success so a fast downstream command (e.g. `head -1`) doesn't
                // surface a spurious I/O error.
                if let Err(e) = child_stdin.write_all(&data)
                    && e.kind() != std::io::ErrorKind::BrokenPipe
                {
                    return Err(e);
                }
                Ok(()) // drop closes the pipe
            })
        });

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

    if let Some(handle) = stdin_thread {
        handle
            .join()
            .unwrap_or_else(|_| Err(std::io::Error::other("stdin thread panicked")))
            .map_err(ShellError::Io)?;
    }

    if let Some(handle) = stdout_thread {
        let bytes = join_io_thread(handle, "stdout")?;
        out.write_all(&bytes)?;
    }
    if let Some(handle) = stderr_thread {
        let bytes = join_io_thread(handle, "stderr")?;
        err.write_all(&bytes)?;
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

    #[test]
    fn join_io_thread_panic_becomes_non_fatal_io_error() {
        let handle = std::thread::spawn(|| -> std::io::Result<Vec<u8>> { panic!("simulated OOM") });
        let result = join_io_thread(handle, "stdout");
        let err = result.expect_err("expected Err from panicking thread");
        assert!(!err.is_fatal());
        assert!(matches!(err, ShellError::Io(_)));
        assert!(err.to_string().contains("stdout"));
    }
}
