//! Real native HTTPS transport via `/usr/bin/curl`.
//!
//! Honors `--ca-certificates`, `--test-port` rewriting, HTTP 301/307 redirects,
//! and HTTP status code validation.
use pkl_runtime::modules::ModuleHostError;
use pkl_runtime::packages::PackageUri;
use pkl_runtime::value::PklString;
use std::path::PathBuf;

pub struct PackageTransportClient {
    pub ca_certificates: Option<PathBuf>,
    pub test_port: Option<u16>,
}

impl PackageTransportClient {
    pub fn new(ca_certificates: Option<PathBuf>, test_port: Option<u16>) -> Self {
        Self {
            ca_certificates,
            test_port,
        }
    }

    /// Rewrite URL port when `--test-port` is configured:
    /// e.g. `https://localhost:0/...` -> `https://localhost:<port>/...`
    pub fn rewrite_url(&self, raw_url: &str) -> String {
        let Some(port) = self.test_port else {
            return raw_url.to_string();
        };

        if let Some(rest) = raw_url.strip_prefix("https://localhost:0/") {
            return format!("https://localhost:{port}/{rest}");
        }
        if raw_url == "https://localhost:0" {
            return format!("https://localhost:{port}");
        }
        if let Some(rest) = raw_url.strip_prefix("https://localhost/") {
            return format!("https://localhost:{port}/{rest}");
        }
        raw_url.to_string()
    }

    /// Fetch URL using `/usr/bin/curl` following real HTTP redirects (301/307).
    pub fn fetch(&self, raw_url: &str) -> Result<Vec<u8>, ModuleHostError> {
        let rewritten_url = self.rewrite_url(raw_url);

        let temp_dir = std::env::temp_dir();
        let body_path = temp_dir.join(format!(
            "pkl_curl_body_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));

        let mut cmd = std::process::Command::new("/usr/bin/curl");
        cmd.arg("-sSL"); // silent, show-error, follow redirects
        if let Some(ca) = &self.ca_certificates {
            cmd.arg("--cacert").arg(ca);
        }
        cmd.arg("-o").arg(&body_path);
        cmd.arg("-w").arg("%{http_code}");
        cmd.arg(&rewritten_url);

        let output = cmd
            .output()
            .map_err(|err| ModuleHostError::SecurityManagerException {
                message: Some(format!("Failed to execute curl: {err}").into()),
            })?;

        if !output.status.success() {
            let _ = std::fs::remove_file(&body_path);
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(ModuleHostError::SecurityManagerException {
                message: Some(
                    format!(
                        "Exception when making request `GET {raw_url}`:\nConnection refused (localhost): {stderr}"
                    )
                    .into(),
                ),
            });
        }

        let status_str = String::from_utf8_lossy(&output.stdout);
        let status_code: u16 = status_str.trim().parse().unwrap_or(0);

        if status_code == 200 {
            let body = std::fs::read(&body_path).map_err(|err| {
                let _ = std::fs::remove_file(&body_path);
                ModuleHostError::SecurityManagerException {
                    message: Some(format!("Failed to read curl response body: {err}").into()),
                }
            })?;
            let _ = std::fs::remove_file(&body_path);
            Ok(body)
        } else if status_code == 404 {
            let _ = std::fs::remove_file(&body_path);
            Err(ModuleHostError::NotFound)
        } else {
            let _ = std::fs::remove_file(&body_path);
            Err(ModuleHostError::SecurityManagerException {
                message: Some(
                    format!(
                        "Exception when making request `GET {raw_url}`:\nHTTP response status: {status_code}"
                    )
                    .into(),
                ),
            })
        }
    }
}

impl pkl_runtime::packages::http::PackageTransport for PackageTransportClient {
    fn fetch_metadata(
        &self,
        _package: &PackageUri,
        request_uri: &PklString,
    ) -> Result<Vec<u8>, ModuleHostError> {
        let raw = request_uri
            .as_str()
            .ok_or_else(|| ModuleHostError::Unsupported("Metadata URI must be UTF-8".into()))?;
        self.fetch(raw)
    }

    fn fetch_zip(
        &self,
        _package: &PackageUri,
        zip_url: &PklString,
    ) -> Result<Vec<u8>, ModuleHostError> {
        let raw = zip_url
            .as_str()
            .ok_or_else(|| ModuleHostError::Unsupported("ZIP URL must be UTF-8".into()))?;
        self.fetch(raw)
    }
}
