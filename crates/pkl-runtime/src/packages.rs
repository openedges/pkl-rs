//! Pure package URI validation, canonical package identity, semantic versioning,
//! metadata acceptance, lockfiles, dependency resolution, caching, and package creation.
//!
//! Core modules under `packages/` are strictly `wasm32-unknown-unknown` pure with
//! `unsafe_code = "deny"`: no `std::fs`, `std::net`, or `std::process`.
pub mod cache;
pub mod cycles;
pub mod http;
pub mod metadata;
pub mod mvs;
pub mod packager;
pub mod project;
pub mod resolver;
pub mod test_runner;
pub mod zip;

use crate::source::SourceText;
use crate::value::PklString;
use std::cmp::Ordering;
use std::fmt;
use std::hash::{Hash, Hasher};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PackageUriFailure {
    UriSyntax(PklString),
    Unsupported(PklString),
    Opaque,
    Scheme(Option<String>),
    MissingAuthority,
    MissingPath,
    ParentSegment,
    MissingVersion(PklString),
    InvalidChecksum(PklString),
    UnknownChecksum(PklString),
    InvalidVersion { text: PklString, overflow: bool },
    MissingFragment,
    RelativeFragment(PklString),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PackageUriError {
    /// Constructor error input; checksum errors use the fragment-stripped URI.
    pub input: PklString,
    pub failure: PackageUriFailure,
}

impl PackageUriError {
    /// The Java exception reason, not a complete caller diagnostic or stack.
    pub fn reason(&self) -> PklString {
        fn surround(prefix: &str, value: &PklString, suffix: &str) -> PklString {
            PklString::from(prefix).concat(value).concat(&suffix.into())
        }
        match &self.failure {
            PackageUriFailure::UriSyntax(reason) | PackageUriFailure::Unsupported(reason) => {
                reason.clone()
            }
            PackageUriFailure::Opaque => surround(
                "Module URI `",
                &self.input,
                "` is missing a `/` after `package:`.",
            ),
            PackageUriFailure::Scheme(scheme) => surround(
                "Invalid scheme: `",
                &scheme.as_deref().unwrap_or("null").into(),
                "`.\n\nPackage URIs must have a `package:` scheme.",
            ),
            PackageUriFailure::MissingAuthority => "Package URIs must have an authority component.\n\nFor example, `example.com` in URI `project://example.com/my/package@1.0.0`.".into(),
            PackageUriFailure::MissingPath => "Package URIs must have a path component.\n\nFor example, `/my/package@1.0.0` in URI `project://example.com/my/package@1.0.0`.".into(),
            PackageUriFailure::ParentSegment => "Package URIs cannot contain `..` segments in the URI's \"path\" component.".into(),
            PackageUriFailure::MissingVersion(path) => surround(
                "Invalid path `",
                path,
                "`.\n\nA package URI must have its path suffixed by its version (e.g. `package://example.com/my/package@1.0.0`).",
            ),
            PackageUriFailure::InvalidChecksum(part) => surround(
                "Invalid checksum specification: `",
                part,
                "`.\n\nChecksums in package uris must have form `sha256:<checksum>`.",
            ),
            PackageUriFailure::UnknownChecksum(algorithm) => surround(
                "Unrecognized checksum algorithm: `",
                algorithm,
                "`.\n\nThe only supported checksum algorithm is sha256.",
            ),
            PackageUriFailure::InvalidVersion { text, overflow } => surround(
                "`",
                text,
                if *overflow {
                    "` is too large to fit into a Version."
                } else {
                    "` could not be parsed as a semantic version number."
                },
            ),
            PackageUriFailure::MissingFragment => "Expected URI to contain a fragment.\n\nThe URI must end in `#`, followed by a path string. For example: `package://example.com/my/project@1.0.0#/path/to/file.pkl`.".into(),
            PackageUriFailure::RelativeFragment(fragment) => surround(
                "Cannot have a relative fragment path `",
                fragment,
                "`.\n\nThe fragment of this URI is interpreted as a path, and it must be absolute (it should start with `/`).",
            ),
        }
    }
}

pub(crate) struct UriComponents {
    pub(crate) scheme: Option<String>,
    pub(crate) opaque: bool,
    pub(crate) authority: Option<PklString>,
    pub(crate) path: PklString,
    pub(crate) fragment: Option<PklString>,
}

/// Semantic version matching Pkl 0.32 `org.pkl.core.Version`.
///
/// Build metadata is preserved for display/serialization, while `Eq`, `Hash`,
/// and `Ord` ignore build metadata per SemVer 2.0.0 and `Version.java:188-206`.
#[derive(Clone, Debug)]
pub struct PackageVersion {
    pub major: i32,
    pub minor: i32,
    pub patch: i32,
    pub pre_release: Option<PklString>,
    pub build: Option<PklString>,
}

impl PackageVersion {
    pub fn parse(input: impl Into<PklString>) -> Result<Self, PackageUriError> {
        let text = input.into();
        parse_version(text.clone()).map_err(|failure| PackageUriError {
            input: text,
            failure,
        })
    }

    pub fn is_normal(&self) -> bool {
        self.pre_release.is_none() && self.build.is_none()
    }

    pub fn is_stable(&self) -> bool {
        self.major != 0 && self.pre_release.is_none()
    }

    pub fn to_pkl_string(&self) -> PklString {
        let mut out = PklString::from(format!("{}.{}.{}", self.major, self.minor, self.patch));
        if let Some(pre) = &self.pre_release {
            out = out.concat(&"-".into()).concat(pre);
        }
        if let Some(build) = &self.build {
            out = out.concat(&"+".into()).concat(build);
        }
        out
    }

    /// SemVer 2.0.0 ordering matching `Version.java:37-53, 188-251`.
    pub fn cmp_semver(&self, other: &Self) -> Ordering {
        self.major
            .cmp(&other.major)
            .then_with(|| self.minor.cmp(&other.minor))
            .then_with(|| self.patch.cmp(&other.patch))
            .then_with(|| match (&self.pre_release, &other.pre_release) {
                (None, None) => Ordering::Equal,
                (None, Some(_)) => Ordering::Greater,
                (Some(_), None) => Ordering::Less,
                (Some(a), Some(b)) => compare_prerelease(a, b),
            })
    }
}

impl PartialEq for PackageVersion {
    fn eq(&self, other: &Self) -> bool {
        self.major == other.major
            && self.minor == other.minor
            && self.patch == other.patch
            && self.pre_release == other.pre_release
    }
}

impl Eq for PackageVersion {}

impl Hash for PackageVersion {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.major.hash(state);
        self.minor.hash(state);
        self.patch.hash(state);
        if let Some(ref pre) = self.pre_release {
            1u8.hash(state);
            hash_pkl_string(pre, state);
        } else {
            0u8.hash(state);
        }
    }
}

fn hash_pkl_string<H: Hasher>(s: &PklString, state: &mut H) {
    for unit in s.encode_utf16() {
        unit.hash(state);
    }
    0xffff_u16.hash(state);
}

impl PartialOrd for PackageVersion {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for PackageVersion {
    fn cmp(&self, other: &Self) -> Ordering {
        self.cmp_semver(other)
    }
}

impl fmt::Display for PackageVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_pkl_string())
    }
}

fn compare_prerelease(a: &PklString, b: &PklString) -> Ordering {
    let a_units: Vec<u16> = a.encode_utf16().collect();
    let b_units: Vec<u16> = b.encode_utf16().collect();
    let a_parts = split_java_dot(&a_units);
    let b_parts = split_java_dot(&b_units);
    for (pa, pb) in a_parts.iter().zip(b_parts.iter()) {
        let ord = compare_prerelease_ident(pa, pb);
        if ord != Ordering::Equal {
            return ord;
        }
    }
    a_parts.len().cmp(&b_parts.len())
}

/// Java `String.split("\\.")` drops trailing empty segments.
fn split_java_dot(units: &[u16]) -> Vec<&[u16]> {
    let mut parts: Vec<&[u16]> = units.split(|&u| u == u16::from(b'.')).collect();
    while parts.last().is_some_and(|p| p.is_empty()) {
        parts.pop();
    }
    parts
}

fn parse_ascii_numeric_ident(units: &[u16]) -> Option<u128> {
    if units.is_empty() {
        return None;
    }
    let mut value = 0u128;
    for &u in units {
        if !(u16::from(b'0')..=u16::from(b'9')).contains(&u) {
            return None;
        }
        value = value
            .checked_mul(10)?
            .checked_add(u128::from(u - u16::from(b'0')))?;
    }
    Some(value)
}

fn compare_prerelease_ident(a: &[u16], b: &[u16]) -> Ordering {
    match (parse_ascii_numeric_ident(a), parse_ascii_numeric_ident(b)) {
        (Some(na), Some(nb)) => na.cmp(&nb),
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => a.cmp(b),
    }
}

/// Canonical package identity matching `org.pkl.core.project.CanonicalPackageUri`.
///
/// Format: `package://<host>[:<port>]<path_without_version>@<majorVersion>`.
/// Scheme is always `package`; userinfo, query, and fragment are stripped.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct CanonicalPackageUri {
    base_uri: PklString,
    major_version: i32,
    canonical: PklString,
}

impl Hash for CanonicalPackageUri {
    fn hash<H: Hasher>(&self, state: &mut H) {
        hash_pkl_string(&self.base_uri, state);
        self.major_version.hash(state);
    }
}

impl CanonicalPackageUri {
    pub fn from_package_uri(package_uri: &PackageUri) -> Self {
        let base_uri = canonical_base_uri(package_uri);
        let major_version = package_uri.version.major;
        let canonical = base_uri
            .concat(&"@".into())
            .concat(&major_version.to_string().into());
        Self {
            base_uri,
            major_version,
            canonical,
        }
    }

    /// Parses a canonical package URI string (e.g. `package://example.com/foo/bar@1`),
    /// matching `CanonicalPackageUri.of(String)`.
    pub fn parse(input: impl Into<PklString>) -> Result<Self, PackageUriError> {
        let input = input.into();
        let raw = input.as_str().ok_or_else(|| PackageUriError {
            input: input.clone(),
            failure: PackageUriFailure::UriSyntax("Invalid UTF-8 in canonical package URI".into()),
        })?;
        let Some(version_idx) = raw.rfind('@') else {
            return Err(PackageUriError {
                input: input.clone(),
                failure: PackageUriFailure::MissingVersion(input),
            });
        };
        let major_str = &raw[version_idx + 1..];
        let major_version: i32 = major_str.parse().map_err(|_| PackageUriError {
            input: input.clone(),
            failure: PackageUriFailure::UriSyntax(
                format!("Invalid major version `{major_str}` in canonical package URI `{raw}`.")
                    .into(),
            ),
        })?;
        let base_str = &raw[..version_idx];
        // Validate base URI syntax via resource URI parser
        let base_pkl = PklString::from(base_str);
        let parts =
            crate::host::resources::package_components(&base_pkl).map_err(|_| PackageUriError {
                input: input.clone(),
                failure: PackageUriFailure::UriSyntax(
                    format!("Illegal character or syntax in URI `{raw}`").into(),
                ),
            })?;
        if parts.opaque || parts.scheme.is_none() {
            return Err(PackageUriError {
                input: input.clone(),
                failure: PackageUriFailure::UriSyntax(
                    format!("Invalid canonical package URI `{raw}`").into(),
                ),
            });
        }
        let canonical = base_pkl
            .concat(&"@".into())
            .concat(&major_version.to_string().into());
        Ok(Self {
            base_uri: base_pkl,
            major_version,
            canonical,
        })
    }

    pub fn base_uri(&self) -> &PklString {
        &self.base_uri
    }

    pub fn major_version(&self) -> i32 {
        self.major_version
    }

    pub fn as_pkl_str(&self) -> &PklString {
        &self.canonical
    }

    pub fn as_str(&self) -> Option<&str> {
        self.canonical.as_str()
    }
}

impl fmt::Display for CanonicalPackageUri {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.canonical)
    }
}

fn canonical_base_uri(package_uri: &PackageUri) -> PklString {
    // Reconstruct `package://<host>[:<port>]<path_without_version>` dropping userinfo, query, fragment.
    let raw = package_uri.uri.as_str().unwrap_or("");
    let after_scheme = raw
        .strip_prefix("projectpackage://")
        .or_else(|| raw.strip_prefix("package://"));
    if let Some(rest) = after_scheme {
        let authority_end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
        let authority = &rest[..authority_end];
        let host_port = authority.rsplit_once('@').map_or(authority, |(_, hp)| hp);
        let encoded_path =
            encode_canonical_path(package_uri.path_without_version.as_str().unwrap_or(""));
        return PklString::from(format!("package://{host_port}{encoded_path}"));
    }
    // Fallback when authority was normalized to registry-less `package:/...`
    PklString::from("package:").concat(&package_uri.path_without_version)
}

fn encode_canonical_path(path: &str) -> String {
    let mut out = String::with_capacity(path.len());
    for byte in path.bytes() {
        if byte.is_ascii_alphanumeric() || b"/-._~!$&'()*+,;=:@".contains(&byte) {
            out.push(char::from(byte));
        } else {
            use std::fmt::Write;
            let _ = write!(&mut out, "%{byte:02X}");
        }
    }
    out
}

#[derive(Clone, Debug)]
pub struct PackageUri {
    pub uri: PklString,
    pub path_without_version: PklString,
    pub version: PackageVersion,
    /// Pkl's constructor accepts arbitrary nonempty checksum text, not just hex.
    pub sha256: Option<PklString>,
}

impl PartialEq for PackageUri {
    fn eq(&self, other: &Self) -> bool {
        self.uri == other.uri
    }
}

impl Eq for PackageUri {}

impl PartialOrd for PackageUri {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for PackageUri {
    fn cmp(&self, other: &Self) -> Ordering {
        self.uri.encode_utf16().cmp(other.uri.encode_utf16())
    }
}

impl Hash for PackageUri {
    fn hash<H: Hasher>(&self, state: &mut H) {
        hash_pkl_string(&self.uri, state);
    }
}

impl fmt::Display for PackageUri {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.uri)
    }
}

impl PackageUri {
    pub fn parse(input: impl Into<PklString>) -> Result<Self, PackageUriError> {
        let input = input.into();
        let parts = crate::host::resources::package_components(&input)?;
        Self::from_components(&input, &parts)
    }

    pub fn is_project_package(&self) -> bool {
        self.uri
            .as_str()
            .is_some_and(|s| s.starts_with("projectpackage:"))
    }

    /// Converts `projectpackage:` -> `package:`, preserving path/version/checksums.
    pub fn to_external_package_uri(&self) -> Self {
        if !self.is_project_package() {
            return self.clone();
        }
        let source = SourceText::from(self.uri.clone());
        let uri = source.entire().strip_prefix("projectpackage:").map_or_else(
            || self.uri.clone(),
            |rest| PklString::from("package:").concat(&rest.to_pkl_string()),
        );
        Self {
            uri,
            path_without_version: self.path_without_version.clone(),
            version: self.version.clone(),
            sha256: self.sha256.clone(),
        }
    }

    /// Converts `package:` -> `projectpackage:`, preserving path/version/checksums.
    pub fn to_project_package_uri(&self) -> Self {
        if self.is_project_package() {
            return self.clone();
        }
        let source = SourceText::from(self.uri.clone());
        let uri = source.entire().strip_prefix("package:").map_or_else(
            || self.uri.clone(),
            |rest| PklString::from("projectpackage:").concat(&rest.to_pkl_string()),
        );
        Self {
            uri,
            path_without_version: self.path_without_version.clone(),
            version: self.version.clone(),
            sha256: self.sha256.clone(),
        }
    }

    /// Returns a copy with any `::sha256:...` checksum suffix stripped.
    pub fn without_checksums(&self) -> Self {
        if self.sha256.is_none() {
            return self.clone();
        }
        let source = SourceText::from(self.uri.clone());
        let view = source.entire();
        let uri = if let Some(index) = view.rfind("::") {
            view.range(..index).to_pkl_string()
        } else {
            self.uri.clone()
        };
        Self {
            uri,
            path_without_version: self.path_without_version.clone(),
            version: self.version.clone(),
            sha256: None,
        }
    }

    /// Human-facing package URI (`package:` scheme, without checksum suffix),
    /// matching `PackageUri.getDisplayName()`.
    pub fn display_name(&self) -> PklString {
        self.to_external_package_uri().without_checksums().uri
    }

    pub fn to_canonical(&self) -> CanonicalPackageUri {
        CanonicalPackageUri::from_package_uri(self)
    }

    /// Builds a `PackageAssetUri` for `asset_path` (must start with `/`).
    pub fn to_package_asset_uri(
        &self,
        asset_path: impl Into<PklString>,
    ) -> Result<PackageAssetUri, PackageUriError> {
        let asset_path = asset_path.into();
        let encoded_fragment = zip::percent_encode_fragment(asset_path.as_str().unwrap_or(""));
        let full = self
            .uri
            .concat(&"#".into())
            .concat(&PklString::from(encoded_fragment));
        PackageAssetUri::parse(full)
    }

    /// Matches `PackageUtils.checkHasNoChecksumComponent(PackageUri)`.
    pub fn check_has_no_checksum_component(&self) -> Result<(), PklString> {
        if self.sha256.is_some() {
            return Err(PklString::from(format!(
                "Did not expect to find a checksum component in this package URI.: {}",
                self.uri
            )));
        }
        Ok(())
    }

    fn from_components(input: &PklString, parts: &UriComponents) -> Result<Self, PackageUriError> {
        let fail = |failure| PackageUriError {
            input: input.clone(),
            failure,
        };
        if parts.opaque {
            return Err(fail(PackageUriFailure::Opaque));
        }
        if !matches!(parts.scheme.as_deref(), Some("package" | "projectpackage")) {
            return Err(fail(PackageUriFailure::Scheme(parts.scheme.clone())));
        }
        if parts.authority.as_ref().is_none_or(PklString::is_empty) {
            return Err(fail(PackageUriFailure::MissingAuthority));
        }
        if parts.path.is_empty() {
            return Err(fail(PackageUriFailure::MissingPath));
        }
        let path_source = SourceText::from(parts.path.clone());
        let path = path_source.entire();
        if path.split("/").any(|part| part.to_pkl_string() == "..") {
            return Err(fail(PackageUriFailure::ParentSegment));
        }
        let at = path
            .rfind("@")
            .ok_or_else(|| fail(PackageUriFailure::MissingVersion(parts.path.clone())))?;
        // IoUtils.stripFragment rebuilds a seven-component URI from decoded
        // getters; simply slicing the raw fragment would retain wrong escapes.
        let base = crate::host::resources::package_without_fragment(input)?;
        let mut version = path.range(at + 1..);
        let mut sha256 = None;
        if let Some(checksum) = path.find("::").filter(|index| *index > at) {
            let text = path.range(checksum + 2..);
            version = path.range(at + 1..checksum);
            // String.split(":") drops trailing empty fields. Do not impose
            // stronger SHA length, hex or strict semver constraints than Java.
            let mut fields: Vec<_> = text.split(":").collect();
            while fields.last().is_some_and(|field| field.is_empty()) {
                fields.pop();
            }
            let failure = if fields.len() != 2 {
                Some(PackageUriFailure::InvalidChecksum(text.to_pkl_string()))
            } else if fields[0].to_pkl_string() != "sha256" {
                Some(PackageUriFailure::UnknownChecksum(
                    fields[0].to_pkl_string(),
                ))
            } else {
                sha256 = Some(fields[1].to_pkl_string());
                None
            };
            if let Some(failure) = failure {
                return Err(PackageUriError {
                    input: base,
                    failure,
                });
            }
        }
        let version = parse_version(version.to_pkl_string()).map_err(fail)?;
        Ok(Self {
            uri: base,
            path_without_version: path.range(..at).to_pkl_string(),
            version,
            sha256,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PackageAssetUri {
    pub uri: PklString,
    pub package: PackageUri,
    pub asset_path: PklString,
}

impl fmt::Display for PackageAssetUri {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.uri)
    }
}

impl PackageAssetUri {
    pub fn parse(input: impl Into<PklString>) -> Result<Self, PackageUriError> {
        let input = input.into();
        let parts = crate::host::resources::package_components(&input)?;
        let package = PackageUri::from_components(&input, &parts)?;
        let fragment = parts.fragment.ok_or_else(|| PackageUriError {
            input: input.clone(),
            failure: PackageUriFailure::MissingFragment,
        })?;
        if !fragment.starts_with("/") {
            return Err(PackageUriError {
                input,
                failure: PackageUriFailure::RelativeFragment(fragment),
            });
        }
        Ok(Self {
            uri: input,
            package,
            asset_path: fragment,
        })
    }

    pub fn to_project_package_asset_uri(&self) -> Self {
        let pkg = self.package.to_project_package_uri();
        let encoded_fragment =
            zip::percent_encode_fragment(self.asset_path.as_str().unwrap_or("/"));
        Self {
            uri: pkg
                .uri
                .concat(&"#".into())
                .concat(&PklString::from(encoded_fragment)),
            package: pkg,
            asset_path: self.asset_path.clone(),
        }
    }

    pub fn to_external_package_asset_uri(&self) -> Self {
        let pkg = self.package.to_external_package_uri();
        let encoded_fragment =
            zip::percent_encode_fragment(self.asset_path.as_str().unwrap_or("/"));
        Self {
            uri: pkg
                .uri
                .concat(&"#".into())
                .concat(&PklString::from(encoded_fragment)),
            package: pkg,
            asset_path: self.asset_path.clone(),
        }
    }

    /// Resolves a relative or root-relative asset path within the same package.
    pub fn resolve(&self, path: &str) -> Result<Self, PackageUriError> {
        let resolved_asset = if path.starts_with('/') {
            zip::normalize_zip_path(path)
        } else {
            let base = self.asset_path.as_str().unwrap_or("/");
            let dir = match base.rfind('/') {
                Some(idx) => &base[..=idx],
                None => "/",
            };
            zip::normalize_zip_path(&format!("{dir}{path}"))
        };
        self.package.to_package_asset_uri(resolved_asset)
    }
}

pub(crate) fn parse_version(text: PklString) -> Result<PackageVersion, PackageUriFailure> {
    let fail = |overflow| PackageUriFailure::InvalidVersion {
        text: text.clone(),
        overflow,
    };
    let units: Vec<u16> = text.encode_utf16().collect();
    let mut index = 0;
    let mut numbers = [0_i32; 3];
    let mut overflow = false;
    for (part, value) in numbers.iter_mut().enumerate() {
        let start = index;
        while let Some(digit) = units.get(index).copied().filter(|u| (48..=57).contains(u)) {
            if let Some(next) = value
                .checked_mul(10)
                .and_then(|n| n.checked_add(i32::from(digit - 48)))
            {
                *value = next;
            } else {
                overflow = true;
            }
            index += 1;
        }
        if index == start {
            return Err(fail(false));
        }
        if part < 2 {
            if units.get(index) != Some(&46) {
                return Err(fail(false));
            }
            index += 1;
        }
    }
    let mut pre_release = None;
    let mut build = None;
    if units.get(index) == Some(&45) {
        index += 1;
        let start = index;
        while index < units.len() && units[index] != 43 {
            index += 1;
        }
        if index == start {
            return Err(fail(false));
        }
        pre_release = Some(PklString::from_utf16(units[start..index].to_vec()));
    }
    if units.get(index) == Some(&43) {
        index += 1;
        if index == units.len()
            || units[index..]
                .iter()
                .any(|u| matches!(*u, 10 | 13 | 0x85 | 0x2028 | 0x2029))
        {
            return Err(fail(false));
        }
        build = Some(PklString::from_utf16(units[index..].to_vec()));
        index = units.len();
    }
    if index != units.len() {
        return Err(fail(false));
    }
    if overflow {
        return Err(fail(true));
    }
    Ok(PackageVersion {
        major: numbers[0],
        minor: numbers[1],
        patch: numbers[2],
        pre_release,
        build,
    })
}
