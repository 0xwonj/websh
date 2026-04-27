fn main() {
    if let Err(error) = websh::cli::run() {
        eprintln!("error: {error}");
        std::process::exit(1);
    }
}
