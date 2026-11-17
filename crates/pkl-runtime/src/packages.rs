//! Pure package URI validation and bounded metadata acceptance.
//! Parsing does not perform I/O or load ZIPs or projects.
//! Rules follow Pkl 0.32 PackageUri/PackageAssetUri and its permissive Version.
pub mod cache;
pub mod http;
pub mod metadata;
pub mod project;
pub mod resolver;
pub mod zip;
use crate::source::SourceText;
use crate::value::PklString;

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
            PackageUriFailure::UriSyntax(reason) | PackageUriFailure::Unsupported(reason) => reason.clone(),
            PackageUriFailure::Opaque => surround("Module URI `", &self.input, "` is missing a `/` after `package:`."),
            PackageUriFailure::Scheme(scheme) => surround("Invalid scheme: `", &scheme.as_deref().unwrap_or("null").into(), "`.\n\nPackage URIs must have a `package:` scheme."),
            PackageUriFailure::MissingAuthority => "Package URIs must have an authority component.\n\nFor example, `example.com` in URI `project://example.com/my/package@1.0.0`.".into(),
            PackageUriFailure::MissingPath => "Package URIs must have a path component.\n\nFor example, `/my/package@1.0.0` in URI `project://example.com/my/package@1.0.0`.".into(),
            PackageUriFailure::ParentSegment => "Package URIs cannot contain `..` segments in the URI's \"path\" component.".into(),
            PackageUriFailure::MissingVersion(path) => surround("Invalid path `", path, "`.\n\nA package URI must have its path suffixed by its version (e.g. `package://example.com/my/package@1.0.0`)."),
            PackageUriFailure::InvalidChecksum(part) => surround("Invalid checksum specification: `", part, "`.\n\nChecksums in package uris must have form `sha256:<checksum>`."),
            PackageUriFailure::UnknownChecksum(algorithm) => surround("Unrecognized checksum algorithm: `", algorithm, "`.\n\nThe only supported checksum algorithm is sha256."),
            PackageUriFailure::InvalidVersion { text, overflow } => surround("`", text, if *overflow { "` is too large to fit into a Version." } else { "` could not be parsed as a semantic version number." }),
            PackageUriFailure::MissingFragment => "Expected URI to contain a fragment.\n\nThe URI must end in `#`, followed by a path string. For example: `package://example.com/my/project@1.0.0#/path/to/file.pkl`.".into(),
            PackageUriFailure::RelativeFragment(fragment) => surround("Cannot have a relative fragment path `", fragment, "`.\n\nThe fragment of this URI is interpreted as a path, and it must be absolute (it should start with `/`)."),
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

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PackageVersion {
    pub major: i32,
    pub minor: i32,
    pub patch: i32,
    pub pre_release: Option<PklString>,
    pub build: Option<PklString>,
}

#[derive(Clone, Debug)]
pub struct PackageUri {
    pub uri: PklString,
    pub path_without_version: PklString,
    pub version: PackageVersion,
    /// Pkl's constructor accepts arbitrary nonempty checksum text, not just hex.
    pub sha256: Option<PklString>,
}

impl PackageUri {
    pub fn parse(input: impl Into<PklString>) -> Result<Self, PackageUriError> {
        let input = input.into();
        let parts = crate::host::resources::package_components(&input)?;
        Self::from_components(&input, &parts)
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

#[derive(Clone, Debug)]
pub struct PackageAssetUri {
    pub uri: PklString,
    pub package: PackageUri,
    pub asset_path: PklString,
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
}

fn parse_version(text: PklString) -> Result<PackageVersion, PackageUriFailure> {
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
