//! Development-only scalar differential probe, not a product CLI or API.
#[path = "support/scalar_probe.rs"]
mod scalar_probe;

fn main() {
    scalar_probe::main();
}
