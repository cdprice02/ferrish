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
        RedirectMode::Overwrite => std::fs::File::create(&target_path).map_err(ShellError::Io),
        RedirectMode::Append => std::fs::OpenOptions::new()
            .append(true)
            .create(true)
            .open(&target_path)
            .map_err(ShellError::Io),
    }
}

/// Resolved stdout destination for one pipeline stage.
enum StageOutput {
    /// Inter-stage stdout: write end feeds the next stage's stdin.
    Pipe(PipeWriter),
    /// Explicit stdout redirect: write directly to this file.
    File(std::fs::File),
    /// No redirect (last stage or single command): inherit the process stdout fd.
    Inherit,
}

/// Resolved stderr destination for one pipeline stage.
enum StageError {
    /// Explicit stderr redirect: write directly to this file.
    File(std::fs::File),
    /// No redirect: inherit the process stderr fd. Applies to all stages.
    Inherit,
}

/// Handle returned after launching one pipeline stage.
enum StageHandle {
    /// A builtin running in a background thread.
    Thread(JoinHandle<ShellResult<Option<ExitCode>>>, Command),
    /// A spawned OS process.
    Process(Child, Command),
}


/// Single dispatch for launching any pipeline stage — builtin or executable.
///
/// Both kinds receive resolved IO destinations via [`StageOutput`] and
/// [`StageError`]. Builtins always run in a thread with a cloned context
/// (POSIX subshell semantics — side-effects like `cd` don't propagate back).
/// Executables spawn a child process. The caller waits for the returned
/// [`StageHandle`]; output flows directly through inherited fds or pipe ends
/// with no intermediate buffering.
fn launch_stage(
    command: Command,
    args: Args,
    stdin: Option<PipeReader>,
    stdout: StageOutput,
    stderr: StageError,
    ctx: &mut ShellCtx,
) -> ShellResult<StageHandle> {
    match command {
        Command::BuiltIn(builtin) => {
            let cmd_for_handle = Command::BuiltIn(builtin.clone());
            let mut out_w: Box<dyn Write + Send> = match stdout {
                StageOutput::Pipe(pw) => Box::new(pw),
                StageOutput::File(f) => Box::new(f),
                StageOutput::Inherit => Box::new(std::io::stdout()),
            };
            let mut err_w: Box<dyn Write + Send> = match stderr {
                StageError::File(f) => Box::new(f),
                StageError::Inherit => Box::new(std::io::stderr()),
            };
            let mut ctx_clone = ctx.clone();
            let h = thread::spawn(move || {
                execute_builtin(&builtin, args, stdin, &mut *out_w, &mut *err_w, &mut ctx_clone)
                    .map_err(|e| ShellError::InCommand {
                        command: Command::BuiltIn(builtin),
                        source: Box::new(e),
                    })
            });
            Ok(StageHandle::Thread(h, cmd_for_handle))
        }

        Command::Executable(executable) => {
            let stdin_stdio = stdin.map_or(Stdio::inherit(), Stdio::from);
            let stdout_stdio = match stdout {
                StageOutput::Pipe(pw) => Stdio::from(pw),
                StageOutput::File(f) => Stdio::from(f),
                StageOutput::Inherit => Stdio::inherit(),
            };
            let stderr_stdio = match stderr {
                StageError::File(f) => Stdio::from(f),
                StageError::Inherit => Stdio::inherit(),
            };
            let args_strs: Vec<String> = args.iter().map(|a| a.to_string()).collect();
            match std::process::Command::new(executable.file_path())
                .args(args_strs)
                .current_dir(&ctx.cwd)
                .stdin(stdin_stdio)
                .stdout(stdout_stdio)
                .stderr(stderr_stdio)
                .spawn()
            {
                Ok(child) => Ok(StageHandle::Process(child, Command::Executable(executable))),
                Err(e) => Err(ShellError::InCommand {
                    command: Command::Executable(executable),
                    source: Box::new(ShellError::ExecutionFailed(e)),
                }),
            }
        }

        Command::Unrecognized(cmd) => Err(ShellError::CommandNotFound {
            name: String::from_utf8_lossy(&cmd).into_owned(),
        }),
    }
}

/// Dispatch and execute a single parsed command with optional redirects.
///
/// Builtins run synchronously in the caller's `ctx` so state-mutating commands
/// like `cd` propagate back to the shell — matching POSIX semantics for
/// commands that are not part of a multi-command pipeline.
pub fn execute(
    command: Command,
    args: Args,
    ctx: &mut ShellCtx,
    stdout_redirect: Option<StdoutRedirection>,
    stderr_redirect: Option<StderrRedirection>,
) -> ShellResult<Option<ExitCode>> {
    let mut out_file: Option<std::fs::File> = stdout_redirect
        .map(|r| open_redirect_file(&r.mode, &r.target, &ctx.cwd))
        .transpose()?;
    let mut err_file: Option<std::fs::File> = stderr_redirect
        .map(|r| open_redirect_file(&r.mode, &r.target, &ctx.cwd))
        .transpose()?;

    match command {
        Command::BuiltIn(builtin) => {
            let mut stdout_default = std::io::stdout();
            let mut stderr_default = std::io::stderr();
            let out: &mut dyn Write = out_file.as_mut().map_or(&mut stdout_default as _, |f| f);
            let err: &mut dyn Write = err_file.as_mut().map_or(&mut stderr_default as _, |f| f);
            execute_builtin(&builtin, args, None, out, err, ctx).map_err(|e| {
                ShellError::InCommand { command: Command::BuiltIn(builtin), source: Box::new(e) }
            })
        }
        Command::Executable(executable) => {
            let stdout_stdio = out_file.map_or(Stdio::inherit(), Stdio::from);
            let stderr_stdio = err_file.map_or(Stdio::inherit(), Stdio::from);
            let args_strs: Vec<String> = args.iter().map(|a| a.to_string()).collect();
            let mut child = std::process::Command::new(executable.file_path())
                .args(args_strs)
                .current_dir(&ctx.cwd)
                .stdin(Stdio::inherit())
                .stdout(stdout_stdio)
                .stderr(stderr_stdio)
                .spawn()
                .map_err(|e| ShellError::InCommand {
                    command: Command::Executable(executable.clone()),
                    source: Box::new(ShellError::ExecutionFailed(e)),
                })?;
            let status = child.wait().map_err(|e| ShellError::InCommand {
                command: Command::Executable(executable.clone()),
                source: Box::new(ShellError::ExecutionFailed(e)),
            })?;
            if status.success() {
                Ok(None)
            } else {
                Err(ShellError::InCommand {
                    command: Command::Executable(executable),
                    source: Box::new(ShellError::NonZeroExit(status)),
                })
            }
        }
        Command::Unrecognized(cmd) => Err(ShellError::CommandNotFound {
            name: String::from_utf8_lossy(&cmd).into_owned(),
        }),
    }
}

/// Execute a [`Pipeline`] (one or more `|`-connected commands).
///
/// A single-stage pipeline delegates to [`execute`]. A multi-stage pipeline
/// creates N-1 OS-level inter-stage pipes, launches all N stages concurrently,
/// and waits for all. Data flows between stages via kernel pipe buffers with no
/// in-process buffering; output of the last stage goes to the inherited process
/// stdout and stderr fds.
///
/// Returns `Ok(Some(code))` when the last stage requests shell exit, `Ok(None)` otherwise.
pub fn execute_pipeline(
    pipeline: Pipeline,
    ctx: &mut ShellCtx,
) -> ShellResult<Option<ExitCode>> {
    if pipeline.len() == 1 {
        let (cmd, args, stdout_redir, stderr_redir) = pipeline.into_iter().next().unwrap();
        return execute(cmd, args, ctx, stdout_redir, stderr_redir);
    }

    let stages: Vec<_> = pipeline.into_iter().collect();
    let n = stages.len();

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
            if i < n - 1 {
                writers[i].take(); // close write end so downstream sees EOF
            }
            StageOutput::File(open_redirect_file(&r.mode, &r.target, &ctx.cwd)?)
        } else if i < n - 1 {
            StageOutput::Pipe(writers[i].take().unwrap())
        } else {
            StageOutput::Inherit
        };

        let stderr = match stderr_redirect {
            Some(r) => StageError::File(open_redirect_file(&r.mode, &r.target, &ctx.cwd)?),
            None => StageError::Inherit,
        };

        match launch_stage(command, args, stdin, stdout, stderr, ctx) {
            Ok(handle) => handles.push(handle),
            Err(launch_err) => {
                drop(readers);
                drop(writers);
                for handle in handles {
                    match handle {
                        StageHandle::Thread(h, _) => { let _ = h.join(); }
                        StageHandle::Process(mut child, _) => { let _ = child.wait(); }
                    }
                }
                return Err(launch_err);
            }
        }
    }

    let mut first_error: Option<ShellError> = None;
    let mut exit_request: Option<ExitCode> = None;
    let last = n - 1;

    for (i, handle) in handles.into_iter().enumerate() {
        let result = match handle {
            StageHandle::Thread(h, command) => h.join().unwrap_or_else(|_| {
                Err(ShellError::InCommand {
                    command,
                    source: Box::new(ShellError::Io(std::io::Error::other(
                        "pipeline stage thread panicked",
                    ))),
                })
            }),
            StageHandle::Process(mut child, command) => {
                let status = child.wait().map_err(|e| ShellError::InCommand {
                    command: command.clone(),
                    source: Box::new(ShellError::ExecutionFailed(e)),
                })?;
                if status.success() {
                    Ok(None)
                } else {
                    Err(ShellError::InCommand {
                        command,
                        source: Box::new(ShellError::NonZeroExit(status)),
                    })
                }
            }
        };
        match result {
            Ok(Some(code)) if i == last => exit_request = Some(code),
            Ok(_) => {}
            Err(e) if first_error.is_none() => {
                if i < last && is_broken_pipe(&e) {
                    // upstream stage killed by downstream exit — expected, not an error
                } else {
                    first_error = Some(e);
                }
            }
            Err(_) => {}
        }
    }

    if let Some(e) = first_error {
        Err(e)
    } else {
        Ok(exit_request)
    }
}

fn is_broken_pipe(e: &ShellError) -> bool {
    match e {
        ShellError::Io(io_err) => io_err.kind() == std::io::ErrorKind::BrokenPipe,
        ShellError::InCommand { source, .. } => is_broken_pipe(source),
        ShellError::NonZeroExit(status) => exit_status_is_sigpipe(status),
        _ => false,
    }
}

#[cfg(unix)]
fn exit_status_is_sigpipe(status: &std::process::ExitStatus) -> bool {
    use std::os::unix::process::ExitStatusExt;
    status.signal() == Some(13) // SIGPIPE
}

#[cfg(not(unix))]
fn exit_status_is_sigpipe(_: &std::process::ExitStatus) -> bool {
    false
}

fn execute_builtin(
    builtin: &BuiltInCommand,
    args: Args,
    _stdin: Option<PipeReader>,
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
            let display_name = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
            writeln!(out, "{} is {}", display_name, path.display())?
        }
        CommandKind::NotFound => {
            return Err(ShellError::CommandNotFound { name: arg.to_string() })
        }
    }

    Ok(())
}

fn execute_pwd(_args: Args, out: &mut dyn Write, ctx: &ShellCtx) -> ShellResult<()> {
    writeln!(out, "{}", ctx.cwd.display())?;
    Ok(())
}
