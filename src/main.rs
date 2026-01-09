#[allow(unused_imports)]
use std::io::{self, BufRead, Write};

fn main() {
    let stdin = io::stdin();
    let mut stdin = stdin.lock();
    let stdout = io::stdout();
    let mut stdout = stdout.lock();

    loop {
        print!("$ ");
        stdout.flush().unwrap();

        let mut buffer = String::new();
        stdin.read_line(&mut buffer).unwrap();
        let buffer = buffer.trim();

        println!("{buffer}: command not found");
        stdout.flush().unwrap();
    }
}
