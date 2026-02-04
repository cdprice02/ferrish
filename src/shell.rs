use std::cell::RefCell;

use crate::{
    Command,
    arg::Args,
    error::ShellResult,
    executor,
    exit::ExitCode,
    io::{ShellIo, StandardIo},
    parser,
};

pub struct Shell<IO> {
    io: RefCell<IO>,
}

pub type StandardShell = Shell<StandardIo>;

impl Shell<()> {
    /// Create a shell builder
    ///
    /// # Example
    /// ```no_run
    /// use ferrish::Shell;
    ///
    /// let mut shell = Shell::builder()
    ///     .with_std_io();
    /// ```
    pub fn builder() -> ShellBuilder {
        ShellBuilder
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
                .write_all(b"\xF0\x9F\xA6\x80> ")?; // 🦀>

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
                Ok(Some(exit_code)) => {
                    // TODO: set exit code in caller env instead of printing
                    writeln!(
                        self.io.borrow_mut().out_writer(),
                        "Exiting with code {}",
                        exit_code
                    )?;
                    break;
                }
                Ok(None) => {}
                Err(e) => {
                    let fatal = e.is_fatal();
                    let e = anyhow::Error::new(e).context(command);
                    writeln!(self.io.borrow_mut().err_writer(), "{:#}", e);

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
        executor::execute(command, args, &mut *self.io.borrow_mut())
    }
}

pub struct ShellBuilder;

impl ShellBuilder {
    /// Configure the shell with standard I/O (stdin/stdout/stderr)
    ///
    /// # Example
    /// ```no_run
    /// use ferrish::Shell;
    ///
    /// let mut shell = Shell::builder()
    ///     .with_std_io()
    /// ```
    pub fn with_std_io(self) -> Shell<StandardIo> {
        Shell {
            io: RefCell::new(StandardIo::default()),
        }
    }

    /// Configure the shell with custom I/O
    ///
    /// # Example
    /// ```no_run
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
        }
    }
}
