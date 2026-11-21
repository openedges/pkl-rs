//! Standalone `pkldoc` CLI binary for Pkl 0.32.0.

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    std::process::exit(pkl_doc::run_cli(&args));
}
