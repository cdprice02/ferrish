use std::io::{PipeReader, PipeWriter, Write};
use std::path::PathBuf;
use std::process::{Child, Stdio};
use std::str::FromStr;
use std::thread::{self, JoinHandle};

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

/// Resolved stdout destination for one pipeline stage.
enum StageOutput {
    /// Intermediate stage: output feeds the next stage's stdin.
    Pipe(PipeWriter),
    /// Any stage with an explicit stdout redirect.
    File(std::fs::File),
    /// Last stage, no redirect: use the shell's output writer.
    Terminal,
}

/// Resolved stderr destination for one pipeline stage.
enum StageError {
    /// Stage with an explicit stderr redirect.
    File(std::fs::File),
    /// No redirect: write to the shell's error writer (builtins) or inherit
    /// the process stderr (executables).
    Terminal,
}

/// Handle returned after launching one pipeline stage.
enum StageHandle {
    /// A spawned OS process.
    Process(Child),
    /// A builtin running in a background thread (intermediate stage).
    Thread(JoinHandle<ShellResult<Option<ExitCode>>>),
    /// A builtin that ran synchronously (last-stage Terminal output).
    Done(ShellResult<Option<ExitCode>>),
}

/// Single dispatch for launching any pipeline stage — builtin or executable.
///
/// Builtins at intermediate positions (non-Terminal stdout) run in a thread
/// with an isolated `ctx` clone so their side-effects (e.g. `cd`) don't
/// propagate back — matching POSIX subshell semantics for pipeline stages.
/// Builtins at the last stage (Terminal stdout) run synchronously.
/// Executables always spawn a child process.
#[allow(clippy::too_many_arguments)]
fn launch_stage(
    command: Command,
    args: Args,
    stdin: Option<PipeReader>,
    stdout: StageOutput,
    stderr: StageError,
    shell_out: &mut dyn Write,
    shell_err: &mut dyn Write,
    ctx: &mut ShellCtx,
) -> ShellResult<StageHandle> {
    match command {
        Command::BuiltIn(builtin) => {
            drop(stdin); // builtins ignore piped stdin; the closed read end
                         // sends SIGPIPE/EOF upstream, which is correct

            // Last stage with no stdout redirect: run synchronously so we can
            // write directly to the shell's `out`/`err` writers (which are not
            // `Send` and cannot be moved into a thread).
            if matches!(stdout, StageOutput::Terminal) {
                let mut err_file: Option<std::fs::File> = match stderr {
                    StageError::File(f) => Some(f),
                    StageError::Terminal => None,
                };
                let eff_err: &mut dyn Write = match err_file.as_mut() {
                    Some(f) => f,
                    None => shell_err,
                };
                let result = execute_builtin(&builtin, args, shell_out, eff_err, ctx)
                    .map_err(|e| ShellError::InCommand {
                        command: Command::BuiltIn(builtin.clone()),
                        source: Box::new(e),
                    });
                return Ok(StageHandle::Done(result));
            }

            let stdout_writer: Box<dyn Write + Send> = match stdout {
                StageOutput::Pipe(pw) => Box::new(pw),
                StageOutput::File(f) => Box::new(f),
                StageOutput::Terminal => unreachable!(),
            };
            let stderr_writer: Box<dyn Write + Send> = match stderr {
                StageError::File(f) => Box::new(f),
                StageError::Terminal => Box::new(std::io::stderr()),
            };

            let mut ctx_clone = ctx.clone();
            let handle = thread::spawn(move || {
                let mut out = stdout_writer;
                let mut err = stderr_writer;
                execute_builtin(&builtin, args, &mut *out, &mut *err, &mut ctx_clone)
                    .map_err(|e| ShellError::InCommand {
                        command: Command::BuiltIn(builtin),
                        source: Box::new(e),
                    })
            });
            Ok(StageHandle::Thread(handle))
        }

        Command::Executable(executable) => {
            let stdin_stdio = stdin.map_or(Stdio::inherit(), Stdio::from);
            let stdout_stdio = match stdout {
                StageOutput::Pipe(pw) => Stdio::from(pw),
                StageOutput::File(f) => Stdio::from(f),
                StageOutput::Terminal => Stdio::piped(),
            };
            let stderr_stdio = match stderr {
                StageError::File(f) => Stdio::from(f),
                StageError::Terminal => Stdio::piped(),
            };

            let args_strs: Vec<String> = args.iter().map(|a| a.to_string()).collect();
            let spawn_result = std::process::Command::new(executable.file_path())
                .args(args_strs)
                .current_dir(&ctx.cwd)
                .stdin(stdin_stdio)
                .stdout(stdout_stdio)
                .stderr(stderr_stdio)
                .spawn()
                .map_err(ShellError::ExecutionFailed);

            spawn_result
                .map(StageHandle::Process)
                .map_err(|e| ShellError::InCommand {
                    command: Command::Executable(executable),
                    source: Box::new(e),
                })
        }

        Command::Unrecognized(cmd) => Err(ShellError::CommandNotFound {
            name: String::from_utf8_lossy(&cmd).into_owned(),
        }),
    }
}

/// Spawn a thread that drains `pipe` into a `Vec<u8>`.
fn spawn_drain_thread<R: std::io::Read + Send + 'static>(
    mut pipe: R,
) -> JoinHandle<std::io::Result<Vec<u8>>> {
    thread::spawn(move || {
        let mut buf = Vec::new();
        std::io::copy(&mut pipe, &mut buf)?;
        Ok(buf)
    })
}

/// Wait for a child process, drain any piped stdout/stderr into `out`/`err`,
/// and return the exit result.
fn drain_and_wait(
    mut child: Child,
    stdout_drain: Option<JoinHandle<std::io::Result<Vec<u8>>>>,
    stderr_drain: Option<JoinHandle<std::io::Result<Vec<u8>>>>,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> ShellResult<Option<ExitCode>> {
    let status = child.wait().map_err(ShellError::ExecutionFailed)?;
    if let Some(t) = stdout_drain {
        out.write_all(&join_io_thread(t, "stdout")?)?;
    }
    if let Some(t) = stderr_drain {
        err.write_all(&join_io_thread(t, "stderr")?)?;
    }
    if !status.success() {
        return Err(ShellError::NonZeroExit(status));
    }
    Ok(None)
}

/// Collect the result of a single (non-last) pipeline stage.
/// Intermediate process stderr is piped — drain it and discard (it already
/// went to the child's own stderr fd chain; we don't re-route it here).
fn wait_one(handle: StageHandle) -> ShellResult<Option<ExitCode>> {
    match handle {
        StageHandle::Done(r) => r,
        StageHandle::Thread(h) => h.join().unwrap_or_else(|_| {
            Err(ShellError::Io(std::io::Error::other(
                "pipeline stage thread panicked",
            )))
        }),
        StageHandle::Process(mut child) => {
            // Drain and discard intermediate stderr so the child doesn't block
            // writing to a full pipe buffer. We don't re-route it because
            // intermediate stages' stderr goes directly to the terminal via
            // the process hierarchy (the parent ferrish process's stderr fd).
            // Actually: we piped it above, so we must drain it to avoid
            // blocking the child. Write it through to the real stderr.
            let stderr_drain = child.stderr.take().map(spawn_drain_thread);
            let status = child.wait().map_err(ShellError::ExecutionFailed)?;
            if let Some(t) = stderr_drain {
                // Write intermediate stage stderr to real process stderr
                std::io::stderr().write_all(&join_io_thread(t, "stderr")?)?;
            }
            if status.success() {
                Ok(None)
            } else {
                Err(ShellError::NonZeroExit(status))
            }
        }
    }
}

/// Wait for all launched pipeline stages and collect their results.
///
/// Drain threads for the last `Process` stage are started *before* waiting
/// on intermediate stages to prevent deadlock: intermediate processes write
/// to OS pipes that the last process reads; if the last process's piped stdout
/// buffer fills up before we drain it, the whole chain stalls.
fn wait_pipeline(
    handles: Vec<StageHandle>,
    shell_out: &mut dyn Write,
    shell_err: &mut dyn Write,
) -> ShellResult<Option<ExitCode>> {
    let mut handles = handles;
    let last = handles.pop().expect("non-empty pipeline checked at call site");

    // Pre-start drain threads for the last Process's stdout and stderr before
    // waiting on intermediate stages.
    let (last_process, last_stdout_drain, last_stderr_drain) = match last {
        StageHandle::Process(mut c) => {
            let stdout_drain = c.stdout.take().map(spawn_drain_thread);
            let stderr_drain = c.stderr.take().map(spawn_drain_thread);
            (Some(c), stdout_drain, stderr_drain)
        }
        other => {
            handles.push(other);
            (None, None, None)
        }
    };

    let last_non_process = if last_process.is_none() {
        handles.pop()
    } else {
        None
    };

    let mut first_error: Option<ShellError> = None;
    let mut exit_request: Option<ExitCode> = None;

    macro_rules! record {
        ($result:expr) => {
            match $result {
                Ok(Some(code)) => exit_request = Some(code),
                Ok(None) => {}
                Err(e) if first_error.is_none() => first_error = Some(e),
                Err(_) => {}
            }
        };
    }

    // Wait for all intermediate stages.
    for handle in handles {
        record!(wait_one(handle));
    }

    // Collect the last stage result.
    if let Some(mut child) = last_process {
        let status = child.wait().map_err(ShellError::ExecutionFailed)?;
        if let Some(t) = last_stdout_drain {
            shell_out.write_all(&join_io_thread(t, "stdout")?)?;
        }
        if let Some(t) = last_stderr_drain {
            shell_err.write_all(&join_io_thread(t, "stderr")?)?;
        }
        if !status.success() && first_error.is_none() {
            first_error = Some(ShellError::NonZeroExit(status));
        }
    } else if let Some(handle) = last_non_process {
        record!(wait_one(handle));
    }

    if let Some(e) = first_error {
        Err(e)
    } else {
        Ok(exit_request)
    }
}

/// Dispatch and execute a parsed command, returning an optional exit code.
pub fn execute(
    command: Command,
    args: Args,
    out: &mut dyn Write,
    err: &mut dyn Write,
    ctx: &mut ShellCtx,
    stdout_redirect: Option<StdoutRedirection>,
    stderr_redirect: Option<StderrRedirection>,
) -> ShellResult<Option<ExitCode>> {
    let stdout = if let Some(r) = stdout_redirect {
        StageOutput::File(open_redirect_file(&r.mode, &r.target, &ctx.cwd)?)
    } else {
        StageOutput::Terminal
    };
    let stderr = if let Some(r) = stderr_redirect {
        StageError::File(open_redirect_file(&r.mode, &r.target, &ctx.cwd)?)
    } else {
        StageError::Terminal
    };

    let handle = launch_stage(command, args, None, stdout, stderr, out, err, ctx)?;
    match handle {
        StageHandle::Done(r) => r,
        StageHandle::Thread(h) => h.join().unwrap_or_else(|_| {
            Err(ShellError::Io(std::io::Error::other(
                "builtin thread panicked",
            )))
        }),
        StageHandle::Process(mut child) => {
            let stdout_drain = child.stdout.take().map(spawn_drain_thread);
            let stderr_drain = child.stderr.take().map(spawn_drain_thread);
            drain_and_wait(child, stdout_drain, stderr_drain, out, err)
        }
    }
}

/// Execute a [`Pipeline`] (one or more `|`-connected commands).
///
/// A single-stage pipeline delegates to [`execute`] unchanged. A multi-stage
/// pipeline creates N-1 OS-level pipes, launches all N stages concurrently
/// (executables as child processes, builtins in threads), then waits for all.
/// Data flows directly between processes via kernel pipe buffers — no
/// in-memory buffering between stages.
///
/// Returns `Ok(Some(code))` when any stage requests shell exit, `Ok(None)` otherwise.
pub fn execute_pipeline(
    pipeline: Pipeline,
    out: &mut dyn Write,
    err: &mut dyn Write,
    ctx: &mut ShellCtx,
) -> ShellResult<Option<ExitCode>> {
    if pipeline.len() == 1 {
        let (cmd, args, stdout_redir, stderr_redir) = pipeline.into_iter().next().unwrap();
        return execute(cmd, args, out, err, ctx, stdout_redir, stderr_redir);
    }

    let stages: Vec<_> = pipeline.into_iter().collect();
    let n = stages.len();

    // Create N-1 inter-stage pipes.  Both ends are consumed by launch_stage
    // (moved into Stdio::from or a thread closure), so the parent holds no
    // open pipe ends after the launch loop — no manual cleanup needed.
    let pipe_pairs: std::io::Result<Vec<_>> = (0..n - 1).map(|_| std::io::pipe()).collect();
    let (mut readers, mut writers): (Vec<_>, Vec<_>) = pipe_pairs
        .map_err(ShellError::Io)?
        .into_iter()
        .map(|(r, w)| (Some(r), Some(w)))
        .unzip();

    let mut handles = Vec::with_capacity(n);

    for (i, (command, args, stdout_redirect, stderr_redirect)) in
        stages.into_iter().enumerate()
    {
        let stdin: Option<PipeReader> = if i == 0 { None } else { readers[i - 1].take() };

        let stdout = if let Some(r) = stdout_redirect {
            StageOutput::File(open_redirect_file(&r.mode, &r.target, &ctx.cwd)?)
        } else if i < n - 1 {
            StageOutput::Pipe(writers[i].take().unwrap())
        } else {
            StageOutput::Terminal
        };

        let stderr = if let Some(r) = stderr_redirect {
            StageError::File(open_redirect_file(&r.mode, &r.target, &ctx.cwd)?)
        } else {
            StageError::Terminal
        };

        handles.push(launch_stage(command, args, stdin, stdout, stderr, out, err, ctx)?);
    }

    wait_pipeline(handles, out, err)
}

fn execute_builtin(
    builtin: &BuiltInCommand,
    args: Args,
    out: &mut dyn Write,
    err: &mut dyn Write,
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

fn execute_echo(args: Args, out: &mut dyn Write) -> ShellResult<()> {
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

fn execute_type(args: Args, out: &mut dyn Write) -> ShellResult<()> {
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
    out: &mut dyn Write,
    ctx: &ShellCtx,
) -> ShellResult<()> {
    writeln!(out, "{}", ctx.cwd.display())?;
    Ok(())
}

/// Join an I/O drain thread and convert both join-failure and I/O errors into
/// a non-fatal [`ShellError::Io`].  `stream` names the pipe ("stdout"/"stderr")
/// and is included in the panic-message for easier debugging.
fn join_io_thread(
    handle: JoinHandle<std::io::Result<Vec<u8>>>,
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
