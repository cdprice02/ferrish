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
    io::ShellIo,
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
    io: &mut impl ShellIo,
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

    match (stdout_file.as_mut(), stderr_file.as_mut()) {
        (Some(out_f), Some(err_f)) => {
            execute_with_writers(command, args, out_f, err_f, ctx)
        }
        (Some(out_f), None) => {
            execute_with_writers(command, args, out_f, io.err_writer(), ctx)
        }
        (None, Some(err_f)) => {
            execute_with_writers(command, args, io.out_writer(), err_f, ctx)
        }
        (None, None) => {
            let (out, err) = io.writers();
            execute_with_writers(command, args, out, err, ctx)
        }
    }
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
    io: &mut impl ShellIo,
    ctx: &mut ShellCtx,
) -> ShellResult<Option<ExitCode>> {
    // Fast path: no pipe operators.
    if pipeline.len() == 1 {
        let (cmd, args, stdout_redir, stderr_redir) = pipeline.into_iter().next().unwrap();
        return execute(cmd, args, io, ctx, stdout_redir, stderr_redir);
    }

    // Multi-stage: carry the previous stage's captured stdout forward.
    let mut stdin_buf: Option<Vec<u8>> = None;
    let last_idx = pipeline.len() - 1;

    for (i, (command, args, stdout_redirect, stderr_redirect)) in pipeline.into_iter().enumerate() {
        let is_last = i == last_idx;

        if is_last {
            // Final stage: write to the shell's real I/O (honouring any redirect).
            return if let Some(buf) = stdin_buf.take() {
                execute_stage_with_stdin(
                    command, args, io, ctx, stdout_redirect, stderr_redirect, &buf,
                )
            } else {
                execute(command, args, io, ctx, stdout_redirect, stderr_redirect)
            };
        }

        // Intermediate stage: capture stdout for the next stage.
        // POSIX: if this stage has a stdout redirect (`cmd > f | next`), honour
        // it — write to the file and pass empty stdin to the next stage.
        let mut out_buf: Vec<u8> = Vec::new();
        let prev = stdin_buf.take().unwrap_or_default();
        execute_stage_capture(
            command, args, io, ctx, stdout_redirect, stderr_redirect, &prev, &mut out_buf,
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
    io: &mut impl ShellIo,
    ctx: &mut ShellCtx,
    stdout_redirect: Option<StdoutRedirection>,
    stderr_redirect: Option<StderrRedirection>,
    stdin_data: &[u8],
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
        None => io.err_writer(),
    };

    let stdin = if stdin_data.is_empty() { None } else { Some(stdin_data) };
    execute_stage_inner(command, args, out_writer, err_writer, ctx, stdin)
}

/// Run the last pipeline stage, feeding `stdin_data` and honouring redirects.
fn execute_stage_with_stdin(
    command: Command,
    args: Args,
    io: &mut impl ShellIo,
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

    let (io_out, io_err) = io.writers();
    let out_writer: &mut dyn std::io::Write = match stdout_file.as_mut() {
        Some(f) => f,
        None => io_out,
    };
    let err_writer: &mut dyn std::io::Write = match stderr_file.as_mut() {
        Some(f) => f,
        None => io_err,
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
    use crate::io::MockIo;
    use std::path::PathBuf;

    fn test_ctx() -> ShellCtx {
        ShellCtx::new(None, std::env::current_dir().unwrap_or_else(|_| PathBuf::from("/")))
    }

    fn run_builtin(
        builtin: BuiltInName,
        args: Vec<Arg>,
        io: &mut MockIo,
        ctx: &mut ShellCtx,
    ) -> ShellResult<Option<ExitCode>> {
        execute(
            Command::BuiltIn(BuiltInCommand::new(builtin)),
            args,
            io,
            ctx,
            None,
            None,
        )
    }

    #[test]
    fn test_execute_builtin_exit() {
        let mut io = MockIo::empty();
        let mut ctx = test_ctx();
        let result = run_builtin(BuiltInName::Exit, vec![], &mut io, &mut ctx);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), Some(ExitCode::SUCCESS));
    }

    #[test]
    fn test_execute_builtin_exit_with_code() {
        let mut io = MockIo::empty();
        let mut ctx = test_ctx();
        let result = run_builtin(
            BuiltInName::Exit,
            vec![Arg::from("1")],
            &mut io,
            &mut ctx,
        );
        assert_eq!(result.unwrap(), Some(ExitCode(1)));
    }

    #[test]
    fn test_execute_type_builtin() {
        let args = vec![Arg::from("echo")];
        let mut out: Vec<u8> = Vec::new();
        execute_type(args, &mut out).unwrap();
        assert_eq!(out, b"echo is a shell builtin\n");
    }

    #[test]
    fn test_execute_type_unrecognized() {
        let args = vec![Arg::from("nonexistentcommand")];
        let mut out: Vec<u8> = Vec::new();
        let result = execute_type(args, &mut out);
        assert!(result.is_err());
        assert_eq!(
            result.err().unwrap(),
            ShellError::CommandNotFound { name: "nonexistentcommand".to_string() }
        );
    }

    #[test]
    fn test_execute_type_exit_builtin() {
        let args = vec![Arg::from("exit")];
        let mut out: Vec<u8> = Vec::new();
        execute_type(args, &mut out).unwrap();
        assert_eq!(out, b"exit is a shell builtin\n");
    }

    #[test]
    fn test_execute_builtin_pwd() {
        let cwd = PathBuf::from("/some/test/path");
        let ctx = ShellCtx::new(None, cwd.clone());
        let mut out: Vec<u8> = Vec::new();
        execute_pwd(vec![], &mut out, &ctx).unwrap();
        assert_eq!(out, format!("{}\n", cwd.display()).as_bytes());
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
        let mut out: Vec<u8> = Vec::new();
        let result = execute_type(args, &mut out);

        // SAFETY: test-only, single-threaded context
        unsafe { std::env::set_var("PATH", &original_path) };

        result.unwrap();
        let output = String::from_utf8(out).unwrap();
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
    fn test_execute_echo_with_redirect_writes_to_file() {
        let dir = tempfile::tempdir().expect("create temp dir");
        let target = dir.path().join("out.txt");
        let mut ctx = ShellCtx::new(None, dir.path().to_path_buf());
        let mut io = MockIo::empty();
        let redir = Some(StdoutRedirection::new(RedirectMode::Overwrite, target.clone()));
        let result = execute(
            Command::BuiltIn(BuiltInCommand::new(BuiltInName::Echo)),
            vec![Arg::from("hello")],
            &mut io,
            &mut ctx,
            redir,
            None,
        );
        assert!(result.is_ok(), "execute with redirect should succeed: {result:?}");
        // stdout (io.output) should be empty — echo went to the file.
        assert_eq!(io.output(), b"", "terminal stdout should be empty");
        let contents = std::fs::read_to_string(&target).expect("redirect file");
        assert_eq!(contents, "hello\n");
    }

    #[test]
    fn test_execute_with_stderr_redirect_writes_to_file() {
        let dir = tempfile::tempdir().expect("create temp dir");
        let err_target = dir.path().join("err.txt");
        let mut ctx = ShellCtx::new(None, dir.path().to_path_buf());
        let mut io = MockIo::empty();
        // exit with invalid argument writes to stderr
        let stderr_redir = Some(StderrRedirection::new(RedirectMode::Overwrite, err_target.clone()));
        let result = execute(
            Command::BuiltIn(BuiltInCommand::new(BuiltInName::Exit)),
            vec![Arg::from("notanumber")],
            &mut io,
            &mut ctx,
            None,
            stderr_redir,
        );
        assert!(result.is_ok(), "execute with stderr redirect should succeed: {result:?}");
        // io.error() (terminal stderr) should be empty — error went to the file.
        assert_eq!(io.error(), b"", "terminal stderr should be empty");
        let contents = std::fs::read_to_string(&err_target).expect("stderr redirect file");
        assert!(contents.contains("numeric argument required"), "expected error in file: {contents}");
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
