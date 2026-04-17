fn main() {
    if let Err(err) = fin_cli::run(std::env::args().skip(1).collect()) {
        eprintln!("{err}");
        std::process::exit(1);
    }
}