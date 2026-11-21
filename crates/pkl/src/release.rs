//! Pkl 0.32.0 release metadata mirroring `org.pkl.core.Release` with honest native runtime info.

use std::sync::OnceLock;

/// Compatibility baseline version string.
pub const PKL_VERSION: &str = "0.32.0";
pub const PKL_MAJOR: u32 = 0;
pub const PKL_MINOR: u32 = 32;
pub const PKL_PATCH: u32 = 0;

/// Metadata describing the current Pkl release and host platform.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Release {
    version: String,
    os_name: String,
    os_version: String,
    arch: String,
    flavor: String,
    version_info: String,
}

static CURRENT_RELEASE: OnceLock<Release> = OnceLock::new();

impl Release {
    /// Returns release metadata for this native Pkl 0.32.0 runtime.
    pub fn current() -> &'static Self {
        CURRENT_RELEASE.get_or_init(Self::detect)
    }

    fn detect() -> Self {
        let os_name = detect_os_name();
        let os_version = detect_os_version();
        let arch = detect_arch();
        let os_part = if os_version.is_empty() {
            os_name.clone()
        } else {
            format!("{os_name} {os_version}")
        };
        let flavor = "native".to_string();
        let version_info = format!("Pkl {PKL_VERSION} ({os_part}, {flavor})");
        Self {
            version: PKL_VERSION.to_string(),
            os_name,
            os_version,
            arch,
            flavor,
            version_info,
        }
    }

    pub fn version(&self) -> &str {
        &self.version
    }

    pub const fn major(&self) -> u32 {
        PKL_MAJOR
    }

    pub const fn minor(&self) -> u32 {
        PKL_MINOR
    }

    pub const fn patch(&self) -> u32 {
        PKL_PATCH
    }

    pub fn os_name(&self) -> &str {
        &self.os_name
    }

    pub fn os_version(&self) -> &str {
        &self.os_version
    }

    pub fn arch(&self) -> &str {
        &self.arch
    }

    pub fn flavor(&self) -> &str {
        &self.flavor
    }

    /// Full version banner, e.g. `Pkl 0.32.0 (macOS 27.2, native)`.
    pub fn version_info(&self) -> &str {
        &self.version_info
    }

    /// Tool-specific version banner, e.g. `pkldoc 0.32.0 (macOS 27.2, native)`.
    pub fn tool_version_info(&self, tool_name: &str) -> String {
        let os_part = if self.os_version.is_empty() {
            self.os_name.clone()
        } else {
            format!("{} {}", self.os_name, self.os_version)
        };
        format!("{tool_name} {} ({os_part}, {})", self.version, self.flavor)
    }

    pub fn documentation_homepage(&self) -> String {
        format!("https://pkl-lang.org/main/{}/", self.version)
    }

    pub fn stdlib_homepage(&self) -> String {
        format!("https://pkl-lang.org/package-docs/pkl/{}/", self.version)
    }
}

fn detect_os_name() -> String {
    match std::env::consts::OS {
        "macos" => "macOS".to_string(),
        "linux" => "Linux".to_string(),
        "windows" => "Windows".to_string(),
        "freebsd" => "FreeBSD".to_string(),
        other => other.to_string(),
    }
}

fn detect_arch() -> String {
    match std::env::consts::ARCH {
        "x86_64" => "amd64".to_string(),
        "aarch64" => "aarch64".to_string(),
        other => other.to_string(),
    }
}

fn detect_os_version() -> String {
    #[cfg(target_os = "macos")]
    {
        if let Ok(plist) =
            std::fs::read_to_string("/System/Library/CoreServices/SystemVersion.plist")
            && let Some(ver) = extract_plist_string(&plist, "ProductVersion")
        {
            return ver;
        }
        if let Ok(out) = std::process::Command::new("sw_vers")
            .arg("-productVersion")
            .output()
            && out.status.success()
        {
            let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if !s.is_empty() {
                return s;
            }
        }
        String::new()
    }
    #[cfg(target_os = "linux")]
    {
        if let Ok(rel) = std::fs::read_to_string("/proc/sys/kernel/osrelease") {
            let s = rel.trim().to_string();
            if !s.is_empty() {
                return s;
            }
        }
        String::new()
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    {
        String::new()
    }
}

#[cfg(target_os = "macos")]
fn extract_plist_string(plist: &str, key: &str) -> Option<String> {
    let needle = format!("<key>{key}</key>");
    let start = plist.find(&needle)? + needle.len();
    let rest = &plist[start..];
    let s_open = rest.find("<string>")? + "<string>".len();
    let s_close = rest[s_open..].find("</string>")?;
    let val = rest[s_open..s_open + s_close].trim();
    if val.is_empty() {
        None
    } else {
        Some(val.to_string())
    }
}
