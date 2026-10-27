//! Run the filesystem expression adapter and native Host regression tests in the
//! default workspace suite, which otherwise only compiles the examples.
#[path = "../examples/support/scalar_probe.rs"]
mod scalar_probe;

// Keep the process entry point compiled while the tests exercise its shared
// evaluation routine directly. Actual process bytes have a separate oracle run.
const _: fn() = scalar_probe::main;
