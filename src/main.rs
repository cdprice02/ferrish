use std::io::{self, BufRead, Write};

enum Command {
    Exit,
    Echo,
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

        let (command, args) = if buffer.contains(' ') {
            let (command, args) = buffer.split_once(' ').expect("buffer contains ` `");
            let args = args.split(' ').collect::<Vec<_>>();
            (command, args)
        } else {
            (buffer, vec![])
        };

        let command = match command {
            "exit" => Command::Exit,
            "echo" => Command::Echo,
            _ => {
                println!("{buffer}: command not found");
                stdout.flush().unwrap();

                continue;
            }
        };

        match command {
            Command::Exit => exit = true,
            Command::Echo => println!("{}", args.join(" ")),
        }
    }
}
