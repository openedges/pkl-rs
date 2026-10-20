//! Shared runtime for the Pkl 0.32.0 compatibility project.
//!
//! This crate currently establishes the native and WebAssembly build boundary.
//! Parsing, evaluation, diagnostics, and host integration are not implemented yet.
//! Future frontends and bindings will use this runtime instead of implementing
//! separate evaluation engines.
