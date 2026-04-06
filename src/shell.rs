use std::cell::RefCell;
use std::path::PathBuf;

use crate::{
    Command,
    arg::Args,
    ctx::ShellCtx,
    error::ShellResult,
    executor,
    exit::ExitCode,
    io::{ShellIo, StandardIo},
    parser,
};

pub struct Shell<IO: ShellIo> {
    io: RefCell<IO>,
    ctx: ShellCtx,
}

pub type StandardShell = Shell<StandardIo>;

/// Non-generic entry points: `prefix()` and `builder()` don't depend on the IO type,
/// so they live on the concrete `Shell<StandardIo>` to avoid type-inference ambiguity.
impl Shell<StandardIo> {
    pub const fn prefix() -> &'static str {
        "\u{1F980}> " // 🦀>
    }

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
    pub fn io(&self) -> std::cell::Ref<'_, IO> {
        self.io.borrow()
    }

    pub fn io_mut(&'_ mut self) -> std::cell::RefMut<'_, IO> {
        self.io.borrow_mut()
    }

    pub fn run(&mut self) -> anyhow::Result<()> {
        loop {
            self.io
                .borrow_mut()
                .out_writer()
                .write_all(Shell::<StandardIo>::prefix().as_bytes())?;

            let mut buffer = Vec::<u8>::new();
            let bytes = self.io.borrow_mut().read_line(&mut buffer)?;
            if bytes == 0 {
                continue;
            }

            let buffer = buffer.trim_ascii();
            if buffer.is_empty() {
                continue;
            }

            let (command, args) = parser::parse(buffer);
            match self.execute_command(command.clone(), args) {
                Ok(Some(_exit_code)) => {
                    // TODO: set exit code in caller env
                    break;
                }
                Ok(None) => {}
                Err(e) => {
                    let fatal = e.is_fatal();
                    let e = anyhow::Error::new(e).context(command);
                    writeln!(self.io.borrow_mut().err_writer(), "{:#}", e)?;

                    if fatal {
                        return Err(e);
                    }
                }
            }
        }

        Ok(())
    }

    pub fn run_script(&mut self, script: &[&str]) -> anyhow::Result<ExitCode> {
        for line in script {
            let buffer = line.as_bytes();
            let buffer = buffer.trim_ascii();

            if buffer.is_empty() {
                // TODO: handle comments
                continue;
            }

            let (command, args) = parser::parse(buffer);
            if let Some(exit_code) = self.execute_command(command, args)? {
                // TODO: set exit code in caller env instead of returning
                return Ok(exit_code);
            }
        }

        Ok(ExitCode::SUCCESS)
    }

    pub fn execute_command(
        &mut self,
        command: Command,
        args: Args,
    ) -> ShellResult<Option<ExitCode>> {
        executor::execute(command, args, &mut *self.io.borrow_mut(), &mut self.ctx)
    }
}

#[derive(Default)]
pub struct ShellBuilder {
    home_dir: Option<PathBuf>,
    cwd: Option<PathBuf>,
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

    fn build_ctx(self) -> ShellCtx {
        let base = ShellCtx::from_env();
        ShellCtx::new(
            self.home_dir.or(base.home_dir),
            self.cwd.unwrap_or(base.cwd),
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
    fn test_shell_prefix() {
        assert_eq!(Shell::prefix(), "\u{1F980}> ");
    }

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
        let result = shell.execute_command(command, args);
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
        let result = shell.execute_command(command, vec![]);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), Some(ExitCode::SUCCESS));
    }
}
