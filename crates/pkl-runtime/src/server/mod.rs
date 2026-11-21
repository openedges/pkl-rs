//! Pure-Rust Wasm-compatible `pkl server` protocol messages, streaming MessagePack
//! codec (`0x20`–`0x2d`), and client-callback resolver registry.

pub mod client_callbacks;
pub mod codec;
pub mod messages;

pub use client_callbacks::ClientCallbackRegistry;
pub use messages::{
    ServerChecksums, ServerDependency, ServerExternalReader, ServerHttp, ServerMessage,
    ServerProject, ServerProxy,
};
