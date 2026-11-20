//! Pure package HTTP URI generation, transport traits, and request accounting.
//!
//! Networking and filesystem I/O are strictly excluded from `pkl-runtime`.
use crate::modules::ModuleHostError;
use crate::packages::PackageUri;
use crate::value::PklString;
use std::cell::Cell;
use std::sync::atomic::{AtomicUsize, Ordering};

thread_local! {
    static REQUEST_COUNTS: Cell<(usize, usize)> = const { Cell::new((0, 0)) };
}

/// Session-scoped request accounting (replaces thread-local counters for new APIs).
#[derive(Debug, Default)]
pub struct RequestMetrics {
    metadata_requests: AtomicUsize,
    zip_requests: AtomicUsize,
}

impl RequestMetrics {
    pub const fn new() -> Self {
        Self {
            metadata_requests: AtomicUsize::new(0),
            zip_requests: AtomicUsize::new(0),
        }
    }

    pub fn record_metadata(&self) {
        self.metadata_requests.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_zip(&self) {
        self.zip_requests.fetch_add(1, Ordering::Relaxed);
    }

    pub fn metadata_count(&self) -> usize {
        self.metadata_requests.load(Ordering::Relaxed)
    }

    pub fn zip_count(&self) -> usize {
        self.zip_requests.load(Ordering::Relaxed)
    }

    pub fn reset(&self) {
        self.metadata_requests.store(0, Ordering::Relaxed);
        self.zip_requests.store(0, Ordering::Relaxed);
    }
}

/// Number of metadata requests recorded during the current thread/session.
pub fn metadata_request_count() -> usize {
    REQUEST_COUNTS.with(|c| c.get().0)
}

/// Number of ZIP requests recorded during the current thread/session.
pub fn zip_request_count() -> usize {
    REQUEST_COUNTS.with(|c| c.get().1)
}

/// Reset request counters for the current thread/session.
pub fn reset_request_counts() {
    REQUEST_COUNTS.with(|c| c.set((0, 0)));
}

/// Increment metadata request count for the current thread/session.
pub fn record_metadata_request() {
    REQUEST_COUNTS.with(|c| c.set((c.get().0 + 1, c.get().1)));
}

/// Increment ZIP request count for the current thread/session.
pub fn record_zip_request() {
    REQUEST_COUNTS.with(|c| c.set((c.get().0, c.get().1 + 1)));
}

/// Computes the HTTPS metadata request URI for a package URI, matching `PackageUri.java:97-103`.
///
/// Only strips `::...` when `package.sha256.is_some()`.
pub fn get_metadata_request_uri(package: &PackageUri) -> PklString {
    let raw = package.uri.as_str().unwrap_or("");
    let ssp = if let Some(s) = raw.strip_prefix("package:") {
        s
    } else if let Some(s) = raw.strip_prefix("projectpackage:") {
        s
    } else {
        raw
    };
    let effective_ssp = if package.sha256.is_some()
        && let Some(idx) = ssp.rfind("::")
    {
        &ssp[..idx]
    } else {
        ssp
    };
    PklString::from("https:").concat(&effective_ssp.into())
}

/// Validates that a metadata request URI is an HTTPS URL.
pub fn validate_metadata_url(raw_uri: &str) -> Result<(), ModuleHostError> {
    if !raw_uri.starts_with("https:") {
        return Err(ModuleHostError::SecurityManagerException {
            message: Some(format!("Expected HTTP(S) URL, but got: {raw_uri}").into()),
        });
    }
    Ok(())
}

/// Typed status/error response for package transport (used by publish checks & resolver).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PackageHttpError {
    BadStatusCode { status_code: u16, url: PklString },
    Security(PklString),
    Transport(PklString),
}

impl PackageHttpError {
    pub fn into_host_error(self) -> ModuleHostError {
        match self {
            Self::BadStatusCode { status_code, url } => ModuleHostError::SecurityManagerException {
                message: Some(
                    format!("Received HTTP status code {status_code} when requesting `{url}`.")
                        .into(),
                ),
            },
            Self::Security(msg) | Self::Transport(msg) => {
                ModuleHostError::SecurityManagerException { message: Some(msg) }
            }
        }
    }
}

/// Host-provided package transport abstraction.
pub trait PackageTransport: Send + Sync {
    /// Fetch metadata bytes over HTTPS for the given package.
    fn fetch_metadata(
        &self,
        package: &PackageUri,
        request_uri: &PklString,
    ) -> Result<Vec<u8>, ModuleHostError>;

    /// Fetch package ZIP archive bytes over HTTPS for the given package.
    fn fetch_zip(
        &self,
        package: &PackageUri,
        zip_url: &PklString,
    ) -> Result<Vec<u8>, ModuleHostError>;

    /// Fetch metadata bytes preserving HTTP status code for publish checks
    /// (`ProjectPackager.checkAlreadyPublishedPackage` needs exact 404 vs non-200).
    fn fetch_metadata_with_status(
        &self,
        package: &PackageUri,
        request_uri: &PklString,
    ) -> Result<Vec<u8>, PackageHttpError> {
        self.fetch_metadata(package, request_uri).map_err(|err| {
            let msg = match err {
                ModuleHostError::SecurityManagerException { message } => {
                    message.unwrap_or_else(|| "Transport error".into())
                }
                ModuleHostError::Io(io) => {
                    io.message.unwrap_or_else(|| "I/O transport error".into())
                }
                ModuleHostError::Unsupported(m) => m,
                ModuleHostError::NotFound => {
                    return PackageHttpError::BadStatusCode {
                        status_code: 404,
                        url: request_uri.clone(),
                    };
                }
                other => format!("{other:?}").into(),
            };
            if let Some(code) = extract_http_status_code(msg.as_str().unwrap_or("")) {
                PackageHttpError::BadStatusCode {
                    status_code: code,
                    url: request_uri.clone(),
                }
            } else {
                PackageHttpError::Transport(msg)
            }
        })
    }
}

fn extract_http_status_code(message: &str) -> Option<u16> {
    for prefix in [
        "HTTP status code ",
        "status code ",
        "The requested URL returned error: ",
    ] {
        if let Some(rest) = message.split(prefix).nth(1) {
            let digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
            if let Ok(code) = digits.parse::<u16>() {
                return Some(code);
            }
        }
    }
    None
}
