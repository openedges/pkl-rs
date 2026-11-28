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
    EvalTimeoutGuard, HostFrameTransformer, HostLogger, NativeHost, NativeHostConfig,
    NativeHostExtensions, NativeStreamingOutputFilesOutcome, PrimaryModuleSource,
};
pub use fs::{
    NativeFs, append_file_nofollow, canonicalize_root_dir, detect_native_platform_metadata,
    file_uri_to_path, is_symlink_or_junction, is_uri_like, join_relative_path_segments,
    open_file_nofollow, path_to_file_uri, read_file_bounded, read_path_bounded,
    read_path_nofollow_bounded, read_path_to_string_bounded, read_static_local_project_import,
    resolve_canonical_or_ancestor, strip_windows_verbatim_prefix,
    try_resolve_canonical_or_ancestor, write_file_nofollow,
};
pub use http::{
    CancelFlag, NativeHttpClient, NativeHttpTransport, ScopedEvalCancel,
    is_current_thread_eval_cancelled,
};
pub use modulepath::{
    NativeModulePathResolver, read_generic_jar_resource, resolve_generic_jar_module,
};
pub use package_host::{
    DiskPackageCache, NativePackageTransport, NativePackagerHost, default_module_cache_dir,
};
pub use zip_archive::ZipReader;
