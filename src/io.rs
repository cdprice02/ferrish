use std::io::{self, BufRead, BufReader, Write};

pub trait ShellIo {
    fn reader(&mut self) -> &mut dyn BufRead;
    fn out_writer(&mut self) -> &mut dyn Write;
    fn err_writer(&mut self) -> &mut dyn Write;

    fn read_line(&mut self, buffer: &mut Vec<u8>) -> io::Result<usize> {
        let mut str = String::new();
        self.reader().read_line(&mut str).inspect(|_| {
            buffer.extend_from_slice(str.as_bytes());
        })
    }
}

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
}

#[derive(Debug)]
pub struct MockIo {
    reader: BufReader<std::io::Cursor<Vec<u8>>>,
    output: Vec<u8>,
    error: Vec<u8>,
}

impl MockIo {
    pub fn new(input: Vec<u8>) -> Self {
        Self {
            reader: BufReader::new(std::io::Cursor::new(input)),
            output: Vec::new(),
            error: Vec::new(),
        }
    }

    pub fn empty() -> Self {
        Self::new(Vec::new())
    }

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

    pub fn output(&self) -> &[u8] {
        &self.output
    }

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
    #[ignore]
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
    #[ignore]
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
    #[ignore]
    fn test_mock_io_from_empty_lines() {
        let lines: &[&str] = &[];
        let io = MockIo::from_lines(lines);
        let input_bytes = io.reader.get_ref().get_ref();
        assert_eq!(input_bytes, b"");
    }
}
