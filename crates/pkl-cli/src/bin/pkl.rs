//! Product `pkl` CLI binary for Apple Pkl 0.32.0.

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    std::process::exit(pkl_cli::run_cli(&args));
}
