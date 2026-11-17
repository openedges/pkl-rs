//! Pure package HTTP URI generation, transport traits, and session request accounting.
//!
//! Networking and filesystem I/O are strictly excluded from `pkl-runtime`.
use crate::modules::ModuleHostError;
use crate::packages::PackageUri;
use crate::value::PklString;
use std::cell::Cell;

thread_local! {
    static REQUEST_COUNTS: Cell<(usize, usize)> = const { Cell::new((0, 0)) };
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

/// Computes the HTTPS metadata request URI for a package URI, matching Java Pkl.
pub fn get_metadata_request_uri(package: &PackageUri) -> PklString {
    let raw = package.uri.as_str().unwrap_or("");
    let ssp = if let Some(s) = raw.strip_prefix("package:") {
        s
    } else if let Some(s) = raw.strip_prefix("projectpackage:") {
        s
    } else {
        raw
    };
    let effective_ssp = if let Some(idx) = ssp.rfind("::") {
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
}
