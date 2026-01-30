use std::io::{self, BufRead, Write};

use ferrish::{executor, parser};

fn main() -> anyhow::Result<()> {
    let stdin = io::stdin();
    let mut stdin = stdin.lock();
    let stdout = io::stdout();
    let mut stdout = stdout.lock();
    let stderr = io::stderr();
    let mut stderr = stderr.lock();

    loop {
        write!(stdout, "🦀> ")?;
        stdout.flush()?;

        let mut buffer = Vec::<u8>::new();
        stdin.read_until(b'\n', &mut buffer)?;

        let buffer = buffer.trim_ascii();
        if buffer.is_empty() {
            // Empty command, just prompt again
            continue;
        }

        let (command, args) = parser::parse(buffer);

        let continue_running = executor::execute(command, args, &mut stdout, &mut stderr)?;

        stdout.flush()?;
        stderr.flush()?;

        if !continue_running {
            break;
        }
    }

    Ok(())
}
