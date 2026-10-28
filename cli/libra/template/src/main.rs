fn main() {
    if let Err(e) = starter::run() {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}
