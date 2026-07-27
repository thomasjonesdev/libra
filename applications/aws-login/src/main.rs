fn main() {
    if let Err(e) = aws_login::run_app() {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}
