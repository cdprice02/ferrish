use std::cell::RefCell;
use std::path::PathBuf;

use crate::{
    Command,
    arg::Args,
    ctx::{ShellConfig, ShellCtx},
    error::ShellResult,
    executor,
    exit::ExitCode,
    io::{ShellIo, StandardIo},
    parser,
    redirect::{StderrRedirection, StdoutRedirection},
};

/// The ferrish shell REPL.
///
/// Generic over an I/O backend; use [`Shell::builder()`] to construct instances.
pub struct Shell<IO: ShellIo> {
    io: RefCell<IO>,
    ctx: ShellCtx,
}

/// Type alias for a [`Shell`] backed by real stdin/stdout/stderr.
pub type StandardShell = Shell<StandardIo>;

/// Non-generic entry points: `builder()` doesn't depend on the IO type,
/// so it lives on the concrete `Shell<StandardIo>` to avoid type-inference ambiguity.
impl Shell<StandardIo> {
    /// Create a shell builder
    ///
    /// # Example
    /// ```
    /// use ferrish::Shell;
    ///
    /// let mut shell = Shell::builder()
    ///     .with_std_io();
    /// ```
    pub fn builder() -> ShellBuilder {
        ShellBuilder::default()
    }
}

impl<IO: ShellIo> Shell<IO> {
    /// Return a shared borrow of the I/O backend.
    pub fn io(&self) -> std::cell::Ref<'_, IO> {
        self.io.borrow()
    }

    /// Return an exclusive borrow of the I/O backend.
    pub fn io_mut(&'_ mut self) -> std::cell::RefMut<'_, IO> {
        self.io.borrow_mut()
    }

    /// Run the interactive REPL loop until `exit` is called or stdin is exhausted.
    pub fn run(&mut self) -> anyhow::Result<ExitCode> {
        loop {
            {
                let mut io = self.io.borrow_mut();
                let w = io.out_writer();
                w.write_all(self.ctx.config.prompt.as_bytes())?;
                w.flush()?;
            }

            let mut buffer = Vec::<u8>::new();
            let bytes = self.io.borrow_mut().read_line(&mut buffer)?;
            if bytes == 0 {
                return Ok(ExitCode::SUCCESS);
            }

            let buffer = buffer.trim_ascii();
            if buffer.is_empty() {
                continue;
            }

            let src = String::from_utf8_lossy(buffer).into_owned();
            let (command, args, stdout_redirect, stderr_redirect) =
                match parser::parse(buffer) {
                    Ok(r) => r,
                    Err(e) => {
                        let report = miette::Report::new(e).with_source_code(src);
                        writeln!(self.io.borrow_mut().err_writer(), "{report:?}")?;
                        continue;
                    }
                };
            match self.execute_command(
                command.clone(),
                args,
                stdout_redirect,
                stderr_redirect,
            ) {
                Ok(Some(exit_code)) => return Ok(exit_code),
                Ok(None) => {}
                Err(e) => {
                    let fatal = e.is_fatal();
                    let report = miette::Report::new(e).with_source_code(src);
                    writeln!(self.io.borrow_mut().err_writer(), "{report:?}")?;
                    if fatal {
                        return Err(anyhow::anyhow!("fatal shell error"));
                    }
                }
            }
        }
    }

    /// Execute a sequence of command lines as a non-interactive script.
    pub fn run_script(&mut self, script: &[&str]) -> anyhow::Result<ExitCode> {
        for line in script {
            let buffer = line.as_bytes();
            let buffer = buffer.trim_ascii();

            if buffer.is_empty() {
                // TODO: handle comments
                continue;
            }

            let (command, args, stdout_redirect, stderr_redirect) = parser::parse(buffer)?;
            if let Some(exit_code) =
                self.execute_command(command, args, stdout_redirect, stderr_redirect)?
            {
                return Ok(exit_code);
            }
        }

        Ok(ExitCode::SUCCESS)
    }

    /// Execute a single parsed command, returning an optional exit code.
    pub fn execute_command(
        &mut self,
        command: Command,
        args: Args,
        stdout_redirect: Option<StdoutRedirection>,
        stderr_redirect: Option<StderrRedirection>,
    ) -> ShellResult<Option<ExitCode>> {
        executor::execute(
            command,
            args,
            &mut *self.io.borrow_mut(),
            &mut self.ctx,
            stdout_redirect,
            stderr_redirect,
        )
    }
}

/// Builder for constructing a [`Shell`] with custom I/O and configuration.
#[derive(Default)]
pub struct ShellBuilder {
    home_dir: Option<PathBuf>,
    cwd: Option<PathBuf>,
    config: Option<ShellConfig>,
}

impl ShellBuilder {
    /// Set an explicit home directory (overrides env HOME / USERPROFILE)
    pub fn with_home_dir(mut self, path: PathBuf) -> Self {
        self.home_dir = Some(path);
        self
    }

    /// Set an explicit initial working directory (overrides process CWD)
    pub fn with_cwd(mut self, path: PathBuf) -> Self {
        self.cwd = Some(path);
        self
    }

    /// Override the shell configuration
    pub fn with_config(mut self, config: ShellConfig) -> Self {
        self.config = Some(config);
        self
    }

    /// Override only the prompt string
    pub fn with_prompt(mut self, prompt: String) -> Self {
        let mut config = self.config.unwrap_or_default();
        config.prompt = prompt;
        self.config = Some(config);
        self
    }

    fn build_ctx(self) -> ShellCtx {
        let base = ShellCtx::from_env();
        ShellCtx::with_config(
            self.home_dir.or(base.home_dir),
            self.cwd.unwrap_or(base.cwd),
            self.config.unwrap_or_default(),
        )
    }

    /// Configure the shell with standard I/O (stdin/stdout/stderr)
    ///
    /// # Example
    /// ```
    /// use ferrish::Shell;
    ///
    /// let mut shell = Shell::builder()
    ///     .with_std_io();
    /// ```
    pub fn with_std_io(self) -> Shell<StandardIo> {
        Shell {
            io: RefCell::new(StandardIo::default()),
            ctx: self.build_ctx(),
        }
    }

    /// Configure the shell with custom I/O
    ///
    /// # Example
    /// ```
    /// use ferrish::Shell;
    /// use ferrish::io::MockIo;
    ///
    /// let io = MockIo::from_lines(&["echo test", "exit"]);
    /// let mut shell = Shell::builder()
    ///     .with_io(io);
    /// ```
    pub fn with_io<IO: ShellIo>(self, io: IO) -> Shell<IO> {
        Shell {
            io: RefCell::new(io),
            ctx: self.build_ctx(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::io::MockIo;
    use crate::Arg;

    #[test]
    fn test_shell_execute_command_echo() {
        let io = MockIo::empty();
        let mut shell = Shell::builder().with_io(io);
        let command = Command::BuiltIn(
            crate::command::builtin::BuiltInCommand::new(
                crate::command::builtin::BuiltInName::Echo,
            ),
        );
        let args = vec![Arg::from("test"), Arg::from("message")];
        let result = shell.execute_command(command, args, None, None);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), None);
        {
            let io_ref = shell.io();
            let output = io_ref.output();
            assert_eq!(output, b"test message\n");
        }
    }

    #[test]
    fn test_shell_execute_command_exit() {
        let io = MockIo::empty();
        let mut shell = Shell::builder().with_io(io);
        let command = Command::BuiltIn(
            crate::command::builtin::BuiltInCommand::new(
                crate::command::builtin::BuiltInName::Exit,
            ),
        );
        let result = shell.execute_command(command, vec![], None, None);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), Some(ExitCode::SUCCESS));
    }
}
