//! Checksum-gated acceptance, parsing, and serialization of Pkl 0.32 `DependencyMetadata`.
//!
//! This module performs no transport, security checks, caching, or ZIP loading.
use super::{PackageUri, PackageUriFailure, PackageVersion, parse_version};
use crate::value::PklString;
use crate::vm::json_parser::syntax::{Event, Reader, Scalar};
use sha2::{Digest, Sha256};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MetadataClassInfo {
    pub module_name: PklString,
    pub class_name: PklString,
    pub module_uri: PklString,
}

#[derive(Clone, Debug, PartialEq)]
pub struct MetadataAnnotation {
    pub class_info: MetadataClassInfo,
    pub properties: Vec<(PklString, MetadataValue)>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum MetadataValue {
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    String(PklString),
    List(Vec<MetadataValue>),
    Set(Vec<MetadataValue>),
    Map(Vec<(MetadataValue, MetadataValue)>),
    PObject(MetadataAnnotation),
    Pattern(PklString),
    DataSize { value: f64, unit: PklString },
    Duration { value: f64, unit: PklString },
    Pair(Box<MetadataValue>, Box<MetadataValue>),
}

#[derive(Clone, Debug)]
pub struct DependencyMetadata {
    pub name: PklString,
    pub package_uri: PackageUri,
    pub version: PackageVersion,
    pub package_zip_url: PklString,
    pub package_zip_sha256: PklString,
    /// Dependency entries in exact JSON serialization order (`DependencyMetadata.java:528`).
    pub dependencies: Vec<RemoteDependency>,
    pub source_code_url_scheme: Option<PklString>,
    pub source_code: Option<PklString>,
    pub documentation: Option<PklString>,
    pub license: Option<PklString>,
    pub license_text: Option<PklString>,
    pub authors: Option<Vec<PklString>>,
    pub issue_tracker: Option<PklString>,
    pub description: Option<PklString>,
    pub annotations: Vec<MetadataAnnotation>,
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
                "Cannot download package `".into(),
                mismatch.package.clone(),
                "` because the computed checksum for package metadata does not match the expected checksum.\n\nComputed checksum: \"".into(),
                mismatch.computed.clone(),
                "\"\nExpected checksum: \"".into(),
                mismatch.expected.clone(),
                "\"\nAsset URL: \"".into(),
                mismatch.request_uri.clone(),
                "\"".into(),
            ]),
            Self::InvalidDependencyMetadata {
                package,
                request_uri,
                cause,
            } => text(&[
                "Received invalid dependency metadata for package `".into(),
                package.clone(),
                "` when making request `GET ".into(),
                request_uri.clone(),
                "`:\n".into(),
                cause.clone(),
            ]),
            Self::InvalidZipUrl { package, zip_uri } => text(&[
                "Expected the zip asset for package `".into(),
                package.clone(),
                "` to be an HTTPS URI, but got `".into(),
                zip_uri.clone(),
                "`.".into(),
            ]),
            Self::Unsupported { reason } => reason.clone(),
        }
    }
}

/// Accept raw bytes only after the Host has checked policy and HTTP status.
/// Expected checksums are case-sensitive, arbitrary strings as in Pkl 0.32.
/// The test-mode marker `$skipChecksumVerification` bypasses metadata verification
/// only (`PackageResolvers.java:160-193`); no ZIP is accepted or verified here.
pub fn accept_metadata(
    package: &PackageUri,
    request_uri: &PklString,
    bytes: &[u8],
    expected_sha256: Option<&PklString>,
    test_mode: bool,
) -> Result<DependencyMetadata, MetadataFailure> {
    let display = package.display_name();
    if let Some(expected) = expected_sha256
        && !(test_mode && expected == "$skipChecksumVerification")
    {
        let computed: PklString = sha256_hex(bytes).into();
        if &computed != expected {
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
    parse_metadata_bytes(package, request_uri, bytes)
}

/// Parses and validates metadata JSON bytes and HTTPS `packageZipUrl` without re-verifying
/// an external checksum (used for cached `.json` files and post-checksum verification).
pub fn parse_metadata_bytes(
    package: &PackageUri,
    request_uri: &PklString,
    bytes: &[u8],
) -> Result<DependencyMetadata, MetadataFailure> {
    let display = package.display_name();
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

/// Computes lowercase hex SHA-256 of raw bytes.
pub fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

impl DependencyMetadata {
    /// Serializes this metadata to UTF-8 JSON matching `DependencyMetadata.writeTo`
    /// (`DependencyMetadata.java:508-709`): 2-space indent, exact field order,
    /// `JsonEscaper` rules, and **no trailing newline**.
    pub fn write_json(&self) -> Result<String, PklString> {
        let mut w = JsonPrettyWriter::new("  ");
        w.begin_object();
        w.name("name");
        w.string_value(self.name.as_str().unwrap_or(""));
        w.name("packageUri");
        w.string_value(self.package_uri.uri.as_str().unwrap_or(""));
        w.name("version");
        w.string_value(self.version.to_pkl_string().as_str().unwrap_or(""));
        w.name("packageZipUrl");
        w.string_value(self.package_zip_url.as_str().unwrap_or(""));
        w.name("packageZipChecksums");
        w.begin_object();
        w.name("sha256");
        w.string_value(self.package_zip_sha256.as_str().unwrap_or(""));
        w.end_object();

        w.name("dependencies");
        w.begin_object();
        for dep in &self.dependencies {
            w.name(dep.name.as_str().unwrap_or(""));
            w.begin_object();
            w.name("uri");
            w.string_value(dep.package_uri.uri.as_str().unwrap_or(""));
            if !dep.sha256.is_empty() {
                w.name("checksums");
                w.begin_object();
                w.name("sha256");
                w.string_value(dep.sha256.as_str().unwrap_or(""));
                w.end_object();
            }
            w.end_object();
        }
        w.end_object();

        if let Some(ref scheme) = self.source_code_url_scheme {
            w.name("sourceCodeUrlScheme");
            w.string_value(scheme.as_str().unwrap_or(""));
        }
        if let Some(ref source_code) = self.source_code {
            w.name("sourceCode");
            w.string_value(source_code.as_str().unwrap_or(""));
        }
        if let Some(ref docs) = self.documentation {
            w.name("documentation");
            w.string_value(docs.as_str().unwrap_or(""));
        }
        if let Some(ref license) = self.license {
            w.name("license");
            w.string_value(license.as_str().unwrap_or(""));
        }
        if let Some(ref license_text) = self.license_text {
            w.name("licenseText");
            w.string_value(license_text.as_str().unwrap_or(""));
        }
        if let Some(ref authors) = self.authors {
            w.name("authors");
            w.begin_array();
            for author in authors {
                w.string_value(author.as_str().unwrap_or(""));
            }
            w.end_array();
        }
        if let Some(ref issue_tracker) = self.issue_tracker {
            w.name("issueTracker");
            w.string_value(issue_tracker.as_str().unwrap_or(""));
        }
        if let Some(ref description) = self.description {
            w.name("description");
            w.string_value(description.as_str().unwrap_or(""));
        }
        if !self.annotations.is_empty() {
            w.name("annotations");
            w.begin_array();
            for ann in &self.annotations {
                write_annotation_object(&mut w, ann)?;
            }
            w.end_array();
        }
        w.end_object();
        Ok(w.finish())
    }
}

fn write_annotation_object(
    w: &mut JsonPrettyWriter,
    ann: &MetadataAnnotation,
) -> Result<(), PklString> {
    w.begin_object();
    w.name("type");
    w.string_value("PObject");
    w.name("classInfo");
    w.begin_object();
    w.name("moduleName");
    w.string_value(ann.class_info.module_name.as_str().unwrap_or(""));
    w.name("class");
    w.string_value(ann.class_info.class_name.as_str().unwrap_or(""));
    w.name("moduleUri");
    w.string_value(ann.class_info.module_uri.as_str().unwrap_or(""));
    w.end_object();
    w.name("properties");
    w.begin_object();
    for (k, v) in &ann.properties {
        w.name(k.as_str().unwrap_or(""));
        write_metadata_value(w, v)?;
    }
    w.end_object();
    w.end_object();
    Ok(())
}

fn write_metadata_value(w: &mut JsonPrettyWriter, value: &MetadataValue) -> Result<(), PklString> {
    match value {
        MetadataValue::Null => w.null_value(),
        MetadataValue::Bool(b) => w.bool_value(*b),
        MetadataValue::Int(i) => w.i64_value(*i),
        MetadataValue::Float(f) => {
            if !f.is_finite() {
                return Err(PklString::from(format!(
                    "Numeric values must be finite, but was {f}"
                )));
            }
            w.f64_value(*f);
        }
        MetadataValue::String(s) => w.string_value(s.as_str().unwrap_or("")),
        MetadataValue::List(items) => {
            w.begin_array();
            for item in items {
                write_metadata_value(w, item)?;
            }
            w.end_array();
        }
        MetadataValue::Set(items) => {
            w.begin_object();
            w.name("type");
            w.string_value("Set");
            w.name("value");
            w.begin_array();
            for item in items {
                write_metadata_value(w, item)?;
            }
            w.end_array();
            w.end_object();
        }
        MetadataValue::Map(entries) => {
            w.begin_object();
            w.name("type");
            w.string_value("Map");
            w.name("value");
            w.begin_array();
            for (k, v) in entries {
                w.begin_object();
                w.name("key");
                write_metadata_value(w, k)?;
                w.name("value");
                write_metadata_value(w, v)?;
                w.end_object();
            }
            w.end_array();
            w.end_object();
        }
        MetadataValue::PObject(obj) => write_annotation_object(w, obj)?,
        MetadataValue::Pattern(pat) => {
            w.begin_object();
            w.name("type");
            w.string_value("Pattern");
            w.name("value");
            w.string_value(pat.as_str().unwrap_or(""));
            w.end_object();
        }
        MetadataValue::DataSize { value, unit } => {
            if !value.is_finite() {
                return Err(PklString::from(format!(
                    "Numeric values must be finite, but was {value}"
                )));
            }
            w.begin_object();
            w.name("type");
            w.string_value("DataSize");
            w.name("unit");
            w.string_value(unit.as_str().unwrap_or(""));
            w.name("value");
            w.f64_value(*value);
            w.end_object();
        }
        MetadataValue::Duration { value, unit } => {
            if !value.is_finite() {
                return Err(PklString::from(format!(
                    "Numeric values must be finite, but was {value}"
                )));
            }
            w.begin_object();
            w.name("type");
            w.string_value("Duration");
            w.name("unit");
            w.string_value(unit.as_str().unwrap_or(""));
            w.name("value");
            w.f64_value(*value);
            w.end_object();
        }
        MetadataValue::Pair(first, second) => {
            w.begin_object();
            w.name("type");
            w.string_value("Pair");
            w.name("first");
            write_metadata_value(w, first)?;
            w.name("second");
            write_metadata_value(w, second)?;
            w.end_object();
        }
    }
    Ok(())
}

/// Pure JSON pretty-writer matching `org.pkl.core.util.json.JsonWriter` and `JsonEscaper`.
pub(crate) struct JsonPrettyWriter {
    out: String,
    indent: &'static str,
    stack: Vec<WriterScope>,
    deferred_name: Option<String>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum WriterScope {
    EmptyDocument,
    NonEmptyDocument,
    EmptyArray,
    NonEmptyArray,
    EmptyObject,
    DanglingName,
    NonEmptyObject,
}

impl JsonPrettyWriter {
    pub(crate) fn new(indent: &'static str) -> Self {
        Self {
            out: String::new(),
            indent,
            stack: vec![WriterScope::EmptyDocument],
            deferred_name: None,
        }
    }

    pub(crate) fn begin_object(&mut self) {
        self.write_deferred_name();
        self.before_value();
        self.stack.push(WriterScope::EmptyObject);
        self.out.push('{');
    }

    pub(crate) fn end_object(&mut self) {
        let scope = self.stack.pop().expect("balanced object");
        if scope == WriterScope::NonEmptyObject {
            self.newline();
        }
        self.out.push('}');
    }

    pub(crate) fn begin_array(&mut self) {
        self.write_deferred_name();
        self.before_value();
        self.stack.push(WriterScope::EmptyArray);
        self.out.push('[');
    }

    pub(crate) fn end_array(&mut self) {
        let scope = self.stack.pop().expect("balanced array");
        if scope == WriterScope::NonEmptyArray {
            self.newline();
        }
        self.out.push(']');
    }

    pub(crate) fn name(&mut self, name: &str) {
        self.deferred_name = Some(name.to_string());
    }

    pub(crate) fn string_value(&mut self, value: &str) {
        self.write_deferred_name();
        self.before_value();
        write_escaped_json_string(&mut self.out, value);
    }

    pub(crate) fn bool_value(&mut self, value: bool) {
        self.write_deferred_name();
        self.before_value();
        self.out.push_str(if value { "true" } else { "false" });
    }

    pub(crate) fn i64_value(&mut self, value: i64) {
        self.write_deferred_name();
        self.before_value();
        use std::fmt::Write;
        let _ = write!(&mut self.out, "{value}");
    }

    pub(crate) fn f64_value(&mut self, value: f64) {
        self.write_deferred_name();
        self.before_value();
        self.out.push_str(&crate::numeric::double_to_string(value));
    }

    pub(crate) fn null_value(&mut self) {
        self.write_deferred_name();
        self.before_value();
        self.out.push_str("null");
    }

    pub(crate) fn trailing_newline(&mut self) {
        self.out.push('\n');
    }

    pub(crate) fn finish(self) -> String {
        self.out
    }

    fn write_deferred_name(&mut self) {
        if let Some(name) = self.deferred_name.take() {
            match self.stack.last().copied() {
                Some(WriterScope::NonEmptyObject) => {
                    self.out.push(',');
                    self.newline();
                    if let Some(slot) = self.stack.last_mut() {
                        *slot = WriterScope::DanglingName;
                    }
                }
                Some(WriterScope::EmptyObject) => {
                    self.newline();
                    if let Some(slot) = self.stack.last_mut() {
                        *slot = WriterScope::DanglingName;
                    }
                }
                _ => {}
            }
            write_escaped_json_string(&mut self.out, &name);
        }
    }

    fn before_value(&mut self) {
        match self.stack.last_mut() {
            Some(scope @ WriterScope::EmptyDocument) => {
                *scope = WriterScope::NonEmptyDocument;
            }
            Some(scope @ WriterScope::EmptyArray) => {
                *scope = WriterScope::NonEmptyArray;
                self.newline();
            }
            Some(WriterScope::NonEmptyArray) => {
                self.out.push(',');
                self.newline();
            }
            Some(scope @ WriterScope::DanglingName) => {
                self.out
                    .push_str(if self.indent.is_empty() { ":" } else { ": " });
                *scope = WriterScope::NonEmptyObject;
            }
            _ => {}
        }
    }

    fn newline(&mut self) {
        if self.indent.is_empty() {
            return;
        }
        self.out.push('\n');
        let depth = self.stack.len().saturating_sub(1);
        for _ in 0..depth {
            self.out.push_str(self.indent);
        }
    }
}

/// Public JSON string escaper matching `JsonEscaper(isHtmlSafe = false)`.
pub fn write_escaped_json_string_pub(out: &mut String, value: &str) {
    write_escaped_json_string(out, value);
}

/// Escapes a JSON string matching `JsonEscaper(isHtmlSafe = false)`.
pub(crate) fn write_escaped_json_string(out: &mut String, value: &str) {
    out.push('"');
    for ch in value.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\u{0008}' => out.push_str("\\b"),
            '\u{000c}' => out.push_str("\\f"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => {
                use std::fmt::Write;
                let _ = write!(out, "\\u{:04x}", c as u32);
            }
            '\u{2028}' => out.push_str("\\u2028"),
            '\u{2029}' => out.push_str("\\u2029"),
            c => out.push(c),
        }
    }
    out.push('"');
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
    Long(i64),
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
    pub(crate) fn required(&self, field: &str) -> Parsed<&Node> {
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
    pub(crate) fn optional_string(&self, field: &str) -> Parsed<Option<PklString>> {
        match self.optional(field) {
            None => Ok(None),
            Some(Node::String(value)) => Ok(Some(value.clone())),
            Some(other) => Err(format_error(Some(field), "string", other)),
        }
    }
    pub(crate) fn string(&self, field: &str) -> Parsed<PklString> {
        match self.optional_string(field)? {
            Some(value) => Ok(value),
            None => {
                self.required(field)?;
                unreachable!("required null or missing field")
            }
        }
    }
    pub(crate) fn object_field(&self, field: &str) -> Parsed<&JsonObject> {
        match self.required(field)? {
            Node::Object(obj) => Ok(obj),
            other => Err(format_error(Some(field), "object", other)),
        }
    }
    pub(crate) fn array_field(&self, field: &str) -> Parsed<&[Node]> {
        match self.required(field)? {
            Node::Array(arr) => Ok(arr),
            other => Err(format_error(Some(field), "array", other)),
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
            Self::Long(_) => "Long",
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
    pub(crate) fn object(&self) -> Parsed<&JsonObject> {
        if let Self::Object(object) = self {
            Ok(object)
        } else {
            Err(format_error(None, "object", self))
        }
    }
    pub(crate) fn object_text(object: &JsonObject) -> Parsed<PklString> {
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
            Self::Long(value) => value.to_string().into(),
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

pub(crate) fn format_error(field: Option<&str>, expected: &str, actual: &Node) -> Issue {
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
                    Scalar::Number(value) => {
                        if let Ok(int_val) = value.parse::<i32>() {
                            Node::Integer(int_val)
                        } else if let Ok(long_val) = value.parse::<i64>() {
                            Node::Long(long_val)
                        } else {
                            Node::Double(value.parse::<f64>().map_err(|_| {
                                unsupported("JSON number conversion is not qualified.")
                            })?)
                        }
                    }
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
    crate::host::resources::package_components(&value)
        .map_err(|_| unsupported("Invalid metadata URI exception projection is not qualified."))?;
    Ok(value)
}

fn package_uri(node: &Node, without_checksums: bool) -> Parsed<PackageUri> {
    let Node::String(value) = node else {
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
    let source_code_url_scheme = object.optional_string("sourceCodeUrlScheme")?;
    let source_code = if let Some(value) = object.optional_string("sourceCode")? {
        Some(parse_uri(value).map_err(|error| mapping(&"sourceCode".into(), error))?)
    } else {
        None
    };
    let documentation = if let Some(value) = object.optional_string("documentation")? {
        Some(parse_uri(value).map_err(|error| mapping(&"documentation".into(), error))?)
    } else {
        None
    };
    let license = object.optional_string("license")?;
    let license_text = object.optional_string("licenseText")?;
    let authors = if let Some(authors_node) = object.optional("authors") {
        Some(parse_authors(authors_node).map_err(|error| mapping(&"authors".into(), error))?)
    } else {
        None
    };
    let issue_tracker = if let Some(value) = object.optional_string("issueTracker")? {
        Some(parse_uri(value).map_err(|error| mapping(&"issueTracker".into(), error))?)
    } else {
        None
    };
    let description = object.optional_string("description")?;
    let annotations = if let Some(ann_node) = object.optional("annotations") {
        parse_annotations(ann_node).map_err(|error| mapping(&"annotations".into(), error))?
    } else {
        Vec::new()
    };
    Ok(DependencyMetadata {
        name,
        package_uri,
        version,
        package_zip_url,
        package_zip_sha256,
        dependencies,
        source_code_url_scheme,
        source_code,
        documentation,
        license,
        license_text,
        authors,
        issue_tracker,
        description,
        annotations,
    })
}

fn parse_annotations(node: &Node) -> Parsed<Vec<MetadataAnnotation>> {
    let Node::Array(values) = node else {
        return Err(format_error(None, "array", node));
    };
    let mut annotations = Vec::with_capacity(values.len());
    for elem in values {
        match parse_metadata_value(elem)? {
            MetadataValue::PObject(obj) => annotations.push(obj),
            other => {
                return Err(Issue::Java(PklString::from(format!(
                    "Could not read annotation. Invalid object: {other:?}"
                ))));
            }
        }
    }
    Ok(annotations)
}

fn is_valid_datasize_unit(symbol: &str) -> bool {
    matches!(
        symbol,
        "b" | "kb" | "kib" | "mb" | "mib" | "gb" | "gib" | "tb" | "tib" | "pb" | "pib"
    )
}

fn is_valid_duration_unit(symbol: &str) -> bool {
    matches!(symbol, "ns" | "us" | "ms" | "s" | "min" | "h" | "d")
}

fn parse_metadata_value(node: &Node) -> Parsed<MetadataValue> {
    match node {
        Node::Null => Ok(MetadataValue::Null),
        Node::Boolean(b) => Ok(MetadataValue::Bool(*b)),
        Node::String(s) => Ok(MetadataValue::String(s.clone())),
        Node::Integer(i) => Ok(MetadataValue::Int(i64::from(*i))),
        Node::Long(l) => Ok(MetadataValue::Int(*l)),
        Node::Double(d) => Ok(MetadataValue::Float(*d)),
        Node::Array(arr) => {
            let mut list = Vec::with_capacity(arr.len());
            for elem in arr {
                list.push(parse_metadata_value(elem)?);
            }
            Ok(MetadataValue::List(list))
        }
        Node::Object(obj) => {
            let ty = obj.string("type")?;
            match ty.as_str().unwrap_or("") {
                "Set" => {
                    let arr = obj.array_field("value")?;
                    let mut set = Vec::with_capacity(arr.len());
                    for elem in arr {
                        set.push(parse_metadata_value(elem)?);
                    }
                    Ok(MetadataValue::Set(set))
                }
                "Map" => {
                    let arr = obj.array_field("value")?;
                    let mut entries = Vec::with_capacity(arr.len());
                    for kv in arr {
                        let kv_obj = kv.object()?;
                        let k = kv_obj
                            .entries
                            .iter()
                            .find(|(k, _)| k == "key")
                            .map(|(_, v)| v)
                            .unwrap_or(&Node::Null);
                        let v = kv_obj
                            .entries
                            .iter()
                            .find(|(k, _)| k == "value")
                            .map(|(_, v)| v)
                            .unwrap_or(&Node::Null);
                        entries.push((parse_metadata_value(k)?, parse_metadata_value(v)?));
                    }
                    Ok(MetadataValue::Map(entries))
                }
                "PObject" => {
                    let class_info_obj = obj.object_field("classInfo")?;
                    let module_name = class_info_obj.string("moduleName")?;
                    let class_name = class_info_obj.string("class")?;
                    let module_uri = parse_uri(class_info_obj.string("moduleUri")?)?;
                    let props_obj = obj.object_field("properties")?;
                    let mut properties = Vec::with_capacity(props_obj.entries.len());
                    for (k, v) in &props_obj.entries {
                        properties.push((k.clone(), parse_metadata_value(v)?));
                    }
                    Ok(MetadataValue::PObject(MetadataAnnotation {
                        class_info: MetadataClassInfo {
                            module_name,
                            class_name,
                            module_uri,
                        },
                        properties,
                    }))
                }
                "Pattern" => {
                    let pattern = obj.string("value")?;
                    Ok(MetadataValue::Pattern(pattern))
                }
                "DataSize" => {
                    let unit = obj.string("unit")?;
                    let val_node = obj.required("value")?;
                    if !is_valid_datasize_unit(unit.as_str().unwrap_or("")) {
                        return Err(Issue::Java(PklString::from(format!(
                            "Invalid DataSize unit symbol: {unit}"
                        ))));
                    }
                    let num = match val_node {
                        Node::Double(d) => *d,
                        Node::Integer(i) => f64::from(*i),
                        Node::Long(l) => *l as f64,
                        other => return Err(format_error(None, "double", other)),
                    };
                    Ok(MetadataValue::DataSize { value: num, unit })
                }
                "Duration" => {
                    let unit = obj.string("unit")?;
                    let val_node = obj.required("value")?;
                    if !is_valid_duration_unit(unit.as_str().unwrap_or("")) {
                        return Err(Issue::Java(PklString::from(format!(
                            "Invalid Duration unit symbol: {unit}"
                        ))));
                    }
                    let num = match val_node {
                        Node::Double(d) => *d,
                        Node::Integer(i) => f64::from(*i),
                        Node::Long(l) => *l as f64,
                        other => return Err(format_error(None, "double", other)),
                    };
                    Ok(MetadataValue::Duration { value: num, unit })
                }
                "Pair" => {
                    let first = obj
                        .entries
                        .iter()
                        .find(|(k, _)| k == "first")
                        .map(|(_, v)| v)
                        .unwrap_or(&Node::Null);
                    let second = obj
                        .entries
                        .iter()
                        .find(|(k, _)| k == "second")
                        .map(|(_, v)| v)
                        .unwrap_or(&Node::Null);
                    Ok(MetadataValue::Pair(
                        Box::new(parse_metadata_value(first)?),
                        Box::new(parse_metadata_value(second)?),
                    ))
                }
                other => Err(Issue::Java(PklString::from(format!(
                    "Could not read annotation. Invalid object type: {other}"
                )))),
            }
        }
    }
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

fn parse_authors(node: &Node) -> Parsed<Vec<PklString>> {
    let Node::Array(values) = node else {
        return Err(format_error(None, "array", node));
    };
    let mut authors = Vec::with_capacity(values.len());
    for value in values {
        match value {
            Node::String(s) => authors.push(s.clone()),
            other => return Err(format_error(None, "string", other)),
        }
    }
    Ok(authors)
}
