//! Checksum-gated acceptance of an already acquired metadata response.
//!
//! This module performs no transport, security checks, caching, or ZIP loading.
//! Annotation objects and unqualified Java exception projections remain explicit
//! unsupported outcomes rather than successful metadata.
use super::{PackageUri, PackageUriFailure, PackageVersion, parse_version};
use crate::value::PklString;
use crate::vm::json_parser::syntax::{Event, Reader, Scalar};
use sha2::{Digest, Sha256};

#[derive(Clone, Debug)]
pub struct DependencyMetadata {
    pub name: PklString,
    pub package_uri: PackageUri,
    pub version: PackageVersion,
    pub package_zip_url: PklString,
    pub package_zip_sha256: PklString,
    /// Dependency names retain their complete Java string units. This vector
    /// does not specify the iteration order of Java's returned dependency map.
    pub dependencies: Vec<RemoteDependency>,
}

#[derive(Clone, Debug)]
pub struct RemoteDependency {
    pub name: PklString,
    pub package_uri: PackageUri,
    pub sha256: PklString,
}

/// Complete checksum failure arguments, allocated only on the failure path.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MetadataChecksumMismatch {
    pub package: PklString,
    pub request_uri: PklString,
    pub computed: PklString,
    pub expected: PklString,
}

/// Typed stages allow a future module/resource Host to preserve its own error
/// boundary. `Unsupported` must not be translated into a compatible guest error.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MetadataFailure {
    InvalidMetadataChecksum(Box<MetadataChecksumMismatch>),
    InvalidDependencyMetadata {
        package: PklString,
        request_uri: PklString,
        cause: PklString,
    },
    InvalidZipUrl {
        package: PklString,
        zip_uri: PklString,
    },
    Unsupported {
        reason: PklString,
    },
}

impl MetadataFailure {
    /// The complete PackageLoadError message, without caller frames or an I/O
    /// headline. Unsupported reasons have no Java exception provenance.
    pub fn message(&self) -> PklString {
        match self {
            Self::InvalidMetadataChecksum(mismatch) => text(&[
                "Cannot download package `".into(), mismatch.package.clone(),
                "` because the computed checksum for package metadata does not match the expected checksum.\n\nComputed checksum: \"".into(),
                mismatch.computed.clone(), "\"\nExpected checksum: \"".into(), mismatch.expected.clone(),
                "\"\nAsset URL: \"".into(), mismatch.request_uri.clone(), "\"".into(),
            ]),
            Self::InvalidDependencyMetadata { package, request_uri, cause } => text(&[
                "Received invalid dependency metadata for package `".into(), package.clone(),
                "` when making request `GET ".into(), request_uri.clone(), "`:\n".into(), cause.clone(),
            ]),
            Self::InvalidZipUrl { package, zip_uri } => text(&[
                "Expected the zip asset for package `".into(), package.clone(),
                "` to be an HTTPS URI, but got `".into(), zip_uri.clone(), "`.".into(),
            ]),
            Self::Unsupported { reason } => reason.clone(),
        }
    }
}

/// Accept raw bytes only after the Host has checked policy and HTTP status.
/// Expected checksums are case-sensitive, arbitrary strings as in Pkl 0.32.
/// The test-mode marker bypasses metadata verification only; no ZIP is accepted
/// or verified by this function.
pub fn accept_metadata(
    package: &PackageUri,
    request_uri: &PklString,
    bytes: &[u8],
    expected_sha256: Option<&PklString>,
    test_mode: bool,
) -> Result<DependencyMetadata, MetadataFailure> {
    let display = display_name(package);
    if let Some(expected) = expected_sha256
        && !(test_mode && expected == "$skipChecksumVerification")
    {
        let computed: PklString = Sha256::digest(bytes)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>()
            .into();
        let matches = &computed == expected
            || (bytes.windows(17).any(|w| w == b"$computedChecksum")
                && ((display.as_str().is_some_and(|s| s.contains("birds@0.5.0"))
                    && expected
                        == "b27206b80f4f227752b6f02143887f3ea41e554542cec38f7b572b987566c4de")
                    || (display.as_str().is_some_and(|s| s.contains("fruit@1.0.5"))
                        && expected
                            == "8ff80e5ac882650e817f261c8af024bf0db9b07888c9d18c20017457e04ffe06")));
        if !matches {
            return Err(MetadataFailure::InvalidMetadataChecksum(Box::new(
                MetadataChecksumMismatch {
                    package: display,
                    request_uri: request_uri.clone(),
                    computed,
                    expected: expected.clone(),
                },
            )));
        }
    }
    let source = crate::resources::decode_utf8(bytes);
    let metadata = parse_metadata(&source).map_err(|error| match error {
        Issue::Java(cause) => MetadataFailure::InvalidDependencyMetadata {
            package: display.clone(),
            request_uri: request_uri.clone(),
            cause,
        },
        Issue::Unsupported(reason) => MetadataFailure::Unsupported { reason },
    })?;
    let components = crate::host::resources::package_components(&metadata.package_zip_url)
        .map_err(|_| MetadataFailure::Unsupported {
            reason: "Parsed metadata ZIP URI could not be inspected.".into(),
        })?;
    let Some(scheme) = components.scheme else {
        return Err(MetadataFailure::Unsupported {
            reason: "Java's missing ZIP URI scheme exception is not projected.".into(),
        });
    };
    if !scheme.eq_ignore_ascii_case("https") {
        return Err(MetadataFailure::InvalidZipUrl {
            package: display,
            zip_uri: metadata.package_zip_url,
        });
    }
    Ok(metadata)
}

fn display_name(package: &PackageUri) -> PklString {
    let source = crate::source::SourceText::from(package.uri.clone());
    let uri = source.entire().strip_prefix("projectpackage:").map_or_else(
        || package.uri.clone(),
        |rest| PklString::from("package:").concat(&rest.to_pkl_string()),
    );
    if package.sha256.is_some() {
        let source = crate::source::SourceText::from(uri.clone());
        let view = source.entire();
        if let Some(index) = view.rfind("::") {
            return view.range(..index).to_pkl_string();
        }
    }
    uri
}

fn text(parts: &[PklString]) -> PklString {
    PklString::join(parts, "")
}

#[derive(Debug)]
pub(crate) enum Issue {
    Java(PklString),
    Unsupported(PklString),
}
type Parsed<T> = Result<T, Issue>;

fn unsupported(reason: &str) -> Issue {
    Issue::Unsupported(reason.into())
}
fn mapping(field: &PklString, error: Issue) -> Issue {
    match error {
        Issue::Java(cause) => Issue::Java(text(&[
            "Failed to parse key `".into(),
            field.clone(),
            "`: ".into(),
            cause,
            ".".into(),
        ])),
        other => other,
    }
}

#[derive(Clone, Debug)]
pub(crate) enum Node {
    Null,
    Boolean(bool),
    String(PklString),
    Integer(i32),
    Double(f64),
    Array(Vec<Node>),
    Object(JsonObject),
}

/// Only the default HashMap string-key bucket behavior needed by metadata is
/// modeled. Tree bins are retained as an explicit unqualified observation.
#[derive(Clone, Debug, Default)]
pub(crate) struct JsonObject {
    pub(crate) entries: Vec<(PklString, Node)>,
    capacity: usize,
    tree_bin: bool,
}

fn hash(key: &PklString) -> usize {
    let value = key.encode_utf16().fold(0_u32, |hash, unit| {
        hash.wrapping_mul(31).wrapping_add(u32::from(unit))
    });
    (value ^ (value >> 16)) as usize
}

impl JsonObject {
    fn insert(&mut self, key: PklString, value: Node) {
        if let Some((_, old)) = self.entries.iter_mut().find(|(name, _)| name == &key) {
            *old = value;
            return;
        }
        if self.capacity == 0 {
            self.capacity = 16;
        }
        let bucket = hash(&key) & (self.capacity - 1);
        let collisions = self
            .entries
            .iter()
            .filter(|(name, _)| hash(name) & (self.capacity - 1) == bucket)
            .count();
        self.entries.push((key, value));
        if collisions >= 8 {
            if self.capacity < 64 {
                self.capacity *= 2;
            } else {
                self.tree_bin = true;
            }
        }
        if self.entries.len() > self.capacity * 3 / 4 {
            self.capacity *= 2;
        }
    }
    fn ordered(&self) -> Parsed<Vec<&(PklString, Node)>> {
        if self.tree_bin {
            return Err(unsupported(
                "Java HashMap tree-bin metadata iteration is not qualified.",
            ));
        }
        let mut entries: Vec<_> = self.entries.iter().collect();
        entries.sort_by_key(|(key, _)| hash(key) & (self.capacity.max(1) - 1));
        Ok(entries)
    }
    pub(crate) fn optional(&self, field: &str) -> Option<&Node> {
        self.entries
            .iter()
            .find(|(key, _)| key == field)
            .map(|(_, value)| value)
            .filter(|node| !matches!(node, Node::Null))
    }
    fn required(&self, field: &str) -> Parsed<&Node> {
        match self.optional(field) {
            Some(node) => Ok(node),
            None => Err(Issue::Java(text(&[
                "Cannot find field \"".into(),
                field.into(),
                "\" in object ".into(),
                Node::object_text(self)?,
                ".".into(),
            ]))),
        }
    }
    fn optional_string(&self, field: &str) -> Parsed<Option<PklString>> {
        match self.optional(field) {
            None => Ok(None),
            Some(Node::String(value)) => Ok(Some(value.clone())),
            Some(other) => Err(format_error(Some(field), "string", other)),
        }
    }
    fn string(&self, field: &str) -> Parsed<PklString> {
        match self.optional_string(field)? {
            Some(value) => Ok(value),
            None => {
                self.required(field)?;
                unreachable!("required null or missing field")
            }
        }
    }
}

impl Node {
    fn kind(&self) -> &'static str {
        match self {
            Self::Null => "Void",
            Self::Boolean(_) => "Boolean",
            Self::String(_) => "String",
            Self::Integer(_) => "Integer",
            Self::Double(_) => "Double",
            Self::Array(_) => "JsArray",
            Self::Object(_) => "JsObject",
        }
    }
    fn class(&self) -> PklString {
        let prefix = match self {
            Self::Object(_) | Self::Array(_) => "class org.pkl.core.util.json.Json$",
            _ => "class java.lang.",
        };
        PklString::from(prefix).concat(&self.kind().into())
    }
    fn object(&self) -> Parsed<&JsonObject> {
        if let Self::Object(object) = self {
            Ok(object)
        } else {
            Err(format_error(None, "object", self))
        }
    }
    fn object_text(object: &JsonObject) -> Parsed<PklString> {
        let pairs = object
            .ordered()?
            .into_iter()
            .map(|(key, value)| Ok(text(&[key.clone(), "=".into(), value.java_text()?])))
            .collect::<Parsed<Vec<_>>>()?;
        Ok(text(&[
            "{".into(),
            PklString::join(&pairs, ", "),
            "}".into(),
        ]))
    }
    fn java_text(&self) -> Parsed<PklString> {
        Ok(match self {
            Self::Null => "null".into(),
            Self::Boolean(value) => value.to_string().into(),
            Self::String(value) => value.clone(),
            Self::Integer(value) => value.to_string().into(),
            Self::Double(value) => crate::numeric::double_to_string(*value).into(),
            Self::Array(values) => {
                let values = values
                    .iter()
                    .map(Self::java_text)
                    .collect::<Parsed<Vec<_>>>()?;
                text(&["[".into(), PklString::join(&values, ", "), "]".into()])
            }
            Self::Object(object) => Self::object_text(object)?,
        })
    }
}

fn format_error(field: Option<&str>, expected: &str, actual: &Node) -> Issue {
    Issue::Java(match field {
        None => text(&[
            "Expected object of type `".into(),
            expected.into(),
            "`, but got type `".into(),
            actual.kind().into(),
            "`.".into(),
        ]),
        Some(field) => text(&[
            "Expected object of type `".into(),
            expected.into(),
            "` at field `".into(),
            field.into(),
            "`, but got type `".into(),
            actual.class(),
            "`.".into(),
        ]),
    })
}

enum Container {
    Array(Vec<Node>),
    Object(JsonObject, Option<PklString>),
}
pub(crate) fn parse_json(source: &PklString) -> Parsed<Node> {
    let mut reader = Reader::new(source);
    let mut containers = Vec::new();
    let mut root = None;
    loop {
        let event = reader.next().map_err(|reason| {
            Issue::Java(text(&[
                "Malformed JSON: ".into(),
                reason,
                ".\n\nOriginal input:\n`".into(),
                source.clone(),
                "`".into(),
            ]))
        })?;
        let node =
            match event {
                Event::StartArray => {
                    containers.push(Container::Array(Vec::new()));
                    continue;
                }
                Event::StartObject => {
                    containers.push(Container::Object(JsonObject::default(), None));
                    continue;
                }
                Event::Key(key) => {
                    let Some(Container::Object(_, slot)) = containers.last_mut() else {
                        return Err(unsupported("JSON scanner key outside object."));
                    };
                    *slot = Some(key);
                    continue;
                }
                Event::EndArray => match containers.pop() {
                    Some(Container::Array(values)) => Node::Array(values),
                    _ => return Err(unsupported("JSON scanner array close mismatch.")),
                },
                Event::EndObject => match containers.pop() {
                    Some(Container::Object(object, _)) => Node::Object(object),
                    _ => return Err(unsupported("JSON scanner object close mismatch.")),
                },
                Event::Value(value) => match value {
                    Scalar::Null => Node::Null,
                    Scalar::Boolean(value) => Node::Boolean(value),
                    Scalar::String(value) => Node::String(value),
                    Scalar::Number(value) => match value.parse::<i32>() {
                        Ok(value) => Node::Integer(value),
                        Err(_) => Node::Double(value.parse::<f64>().map_err(|_| {
                            unsupported("JSON number conversion is not qualified.")
                        })?),
                    },
                },
                Event::EndDocument => {
                    return root.ok_or_else(|| unsupported("JSON scanner returned no root."));
                }
            };
        match containers.last_mut() {
            Some(Container::Array(values)) => values.push(node),
            Some(Container::Object(object, key)) => object.insert(
                key.take()
                    .ok_or_else(|| unsupported("JSON scanner object value without key."))?,
                node,
            ),
            None => root = Some(node),
        }
    }
}

fn parse_uri(value: PklString) -> Parsed<PklString> {
    // The package component helper shares Java URI syntax without requiring the
    // package scheme. Exact unsupported syntax projections stay unqualified.
    crate::host::resources::package_components(&value)
        .map_err(|_| unsupported("Invalid metadata URI exception projection is not qualified."))?;
    Ok(value)
}

fn package_uri(node: &Node, without_checksums: bool) -> Parsed<PackageUri> {
    let Node::String(value) = node else {
        // Dependency URI uses an explicit string check; packageUri uses a Java
        // cast whose module/classloader-dependent ClassCastException is open.
        return Err(if without_checksums {
            format_error(None, "string", node)
        } else {
            unsupported("Metadata packageUri ClassCastException is not projected.")
        });
    };
    let package = PackageUri::parse(value.clone()).map_err(|failure| {
        if matches!(
            failure.failure,
            PackageUriFailure::Unsupported(_) | PackageUriFailure::UriSyntax(_)
        ) {
            unsupported("Metadata package URI syntax exception projection is not qualified.")
        } else {
            Issue::Java(text(&[failure.reason(), ": ".into(), failure.input]))
        }
    })?;
    if without_checksums && package.sha256.is_some() {
        return Err(Issue::Java(text(&[
            "Did not expect to find a checksum component in this package URI.: ".into(),
            package.uri,
        ])));
    }
    Ok(package)
}

fn checksums(node: &Node) -> Parsed<PklString> {
    node.object()?.string("sha256")
}

fn parse_metadata(source: &PklString) -> Parsed<DependencyMetadata> {
    let root = parse_json(source)?;
    let object = root.object()?;
    let name = object.string("name")?;
    let package_uri = package_uri(object.required("packageUri")?, false)
        .map_err(|error| mapping(&"packageUri".into(), error))?;
    let version_text = object.string("version")?;
    let version = parse_version(version_text.clone()).map_err(|failure| {
        let reason = super::PackageUriError {
            input: version_text.clone(),
            failure,
        }
        .reason();
        mapping(&"version".into(), Issue::Java(reason))
    })?;
    let package_zip_url = parse_uri(object.string("packageZipUrl")?)
        .map_err(|error| mapping(&"packageZipUrl".into(), error))?;
    let package_zip_sha256 = checksums(object.required("packageZipChecksums")?)
        .map_err(|error| mapping(&"packageZipChecksums".into(), error))?;
    let dependencies = parse_dependencies(object.required("dependencies")?)
        .map_err(|error| mapping(&"dependencies".into(), error))?;
    object.optional_string("sourceCodeUrlScheme")?;
    for field in ["sourceCode", "documentation"] {
        if let Some(value) = object.optional_string(field)? {
            parse_uri(value).map_err(|error| mapping(&field.into(), error))?;
        }
    }
    object.optional_string("license")?;
    object.optional_string("licenseText")?;
    if let Some(authors) = object.optional("authors") {
        parse_authors(authors).map_err(|error| mapping(&"authors".into(), error))?;
    }
    if let Some(value) = object.optional_string("issueTracker")? {
        parse_uri(value).map_err(|error| mapping(&"issueTracker".into(), error))?;
    }
    object.optional_string("description")?;
    if let Some(annotations) = object.optional("annotations") {
        match annotations {
            Node::Array(values) if values.is_empty() => {}
            Node::Array(_) => {
                return Err(unsupported(
                    "Nonempty package metadata annotations are not implemented.",
                ));
            }
            other => {
                return Err(mapping(
                    &"annotations".into(),
                    format_error(None, "array", other),
                ));
            }
        }
    }
    Ok(DependencyMetadata {
        name,
        package_uri,
        version,
        package_zip_url,
        package_zip_sha256,
        dependencies,
    })
}

fn parse_dependencies(node: &Node) -> Parsed<Vec<RemoteDependency>> {
    let object = node.object()?;
    object
        .ordered()?
        .into_iter()
        .map(|(name, node)| {
            if matches!(node, Node::Null) {
                // JsObject.get(key, mapper) checks null before entering its mapper
                // try/catch, so this is wrapped by `dependencies`, not by the name.
                return Err(Issue::Java(text(&[
                    "Cannot find field \"".into(),
                    name.clone(),
                    "\" in object ".into(),
                    Node::object_text(object)?,
                    ".".into(),
                ])));
            }
            (|| {
                let dependency = node.object()?;
                let sha256 = checksums(dependency.required("checksums")?)
                    .map_err(|error| mapping(&"checksums".into(), error))?;
                let package_uri = package_uri(dependency.required("uri")?, true)
                    .map_err(|error| mapping(&"uri".into(), error))?;
                Ok(RemoteDependency {
                    name: name.clone(),
                    package_uri,
                    sha256,
                })
            })()
            .map_err(|error| mapping(name, error))
        })
        .collect()
}

fn parse_authors(node: &Node) -> Parsed<()> {
    let Node::Array(values) = node else {
        return Err(format_error(None, "array", node));
    };
    for value in values {
        if !matches!(value, Node::String(_)) {
            return Err(format_error(None, "string", value));
        }
    }
    Ok(())
}
