//! CA certificate bundle loading and `rustls::ClientConfig` construction matching
//! Pkl 0.32.0 `JdkHttpClient.java:127-192` and `CliCommand.kt:217-250`.

use pkl_runtime::host::http::HttpError;
use rustls::RootCertStore;
use rustls_pki_types::CertificateDer;
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Bundled Pkl 0.32.0 root CA certificates verbatim from `pkl-commons-cli/PklCARoots.pem`.
pub const BUNDLED_PKL_CA_ROOTS_PEM: &[u8] = include_bytes!("../assets/PklCARoots.pem");

/// Source configuration for CA certificates following Pkl 0.32.0 CLI precedence:
/// 1. Explicit `--ca-certificates` files (repeatable)
/// 2. Regular files in `~/.pkl/cacerts/` (or overridden `cacerts_dir`)
/// 3. Bundled `PklCARoots.pem`
#[derive(Clone, Debug, Default)]
pub struct CaCertSource {
    /// Explicit `--ca-certificates` paths (highest precedence).
    pub explicit_files: Vec<PathBuf>,
    /// Override for `~/.pkl/cacerts/` directory (None => auto-detect `$HOME/.pkl/cacerts` if present).
    pub cacerts_dir: Option<PathBuf>,
    /// Extra raw PEM byte buffers (API / test convenience).
    pub extra_pem_bytes: Vec<Vec<u8>>,
    /// When true, skip probing `$HOME/.pkl/cacerts` so tests remain hermetic.
    pub disable_home_cacerts: bool,
}

impl CaCertSource {
    /// Default CLI-like CA certificate source (`~/.pkl/cacerts/` if populated, else bundled `PklCARoots.pem`).
    pub fn bundled_default() -> Self {
        Self::default()
    }

    /// Hermetic default using bundled `PklCARoots.pem` without reading `$HOME/.pkl/cacerts`.
    pub fn bundled_only() -> Self {
        Self {
            disable_home_cacerts: true,
            ..Self::default()
        }
    }

    pub fn from_files(files: Vec<PathBuf>) -> Self {
        Self {
            explicit_files: files,
            disable_home_cacerts: true,
            ..Self::default()
        }
    }

    /// Resolves and builds a `rustls::ClientConfig` from the configured CA certificate source.
    pub fn build_tls_config(&self) -> Result<Arc<rustls::ClientConfig>, HttpError> {
        let mut store = RootCertStore::empty();

        if !self.explicit_files.is_empty() || !self.extra_pem_bytes.is_empty() {
            for path in &self.explicit_files {
                load_cert_file_into_store(path, &mut store)?;
            }
            for (idx, bytes) in self.extra_pem_bytes.iter().enumerate() {
                load_cert_bytes_into_store(bytes, &format!("<bytes:{idx}>"), &mut store)?;
            }
        } else {
            let dir_files = self.discover_cacerts_dir_files()?;
            if !dir_files.is_empty() {
                for path in &dir_files {
                    load_cert_file_into_store(path, &mut store)?;
                }
            } else {
                load_cert_bytes_into_store(BUNDLED_PKL_CA_ROOTS_PEM, "PklCARoots.pem", &mut store)?;
            }
        }

        let provider = Arc::new(rustls::crypto::ring::default_provider());
        let config = rustls::ClientConfig::builder_with_provider(provider)
            .with_protocol_versions(&[&rustls::version::TLS13, &rustls::version::TLS12])
            .map_err(|err| {
                HttpError::HttpClientException(
                    format!("Error initializing HTTP client:\n{err}").into(),
                )
            })?
            .with_root_certificates(store)
            .with_no_client_auth();

        Ok(Arc::new(config))
    }

    fn discover_cacerts_dir_files(&self) -> Result<Vec<PathBuf>, HttpError> {
        let dir = if let Some(override_dir) = &self.cacerts_dir {
            Some(override_dir.clone())
        } else if !self.disable_home_cacerts {
            std::env::var_os("HOME")
                .or_else(|| std::env::var_os("USERPROFILE"))
                .map(|home| PathBuf::from(home).join(".pkl").join("cacerts"))
        } else {
            None
        };

        let Some(dir) = dir else {
            return Ok(Vec::new());
        };
        if !dir.is_dir() {
            return Ok(Vec::new());
        }
        let read_dir = std::fs::read_dir(&dir).map_err(|err| {
            HttpError::HttpClientException(
                format!(
                    "Error reading CA certificate file `{}`:\n{err}",
                    dir.display()
                )
                .into(),
            )
        })?;
        let mut files = Vec::new();
        for entry in read_dir {
            let entry = entry.map_err(|err| {
                HttpError::HttpClientException(
                    format!(
                        "Error reading CA certificate file `{}`:\n{err}",
                        dir.display()
                    )
                    .into(),
                )
            })?;
            let path = entry.path();
            if path.is_file() {
                files.push(path);
            }
        }
        files.sort();
        Ok(files)
    }
}

fn load_cert_file_into_store(path: &Path, store: &mut RootCertStore) -> Result<(), HttpError> {
    let display = path.display().to_string();
    let bytes = match std::fs::read(path) {
        Ok(b) => b,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            return Err(HttpError::HttpClientException(
                format!("Cannot find CA certificate file `{display}`.").into(),
            ));
        }
        Err(err) => {
            return Err(HttpError::HttpClientException(
                format!("Error reading CA certificate file `{display}`:\n{err}").into(),
            ));
        }
    };
    load_cert_bytes_into_store(&bytes, &display, store)
}

fn load_cert_bytes_into_store(
    bytes: &[u8],
    source_display: &str,
    store: &mut RootCertStore,
) -> Result<(), HttpError> {
    if bytes.is_empty() || bytes.iter().all(u8::is_ascii_whitespace) {
        return Err(HttpError::HttpClientException(
            format!("CA certificate file `{source_display}` is empty.").into(),
        ));
    }

    let mut certs: Vec<CertificateDer<'static>> = Vec::new();
    let has_pem_marker = bytes.windows(11).any(|w| w == b"-----BEGIN ");

    if has_pem_marker {
        let mut cursor = BufReader::new(bytes);
        for item in rustls_pemfile::certs(&mut cursor) {
            let der = item.map_err(|err| {
                let reason = format_pem_parse_error(&err);
                HttpError::HttpClientException(
                    format!("Error parsing CA certificate file `{source_display}`:\n{reason}")
                        .into(),
                )
            })?;
            certs.push(der);
        }
    } else if bytes.first() == Some(&0x30) {
        // Accept raw DER X.509 certificate just like Java's CertificateFactory.generateCertificates.
        certs.push(CertificateDer::from(bytes.to_vec()));
    } else {
        return Err(HttpError::HttpClientException(
            format!(
                "Error parsing CA certificate file `{source_display}`:\nNo certificate data found"
            )
            .into(),
        ));
    }

    if certs.is_empty() {
        return Err(HttpError::HttpClientException(
            format!(
                "Error parsing CA certificate file `{source_display}`:\nNo certificate data found"
            )
            .into(),
        ));
    }

    for cert in certs {
        store.add(cert).map_err(|err| {
            HttpError::HttpClientException(
                format!("Error parsing CA certificate file `{source_display}`:\n{err}").into(),
            )
        })?;
    }
    Ok(())
}

fn format_pem_parse_error(err: &std::io::Error) -> String {
    let msg = err.to_string();
    if err.kind() == std::io::ErrorKind::UnexpectedEof
        || msg.contains("section end")
        || msg.contains("missing")
    {
        // Matches JDK `sun.security.provider.X509Factory` on truncated PEM blocks.
        "Incomplete data".to_string()
    } else {
        msg
    }
}
