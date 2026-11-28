//! Native interactive & piped REPL (`pkl repl`) for Pkl 0.32.0.
//!
//! Shares the entire stateful live-heap REPL engine (`pkl_runtime::repl`) with Wasm targets,
//! keeping native terminal/stdio and `NativeHost` wiring isolated in this crate.

mod runner;
pub mod terminal;

pub use pkl_host_native::NativeHostConfig;

use std::io::{BufRead, Write};
use std::path::PathBuf;

/// Configuration for running `pkl repl` from the CLI or an embedder.
#[derive(Clone, Debug)]
pub struct ReplCliConfig {
    pub host_config: pkl_host_native::NativeHostConfig,
    pub working_dir: PathBuf,
    pub output_format: Option<String>,
    pub color: bool,
}

/// Runs `pkl repl` on process `stdin`/`stdout`/`stderr`, returning the process exit code (`0`/`1`).
pub fn run_cli_repl(config: ReplCliConfig) -> i32 {
    runner::run_cli(config)
}

/// Runs `pkl repl` against arbitrary `BufRead`/`Write` streams (used by tests and embedders).
pub fn run_repl_io<R: BufRead, W: Write, E: Write>(
    config: ReplCliConfig,
    input: R,
    output: W,
    stderr: E,
    is_tty: bool,
) -> i32 {
    runner::run_io(config, input, output, stderr, is_tty)
}
