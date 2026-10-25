//! Target-only proposed native logged string session for collection every.
//! Promote the helper under crates/pkl-runtime/examples/support/ so Cargo does
//! not discover it as a standalone example.
#[path = "support/every_logger_session.rs"]
mod every_logger_session;

fn main() {
    every_logger_session::entrypoint(false);
}
