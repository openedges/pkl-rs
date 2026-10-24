//! Generic string-only source-session capture against independent Java fixtures.
//! Usage: OUTPUT INPUT URI. The output directory must already exist and be empty.
#[path = "support/string_session.rs"]
mod string_session;

fn main() {
    string_session::entrypoint(false);
}
