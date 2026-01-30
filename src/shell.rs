use std::{cell::RefCell, io};

use crate::{
    Command,
    arg::Args,
    executor,
    io::{ShellIo, StandardIo},
    parser,
};

pub struct Shell<IO> {
    io: RefCell<IO>,
}

pub type StandardShell =
    Shell<StandardIo<io::StdinLock<'static>, io::StdoutLock<'static>, io::StderrLock<'static>>>;

impl Shell<()> {
    /// Create a shell builder
    ///
    /// # Example
    /// ```no_run
    /// use ferrish::Shell;
    ///
    /// let mut shell = Shell::builder()
    ///     .with_standard_io()?
    ///     .run()?;
    /// # Ok::<(), anyhow::Error>(())
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

    /// Run the shell REPL
    pub fn run(&mut self) -> anyhow::Result<()> {
        loop {
            self.io.borrow_mut().write_out(b"\xF0\x9F\xA6\x80> ")?; // 🦀>
            self.io.borrow_mut().flush()?;

            let mut buffer = Vec::<u8>::new();
            self.io.borrow_mut().read_line(&mut buffer)?;
            let buffer = buffer.trim_ascii();

            if buffer.is_empty() {
                continue;
            }

            let (command, args) = parser::parse(buffer);
            let should_continue = self.execute_command(command, args)?;

            if !should_continue {
                break;
            }
        }

        Ok(())
    }

    pub fn execute_command(&mut self, command: Command, args: Args) -> anyhow::Result<bool> {
        // Adapters to convert ShellIo trait to std::io::Write
        struct OutWriter<'a, IO: ShellIo> {
            io: &'a RefCell<IO>,
        }
        struct ErrWriter<'a, IO: ShellIo> {
            io: &'a RefCell<IO>,
        }

        impl<IO: ShellIo> std::io::Write for OutWriter<'_, IO> {
            fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
                self.io.borrow_mut().write_out(buf)?;
                Ok(buf.len())
            }

            fn flush(&mut self) -> std::io::Result<()> {
                self.io.borrow_mut().flush()
            }
        }

        impl<IO: ShellIo> std::io::Write for ErrWriter<'_, IO> {
            fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
                self.io.borrow_mut().write_err(buf)?;
                Ok(buf.len())
            }

            fn flush(&mut self) -> std::io::Result<()> {
                self.io.borrow_mut().flush()
            }
        }

        let mut out_writer = OutWriter { io: &self.io };
        let mut err_writer = ErrWriter { io: &self.io };

        executor::execute(command, args, &mut out_writer, &mut err_writer)
    }

    pub fn run_script(&mut self, script: &[&str]) -> anyhow::Result<usize> {
        let mut count = 0;

        for line in script {
            let buffer = line.as_bytes();
            let buffer = buffer.trim_ascii();

            if buffer.is_empty() {
                // TODO: handle comments
                continue;
            }

            let (command, args) = parser::parse(buffer);
            let should_continue = self.execute_command(command, args)?;
            count += 1;

            if !should_continue {
                break;
            }
        }

        Ok(count)
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
    ///     .with_standard_io()
    ///     .run()?;
    /// # Ok::<(), anyhow::Error>(())
    /// ```
    pub fn with_standard_io(
        self,
    ) -> Shell<StandardIo<io::BufReader<io::Stdin>, io::Stdout, io::Stderr>> {
        let stdin = io::BufReader::new(io::stdin());
        let stdout = io::stdout();
        let stderr = io::stderr();

        Shell {
            io: RefCell::new(StandardIo::new(stdin, stdout, stderr)),
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
    ///     .with_io(io)
    ///     .run()?;
    /// # Ok::<(), anyhow::Error>(())
    /// ```
    pub fn with_io<IO: ShellIo>(self, io: IO) -> Shell<IO> {
        Shell {
            io: RefCell::new(io),
        }
    }
}
