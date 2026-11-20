//! Unified native Pkl 0.32.0 CLI binary (`pkl_m4_cli`) for Milestone M4 differential verification.

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    std::process::exit(pkl_host_native::cli::run_cli(&args));
}
