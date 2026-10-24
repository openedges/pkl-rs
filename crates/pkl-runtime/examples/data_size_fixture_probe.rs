//! Legacy DataSize string protocol over the shared native session driver.
#[path = "support/string_session.rs"]
mod string_session;

fn main() {
    string_session::entrypoint(true);
}
