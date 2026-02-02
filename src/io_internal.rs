use std::io::{self, BufRead, Write};

pub trait ShellIo {
    fn read_line(&mut self, buffer: &mut Vec<u8>) -> io::Result<usize>;
    fn write_out(&mut self, data: &[u8]) -> io::Result<()>;
    fn write_err(&mut self, data: &[u8]) -> io::Result<()>;
    fn flush(&mut self) -> io::Result<()>;
}

pub struct StandardIo<R, WO, WE> {
    reader: R,
    out_writer: WO,
    err_writer: WE,
}

impl<R, WO, WE> StandardIo<R, WO, WE> {
    pub fn new(reader: R, out_writer: WO, err_writer: WE) -> Self {
        Self {
            reader,
            out_writer,
            err_writer,
        }
    }
}

impl<R: BufRead, WO: Write, WE: Write> ShellIo for StandardIo<R, WO, WE> {
    fn read_line(&mut self, buffer: &mut Vec<u8>) -> io::Result<usize> {
        self.reader.read_until(b'\n', buffer)
    }

    fn write_out(&mut self, data: &[u8]) -> io::Result<()> {
        self.out_writer.write_all(data)
    }

    fn write_err(&mut self, data: &[u8]) -> io::Result<()> {
        self.err_writer.write_all(data)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.out_writer.flush()?;
        self.err_writer.flush()?;
        Ok(())
    }
}
