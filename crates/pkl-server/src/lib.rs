//! Native stdio MessagePack IPC server (`pkl server`) for Pkl 0.32.0.
//!
//! Matches `org.pkl.server.Server` + `org.pkl.cli.CliServer` (`0x20`–`0x2d`),
//! reusing the Wasm-pure protocol codec and client-callback resolvers from
//! `pkl_runtime::server` and `pkl_runtime::external_reader`.

mod evaluator;
mod server;

use std::fmt;
use std::io::{self, Read, Write};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ServerError {
    pub message: String,
}

impl ServerError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for ServerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for ServerError {}

/// CLI entry point for `pkl server` (`CliServer.kt:24-31`).
///
/// Communicates over `stdin`/`stdout` with raw concatenated MessagePack `[code, map]`
/// frames. Returns `0` on clean EOF; prints protocol/decode errors to `stderr` and
/// returns `1` on wire or fatal configuration errors.
pub fn run_cli_server() -> i32 {
    match run_stream_server(io::stdin(), io::stdout()) {
        Ok(()) => 0,
        Err(err) => {
            let mut stderr = io::stderr().lock();
            let _ = stderr.write_all(err.message.as_bytes());
            if !err.message.ends_with('\n') {
                let _ = stderr.write_all(b"\n");
            }
            let _ = stderr.flush();
            1
        }
    }
}

/// Runs the Pkl 0.32.0 MessagePack server over arbitrary `Read` and `Write` streams.
pub fn run_stream_server<R: Read + Send + 'static, W: Write + Send + 'static>(
    input: R,
    output: W,
) -> Result<(), ServerError> {
    server::run_stream_server(input, output)
}
