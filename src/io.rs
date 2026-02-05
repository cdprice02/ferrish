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
