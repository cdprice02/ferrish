fn main() {
    match ferrish::run() {
        Ok(code) => std::process::exit(code.as_i32()),
        Err(e) => {
            eprintln!("ferrish: {e:#}");
            std::process::exit(1);
        }
    }
}
