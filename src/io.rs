use std::io::{self, BufRead, BufReader, Write};

/// Abstraction over shell I/O streams, enabling both real and mock implementations.
pub trait ShellIo {
    /// Return a mutable reference to the input reader.
    fn reader(&mut self) -> &mut dyn BufRead;
    /// Return a mutable reference to the standard output writer.
    fn out_writer(&mut self) -> &mut dyn Write;
    /// Return a mutable reference to the standard error writer.
    fn err_writer(&mut self) -> &mut dyn Write;
    /// Return mutable references to both the stdout and stderr writers simultaneously.
    ///
    /// This is needed when the caller must hold both writers at the same time
    /// (e.g. to write command output to a redirected file while still sending
    /// error messages to the terminal stderr).
    fn writers(&mut self) -> (&mut dyn Write, &mut dyn Write);

    /// Read one line (up to and including `\n`) into `buffer`.
    fn read_line(&mut self, buffer: &mut Vec<u8>) -> io::Result<usize> {
        self.reader().read_until(b'\n', buffer)
    }
}

/// [`ShellIo`] implementation backed by the real `stdin`/`stdout`/`stderr` streams.
#[derive(Debug)]
pub struct StandardIo {
    reader: std::io::BufReader<std::io::Stdin>,
    out_writer: std::io::Stdout,
    err_writer: std::io::Stderr,
}

impl Default for StandardIo {
    fn default() -> Self {
        Self {
            reader: std::io::BufReader::new(std::io::stdin()),
            out_writer: std::io::stdout(),
            err_writer: std::io::stderr(),
        }
    }
}

impl ShellIo for StandardIo {
    fn reader(&mut self) -> &mut dyn BufRead {
        &mut self.reader
    }

    fn out_writer(&mut self) -> &mut dyn Write {
        &mut self.out_writer
    }

    fn err_writer(&mut self) -> &mut dyn Write {
        &mut self.err_writer
    }

    fn writers(&mut self) -> (&mut dyn Write, &mut dyn Write) {
        (&mut self.out_writer, &mut self.err_writer)
    }
}

/// In-memory [`ShellIo`] implementation for use in tests.
#[derive(Debug)]
pub struct MockIo {
    reader: BufReader<std::io::Cursor<Vec<u8>>>,
    output: Vec<u8>,
    error: Vec<u8>,
}

impl MockIo {
    /// Create a `MockIo` with the given raw bytes as stdin input.
    pub fn new(input: Vec<u8>) -> Self {
        Self {
            reader: BufReader::new(std::io::Cursor::new(input)),
            output: Vec::new(),
            error: Vec::new(),
        }
    }

    /// Create a `MockIo` with no stdin input.
    pub fn empty() -> Self {
        Self::new(Vec::new())
    }

    /// Create a `MockIo` where stdin contains each element of `lines` as a newline-terminated line.
    pub fn from_lines(lines: &[&str]) -> Self {
        let input_lines = lines
            .iter()
            .map(|line| {
                let mut v = line.as_bytes().to_vec();
                v.push(b'\n');
                v
            })
            .collect::<Vec<_>>();

        Self::new(input_lines.concat())
    }

    /// Return all bytes written to the stdout writer so far.
    pub fn output(&self) -> &[u8] {
        &self.output
    }

    /// Return all bytes written to the stderr writer so far.
    pub fn error(&self) -> &[u8] {
        &self.error
    }
}

impl ShellIo for MockIo {
    fn reader(&mut self) -> &mut dyn BufRead {
        &mut self.reader
    }

    fn out_writer(&mut self) -> &mut dyn Write {
        &mut self.output
    }

    fn err_writer(&mut self) -> &mut dyn Write {
        &mut self.error
    }

    fn writers(&mut self) -> (&mut dyn Write, &mut dyn Write) {
        (&mut self.output, &mut self.error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[allow(unused_imports)]
    use std::io::Write;

    #[test]
    fn test_mock_io_empty() {
        let io = MockIo::empty();
        assert_eq!(io.output().len(), 0);
        assert_eq!(io.error().len(), 0);
    }

    #[test]
    fn test_mock_io_new_with_input() {
        let input = b"hello\nworld\n".to_vec();
        let io = MockIo::new(input.clone());
        assert_eq!(io.output().len(), 0);
    }

    #[test]
    fn test_mock_io_from_lines() {
        let lines = &["echo hello", "echo world"];
        let io = MockIo::from_lines(lines);
        let input_bytes = io.reader.get_ref().get_ref();
        let expected = b"echo hello\necho world\n";
        assert_eq!(input_bytes, expected);
    }

    #[test]
    fn test_mock_io_from_single_line() {
        let lines = &["echo test"];
        let io = MockIo::from_lines(lines);
        let input_bytes = io.reader.get_ref().get_ref();
        assert_eq!(input_bytes, b"echo test\n");
    }

    #[test]
    fn test_mock_io_read_line() {
        let lines = &["hello", "world"];
        let mut io = MockIo::from_lines(lines);
        let mut buffer = Vec::new();
        let bytes_read = io.read_line(&mut buffer).unwrap();
        assert!(bytes_read > 0);
        assert_eq!(buffer, b"hello\n");
    }

    #[test]
    fn test_mock_io_output_write() {
        let mut io = MockIo::empty();
        io.out_writer().write_all(b"test output").unwrap();
        assert_eq!(io.output(), b"test output");
    }

    #[test]
    fn test_mock_io_error_write() {
        let mut io = MockIo::empty();
        io.err_writer().write_all(b"test error").unwrap();
        assert_eq!(io.error(), b"test error");
    }

    #[test]
    fn test_mock_io_from_empty_lines() {
        let lines: &[&str] = &[];
        let io = MockIo::from_lines(lines);
        let input_bytes = io.reader.get_ref().get_ref();
        assert_eq!(input_bytes, b"");
    }
}
