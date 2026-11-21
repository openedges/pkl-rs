//! Public HTTP client builder mirroring `org.pkl.core.http.{HttpClient, HttpClient.Builder}`.

use crate::error::{PklError, Result};
use pkl_host_native::{CaCertSource, CancelFlag, NativeHttpClient};
use pkl_runtime::host::http::{HttpClientConfig, ProxyConfig};
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

/// Reusable HTTP/HTTPS client shareable across multiple [`crate::Evaluator`] instances.
#[derive(Clone)]
pub struct HttpClient {
    inner: Arc<NativeHttpClient>,
}

impl HttpClient {
    pub fn builder() -> HttpClientBuilder {
        HttpClientBuilder::new()
    }

    pub fn close(&self) {
        self.inner.close();
    }

    pub fn is_closed(&self) -> bool {
        self.inner.is_closed()
    }

    pub(crate) fn inner(&self) -> Arc<NativeHttpClient> {
        self.inner.clone()
    }
}

/// Builder for [`HttpClient`] (`org.pkl.core.http.HttpClient.Builder`).
#[derive(Clone, Debug, Default)]
pub struct HttpClientBuilder {
    connect_timeout: Option<Duration>,
    request_timeout: Option<Duration>,
    ca_cert_files: Vec<PathBuf>,
    ca_cert_pems: Vec<Vec<u8>>,
    proxy_address: Option<String>,
    no_proxy: Vec<String>,
    rewrites: BTreeMap<String, String>,
    headers: BTreeMap<String, Vec<(String, String)>>,
    test_port: Option<u16>,
}

impl HttpClientBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_connect_timeout(mut self, timeout: Duration) -> Self {
        self.connect_timeout = Some(timeout);
        self
    }

    pub fn with_request_timeout(mut self, timeout: Duration) -> Self {
        self.request_timeout = Some(timeout);
        self
    }

    pub fn add_certificates_file(mut self, path: impl Into<PathBuf>) -> Self {
        self.ca_cert_files.push(path.into());
        self
    }

    pub fn add_certificates_pem(mut self, pem_bytes: impl Into<Vec<u8>>) -> Self {
        self.ca_cert_pems.push(pem_bytes.into());
        self
    }

    pub fn with_proxy(mut self, address: impl Into<String>, no_proxy: Vec<String>) -> Self {
        self.proxy_address = Some(address.into());
        self.no_proxy = no_proxy;
        self
    }

    pub fn add_rewrite(
        mut self,
        source_prefix: impl Into<String>,
        target_prefix: impl Into<String>,
    ) -> Self {
        self.rewrites
            .insert(source_prefix.into(), target_prefix.into());
        self
    }

    pub fn with_rewrites(mut self, rewrites: BTreeMap<String, String>) -> Self {
        self.rewrites = rewrites;
        self
    }

    pub fn add_header(
        mut self,
        url_prefix: impl Into<String>,
        name: impl Into<String>,
        value: impl Into<String>,
    ) -> Self {
        self.headers
            .entry(url_prefix.into())
            .or_default()
            .push((name.into(), value.into()));
        self
    }

    pub fn with_test_port(mut self, port: u16) -> Self {
        self.test_port = Some(port);
        self
    }

    /// Builds the [`HttpClient`] lazily (TLS context and sockets are initialized on first request).
    pub fn build(self) -> Result<HttpClient> {
        let mut cfg = HttpClientConfig::default();
        if let Some(d) = self.connect_timeout {
            cfg.connect_timeout_ms = d.as_millis().min(u64::MAX as u128) as u64;
        }
        if let Some(d) = self.request_timeout {
            cfg.request_timeout_ms = d.as_millis().min(u64::MAX as u128) as u64;
        }
        cfg.proxy = ProxyConfig::new(self.proxy_address.as_deref(), &self.no_proxy)
            .map_err(PklError::configuration)?;
        cfg = cfg
            .with_rewrites(self.rewrites)
            .map_err(PklError::configuration)?;
        for (url_pattern, hdrs) in self.headers {
            cfg.add_headers(&url_pattern, hdrs)
                .map_err(PklError::configuration)?;
        }
        cfg.test_port = self.test_port;

        let ca_certs = if !self.ca_cert_files.is_empty() || !self.ca_cert_pems.is_empty() {
            CaCertSource {
                explicit_files: self.ca_cert_files,
                cacerts_dir: None,
                extra_pem_bytes: self.ca_cert_pems,
                disable_home_cacerts: true,
            }
        } else {
            CaCertSource::bundled_default()
        };

        let inner = Arc::new(NativeHttpClient::new(cfg, ca_certs, CancelFlag::new()));
        Ok(HttpClient { inner })
    }
}
