//! Native OS host adapter for `pkl-runtime`.
//!
//! All platform-specific filesystem (`O_NOFOLLOW`, canonical `rootDir`), `modulepath:`
//! (directories + `.jar`/`.zip`/executable-JAR), `jar:file:`, HTTP/HTTPS (`rustls` +
//! HTTP/1.1 + HTTP/CONNECT proxy + CA certificates), package disk cache/transport,
//! external reader subprocess wiring, and monotonic wall-clock/cancellation I/O
//! live here so `pkl-runtime` remains pure `wasm32-unknown-unknown`.

pub mod certs;
pub mod cli;
pub mod driver;
pub mod fs;
pub mod http;
pub mod modulepath;
pub mod package_host;
pub mod zip_archive;

pub use certs::{BUNDLED_PKL_CA_ROOTS_PEM, CaCertSource};
pub use driver::{
    CleanupReport, CustomModuleReader, CustomResourceReader, CustomSecurityChecker,
    HostFrameTransformer, HostLogger, NativeHost, NativeHostConfig, NativeHostExtensions,
    NativeStreamingOutputFilesOutcome, PrimaryModuleSource,
};
pub use fs::{NativeFs, canonicalize_root_dir, file_uri_to_path, path_to_file_uri};
pub use http::{CancelFlag, NativeHttpClient, NativeHttpTransport};
pub use modulepath::{
    NativeModulePathResolver, read_generic_jar_resource, resolve_generic_jar_module,
};
pub use package_host::{
    DiskPackageCache, NativePackageTransport, NativePackagerHost, default_module_cache_dir,
};
pub use zip_archive::ZipReader;
