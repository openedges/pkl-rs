//! Pkl 0.32.0 External Module & Resource Reader protocol, Wasm-pure Host
//! abstractions, and native OS subprocess runner.
//!
//! Architecture:
//! - [`messages`]: All 13 external-reader wire messages (`0x26`–`0x32`), reader
//!   specs, [`ExternalReaderConfig`], and [`shlex`] CLI parser (Wasm-pure).
//! - [`codec`]: Streaming & slice MessagePack encoder/decoder with exact Pkl 0.32.0
//!   `errorMessages.properties` diagnostics and key-omission nullability (Wasm-pure).
//! - [`transport`]: [`ExternalReaderTransport`] trait, in-memory [`DirectTransport`]
//!   and [`MockExternalReaderPeer`], caching resolvers, and [`ExternalReaderRegistry`]
//!   Host dispatch for `ModuleRequest`/`ReadRequest`/`GlobRequest` (Wasm-pure).
//! - [`process`] (`#[cfg(not(target_arch = "wasm32"))]`): [`NativeExternalReaderProcess`]
//!   implementing lazy spawn, stderr inheritance, cancel/timeout wakeup, and graceful
//!   `CloseExternalProcess` (`0x32`) → 3s timeout → force-kill + reap cleanup.

pub mod codec;
pub mod messages;
#[cfg(not(target_arch = "wasm32"))]
pub mod process;
pub mod transport;

pub use messages::{
    ExternalReaderConfig, ExternalReaderError, ExternalReaderMessage, MessageType,
    ModuleReaderSpec, ResourceReaderSpec, WirePathElement, shlex,
};
#[cfg(not(target_arch = "wasm32"))]
pub use process::{DEFAULT_CLOSE_TIMEOUT, NativeExternalReaderProcess};
pub use transport::{
    DirectTransport, ExternalModuleResolver, ExternalReaderProcessHandle, ExternalReaderRegistry,
    ExternalReaderTransport, ExternalResourceResolver, MockExternalModuleReader,
    MockExternalReaderPeer, MockExternalResourceReader, RawFrameTransport,
};
