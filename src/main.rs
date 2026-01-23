use std::io::{self, BufRead, Write};

enum Command {
    Exit,
}

fn main() {
    let stdin = io::stdin();
    let mut stdin = stdin.lock();
    let stdout = io::stdout();
    let mut stdout = stdout.lock();

    let mut exit = false;
    while !exit {
        print!("$ ");
        stdout.flush().unwrap();

        let mut buffer = String::new();
        stdin.read_line(&mut buffer).unwrap();
        let buffer = buffer.trim();

        let command = match buffer {
            "exit" => Command::Exit,
            _ => {
                println!("{buffer}: command not found");
                stdout.flush().unwrap();

                continue;
            }
        };

        match command {
            Command::Exit => exit = true,
        }
    }
}
