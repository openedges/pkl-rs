//! Bounded preconfigured resource policy shared by the scalar development adapters.
//! Parsing, resolution, and authorization are separate from platform I/O. The
//! current profile has empty environment/property maps and no allowed-I/O reader.
use crate::value::{ErrorKind, RuntimeError, Value};
use crate::vm::ReadRequest;

pub(crate) fn default_result(request: &ReadRequest) -> Result<Value, RuntimeError> {
    configured_result(request, &[], &[])
}

pub(crate) fn resolved_uri(request: &ReadRequest) -> Result<String, RuntimeError> {
    if let Some(uri) = &request.resolved_uri {
        return Ok(uri.to_string());
    }
    Ok(ResourceUri::from_authored(&request.uri)?
        .resolve(request.module_uri.as_deref())?
        .normalize()
        .text)
}

pub(crate) fn configured_result(
    request: &ReadRequest,
    properties: &[(String, String)],
    environment: &[(String, String)],
) -> Result<Value, RuntimeError> {
    match configured_read(request, properties, environment)? {
        crate::resources::ReadResult::Text(value) => Ok(Value::String(value)),
        crate::resources::ReadResult::Missing if request.nullable => Ok(Value::Null),
        crate::resources::ReadResult::Missing => Err(missing(&request.uri)),
        crate::resources::ReadResult::Resource(_) => unreachable!("configured scalar readers"),
    }
}

pub(crate) fn missing(authored: &str) -> RuntimeError {
    RuntimeError::host_language(format!("Cannot find resource `{authored}`."))
}

pub(crate) fn configured_read(
    request: &ReadRequest,
    properties: &[(String, String)],
    environment: &[(String, String)],
) -> Result<crate::resources::ReadResult, RuntimeError> {
    let uri = ResourceUri::parse(&resolved_uri(request)?).expect("resolved resource URI");
    let scheme = uri.scheme.as_deref().expect("resolved URI is absolute");
    if matches!(scheme, "http" | "https") && !uri.has_supported_http_authority() {
        // These registered readers construct HttpRequest before checking policy.
        // Registry authorities and opaque HTTP URIs can fail at that earlier step.
        return Err(unsupported(
            "HTTP resource request construction for this URI form is not implemented.",
        ));
    }
    if !matches!(
        scheme,
        "prop" | "env" | "file" | "modulepath" | "package" | "projectpackage" | "https"
    ) {
        return Err(RuntimeError::host_language(format!(
            "Refusing to read resource `{}` because it does not match any entry in the resource allowlist (`--allowed-resources`).",
            uri.text
        )));
    }
    if matches!(scheme, "env" | "prop") {
        // URI.getSchemeSpecificPart decodes escapes; property/environment maps
        // are explicit evaluator inputs, never process environment lookups.
        let raw = uri
            .text
            .split_once(':')
            .expect("absolute URI")
            .1
            .split('#')
            .next()
            .unwrap();
        let mut bytes = Vec::with_capacity(raw.len());
        let mut index = 0;
        while index < raw.len() {
            if raw.as_bytes()[index] == b'%' {
                bytes.push(
                    u8::from_str_radix(&raw[index + 1..index + 3], 16).expect("validated escape"),
                );
                index += 3;
            } else {
                bytes.push(raw.as_bytes()[index]);
                index += 1;
            }
        }
        let key = crate::resources::decode_utf8(&bytes);
        let entries = if scheme == "prop" {
            properties
        } else {
            environment
        };
        if let Some((_, value)) = entries
            .iter()
            .rev()
            .find(|(name, _)| Some(name.as_str()) == key.as_str())
        {
            return Ok(crate::resources::ReadResult::Text(value.as_str().into()));
        }
        return Ok(crate::resources::ReadResult::Missing);
    }
    Err(unsupported(format!(
        "The default resource Host does not yet implement allowed `{scheme}` reads (`{}`).",
        uri.text
    )))
}

/// Java URI equality is distinct from the spelling passed to a resource reader.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct CacheKey {
    scheme: Option<String>,
    opaque: bool,
    authority: Option<AuthorityKey>,
    path: String,
    query: Option<String>,
    fragment: Option<String>,
}
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum AuthorityKey {
    Registry(String),
    Server {
        user_info: Option<String>,
        host: String,
        port: Option<i32>,
    },
}

pub(crate) fn cache_key(uri: &str) -> CacheKey {
    let parsed = ResourceUri::parse(uri).expect("resolved resource URI was parsed");
    CacheKey {
        scheme: parsed.scheme.map(|scheme| scheme.to_ascii_lowercase()),
        opaque: parsed.opaque,
        authority: parsed.authority.filter(|a| !a.is_empty()).map(|authority| {
            server_authority(&authority)
                .unwrap_or_else(|| AuthorityKey::Registry(fold_escapes(&authority)))
        }),
        path: fold_escapes(&parsed.path),
        query: parsed.query.map(|part| fold_escapes(&part)),
        fragment: parsed.fragment.map(|part| fold_escapes(&part)),
    }
}

/// SecurityManagers.getDefaultTrustLevel uses the resolved module URI, not a
/// diagnostic URI or resource-reader key. The nested marker uses decoded SSP;
/// other jar URLs are peeled without recursion or rewriting the embedded URI.
pub(crate) fn module_trust_level(uri: &str) -> Result<i32, crate::modules::ModuleHostError> {
    let mut text = uri;
    loop {
        let parsed = ResourceUri::parse(text).map_err(|_| {
            crate::modules::ModuleHostError::Unsupported(
                "Default module trust requires a valid absolute URI.".into(),
            )
        })?;
        return Ok(match parsed.scheme.as_deref() {
            Some("repl") => 40,
            Some("file") => {
                if parsed.authority.as_deref().is_some_and(|authority| {
                    !authority.is_empty() && server_authority(authority).is_some()
                }) {
                    10
                } else {
                    30
                }
            }
            Some("jar") => {
                let nested = decoded_scheme_specific_part(text)
                    .and_then(|part| part.as_str().map(|part| part.starts_with("nested:")))
                    .unwrap_or(false);
                text = &text[4..];
                if nested {
                    20
                } else {
                    continue;
                }
            }
            Some("modulepath") => 20,
            Some("pkl") => 0,
            Some(_) => 10,
            None => {
                return Err(crate::modules::ModuleHostError::Unsupported(
                    "Default module trust requires an absolute URI.".into(),
                ));
            }
        });
    }
}

/// URI.getSchemeSpecificPart decodes escapes and excludes the fragment. The
/// original URI spelling remains separate for trust events and cache keys.
pub(crate) fn decoded_scheme_specific_part(uri: &str) -> Option<crate::value::PklString> {
    ResourceUri::parse(uri).ok()?;
    let raw = uri.split_once(':')?.1.split('#').next()?;
    let mut bytes = Vec::with_capacity(raw.len());
    let mut index = 0;
    while index < raw.len() {
        if raw.as_bytes()[index] == b'%' {
            bytes.push(u8::from_str_radix(&raw[index + 1..index + 3], 16).ok()?);
            index += 3;
        } else {
            bytes.push(raw.as_bytes()[index]);
            index += 1;
        }
    }
    Some(crate::resources::decode_utf8(&bytes))
}

fn fold_escapes(text: &str) -> String {
    let mut bytes = text.as_bytes().to_vec();
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' {
            bytes[index + 1].make_ascii_uppercase();
            bytes[index + 2].make_ascii_uppercase();
            index += 3;
        } else {
            index += 1;
        }
    }
    String::from_utf8(bytes).expect("ASCII case folding preserves UTF-8")
}

fn server_authority(authority: &str) -> Option<AuthorityKey> {
    let (user_info, host_port) = match authority.split_once('@') {
        Some((user, host)) if !host.contains('@') => (Some(fold_escapes(user)), host),
        Some(_) => return None,
        None => (None, authority),
    };
    let (host, port) = if host_port.starts_with('[') {
        let end = host_port.find(']')?;
        let host = &host_port[..=end];
        host[1..end].parse::<std::net::Ipv6Addr>().ok()?;
        let suffix = &host_port[end + 1..];
        (
            host,
            if suffix.is_empty() {
                None
            } else {
                Some(suffix.strip_prefix(':')?)
            },
        )
    } else {
        let (host, port) = host_port
            .split_once(':')
            .map_or((host_port, None), |(host, port)| (host, Some(port)));
        if !server_hostname(host) {
            return None;
        }
        (host, port)
    };
    let port = match port {
        None | Some("") => None,
        Some(port) if port.bytes().all(|byte| byte.is_ascii_digit()) => {
            Some(port.parse::<i32>().ok()?)
        }
        Some(_) => return None,
    };
    Some(AuthorityKey::Server {
        user_info,
        host: host.to_ascii_lowercase(),
        port,
    })
}

fn server_hostname(host: &str) -> bool {
    // URI first attempts a four-part decimal IPv4 address. In particular,
    // leading zeroes are allowed; host spelling is still significant in equals.
    let parts = host.split('.').collect::<Vec<_>>();
    if parts.len() == 4
        && parts.iter().all(|part| {
            !part.is_empty()
                && part.bytes().all(|c| c.is_ascii_digit())
                && part.parse::<u8>().is_ok()
        })
    {
        return true;
    }
    let host = host.strip_suffix('.').unwrap_or(host);
    let labels = host.split('.').collect::<Vec<_>>();
    !host.is_empty()
        && labels.iter().all(|label| {
            !label.is_empty()
                && label.as_bytes()[0].is_ascii_alphanumeric()
                && label.as_bytes()[label.len() - 1].is_ascii_alphanumeric()
                && label
                    .bytes()
                    .all(|c| c.is_ascii_alphanumeric() || c == b'-')
        })
        && (labels.len() == 1 || labels.last().unwrap().as_bytes()[0].is_ascii_alphabetic())
}

/// Return an authored triple-dot path. Parsing happens before ModuleKey I/O.
pub(crate) fn read_triple_path(authored: &str) -> Result<Option<String>, RuntimeError> {
    triple_path(&ResourceUri::from_authored(authored)?, authored, "Resource")
}

pub(crate) fn import_triple_path(authored: &str) -> Result<Option<String>, RuntimeError> {
    let text = if is_uri_like(authored) {
        authored.to_owned()
    } else {
        quote_path(authored)
    };
    let parsed = ResourceUri::parse(&text)
        .map_err(|_| unsupported("Import URI must be parsed before preparation."))?;
    triple_path(&parsed, authored, "Module")
}

fn decode_escapes(text: &str) -> String {
    let mut bytes = Vec::new();
    let mut index = 0;
    while index < text.len() {
        if text.as_bytes()[index] == b'%' {
            bytes.push(
                u8::from_str_radix(&text[index + 1..index + 3], 16).expect("validated escape"),
            );
            index += 3;
        } else {
            bytes.push(text.as_bytes()[index]);
            index += 1;
        }
    }
    crate::resources::decode_utf8(&bytes)
        .as_str()
        .expect("UTF8 decoder produces scalars")
        .to_owned()
}

fn triple_path(
    uri: &ResourceUri,
    authored: &str,
    kind: &str,
) -> Result<Option<String>, RuntimeError> {
    if uri.scheme.is_some() {
        return Ok(None);
    }
    let ssp = decode_escapes(uri.text.split('#').next().unwrap());
    if !ssp.starts_with("...") {
        return Ok(None);
    }
    if ssp == "..." {
        return Ok(Some(String::new()));
    }
    if let Some(path) = ssp.strip_prefix(".../").filter(|path| !path.is_empty()) {
        return Ok(Some(path.to_owned()));
    }
    let mut error =
        RuntimeError::host_language(format!("{kind} URI `{authored}` has invalid syntax."));
    error.hint = Some("expected `...` or `.../path/to/my_module.pkl`".into());
    Err(error)
}

/// Candidate order belongs to the shared engine. The Host performs only the
/// original ModuleKey's `hasElement`/security operation for each candidate.
pub(crate) fn triple_candidates(
    module: &str,
    path: &str,
    capabilities: crate::modules::ModuleCapabilities,
) -> Result<Vec<String>, RuntimeError> {
    if !capabilities.local || !capabilities.hierarchical {
        let mut error = RuntimeError::host_language(format!(
            "Cannot resolve a triple-dot import from module URI `{module}`."
        ));
        error.hint = Some("Triple-dot imports may only be resolved by module schemes that are considered local, and have hierarchical URIs.".into());
        // IoUtils constructs this VmException without a node/source section.
        error.unavailable_origin = true;
        return Err(error);
    }
    let base = ResourceUri::parse(module)
        .map_err(|_| unsupported("Unsupported triple-dot declaration URI."))?;
    let current = decode_escapes(if capabilities.fragment_paths {
        base.fragment
            .as_deref()
            .ok_or_else(|| unsupported("Fragment-path ModuleKey has no fragment."))?
    } else {
        &base.path
    });
    let effective = if path.is_empty() {
        current.rsplit('/').next().unwrap()
    } else {
        path
    };
    let mut index = current
        .rfind('/')
        .and_then(|last| current[..last].rfind('/'));
    let mut paths = Vec::new();
    while let Some(at) = index.filter(|at| *at > 0) {
        paths.push(format!("{}{effective}", &current[..=at]));
        index = current[..at].rfind('/');
    }
    paths.push(format!("/{effective}"));
    let mut result = Vec::new();
    for candidate in paths.into_iter().filter(|candidate| candidate != &current) {
        let mut uri = base.clone();
        if capabilities.fragment_paths {
            uri.fragment = Some(quote_path(&normalize_path(&candidate)));
        } else {
            uri.path = quote_path(&normalize_path(&candidate));
            uri.query = None;
            uri.fragment = None;
        }
        uri.rebuild();
        result.push(uri.text);
    }
    Ok(result)
}

/// Shared URI grammar for module imports. Preparation is eager; Host resolution
/// and source loading remain lazy operations with separate cache boundaries.
pub(crate) fn prepare_module_uri(
    authored: &str,
    base: Option<&str>,
) -> Result<(String, String), RuntimeError> {
    let text = if is_uri_like(authored) {
        authored.to_owned()
    } else {
        quote_path(authored)
    };
    let parsed = ResourceUri::parse(&text).map_err(|error| match error {
        ParseError::Syntax(reason) => {
            let mut error =
                RuntimeError::host_language(format!("Module URI `{authored}` has invalid syntax."));
            error.hint = Some(reason.into());
            error
        }
        ParseError::Unsupported(reason) => unsupported(reason),
    })?;
    if triple_path(&parsed, authored, "Module")?.is_some() {
        // Module AST preparation resolves these with the original ModuleKey
        // before guest initialization, including unforced import expressions.
        return Ok((parsed.text.clone(), parsed.text));
    }
    if parsed.path.starts_with('@') {
        return Err(unsupported(
            "Module dependency and triple-dot resolution is not implemented.",
        ));
    }
    // A source-less compiler can retain a relative import until its primary
    // ModuleKey is registered, while still rejecting malformed URI syntax now.
    let parsed = if parsed.scheme.is_none() && base.is_none() {
        parsed
    } else {
        parsed.resolve(base)?
    };
    let absolute = parsed.text.clone();
    Ok((absolute, parsed.normalize().text))
}

#[derive(Clone, Debug)]
struct ResourceUri {
    text: String,
    scheme: Option<String>,
    authority: Option<String>,
    path: String,
    query: Option<String>,
    fragment: Option<String>,
    opaque: bool,
}

impl ResourceUri {
    fn from_authored(authored: &str) -> Result<Self, RuntimeError> {
        let text = if is_uri_like(authored) {
            authored.to_owned()
        } else {
            quote_path(authored)
        };
        let uri = Self::parse(&text).map_err(|error| match error {
            ParseError::Syntax(reason) => {
                let mut error = RuntimeError::host_language(format!(
                    "Resource URI `{authored}` has invalid syntax."
                ));
                error.hint = Some(reason.into());
                error
            }
            ParseError::Unsupported(reason) => unsupported(reason),
        })?;
        if !is_uri_like(authored)
            && uri
                .authority
                .as_ref()
                .is_some_and(|authority| !authority.is_empty())
        {
            // The path constructor requests server-authority validation, unlike
            // the URI-string constructor's registry-authority fallback.
            return Err(unsupported(
                "Network-path resource authority parsing is not implemented.",
            ));
        }
        Ok(uri)
    }

    fn has_supported_http_authority(&self) -> bool {
        if self.opaque {
            return false;
        }
        let Some(authority) = &self.authority else {
            return false;
        };
        let (host, port) = authority
            .split_once(':')
            .map_or((authority.as_str(), None), |(host, port)| {
                (host, Some(port))
            });
        if port.is_some_and(|port| {
            !port.is_empty()
                && (!port.bytes().all(|c| c.is_ascii_digit()) || port.parse::<i32>().is_err())
        }) {
            return false;
        }
        // URI.getHost must be non-null before HttpRequest can be constructed.
        // IPv6, user info and non-ASCII hosts remain separate parsing work.
        let host = host.strip_suffix('.').unwrap_or(host);
        if host.contains('.')
            && host
                .rsplit('.')
                .next()
                .is_some_and(|label| label.starts_with(|c: char| c.is_ascii_digit()))
        {
            return host.parse::<std::net::Ipv4Addr>().is_ok();
        }
        !host.is_empty()
            && host.split('.').all(|label| {
                !label.is_empty()
                    && label.as_bytes()[0].is_ascii_alphanumeric()
                    && label.as_bytes()[label.len() - 1].is_ascii_alphanumeric()
                    && label
                        .bytes()
                        .all(|c| c.is_ascii_alphanumeric() || c == b'-')
            })
    }

    fn parse(text: &str) -> Result<Self, ParseError> {
        let (body, fragment) = text
            .split_once('#')
            .map_or((text, None), |(a, b)| (a, Some(b)));
        // java.net.URI recognizes a scheme only before the first /, ? or #.
        let colon = body.find(':').filter(|colon| {
            body.find(['/', '?'])
                .is_none_or(|delimiter| *colon < delimiter)
        });
        let (scheme, rest) = if let Some(colon) = colon {
            let scheme = &body[..colon];
            if scheme.is_empty() {
                return Err(ParseError::Syntax("Expected scheme name"));
            }
            if !scheme.as_bytes()[0].is_ascii_alphabetic()
                || !scheme
                    .bytes()
                    .all(|c| c.is_ascii_alphanumeric() || b"+-.".contains(&c))
            {
                return Err(ParseError::Syntax("Illegal character in scheme name"));
            }
            let rest = &body[colon + 1..];
            if rest.is_empty() {
                return Err(ParseError::Syntax("Expected scheme-specific part"));
            }
            (Some(scheme.to_owned()), rest)
        } else {
            (None, body)
        };
        let opaque = scheme.is_some() && !rest.starts_with('/');
        let (authority, path, query) = if opaque {
            validate(rest, Component::Opaque)?;
            (None, rest, None)
        } else {
            let (hierarchy, query) = rest
                .split_once('?')
                .map_or((rest, None), |(a, b)| (a, Some(b)));
            let (authority, path) = if let Some(without_prefix) = hierarchy.strip_prefix("//") {
                let (authority, path) = without_prefix
                    .find('/')
                    .map_or((without_prefix, ""), |slash| {
                        (&without_prefix[..slash], &without_prefix[slash..])
                    });
                if authority.is_empty() && path.is_empty() && query.is_none() && fragment.is_none()
                {
                    return Err(ParseError::Syntax("Expected authority"));
                }
                validate_authority(authority)?;
                (Some(authority.to_owned()), path)
            } else {
                (None, hierarchy)
            };
            validate(path, Component::Path)?;
            if let Some(query) = query {
                validate(query, Component::Query)?;
            }
            (authority, path, query)
        };
        if let Some(fragment) = fragment {
            validate(fragment, Component::Fragment)?;
        }
        Ok(Self {
            text: text.into(),
            scheme,
            authority,
            path: path.into(),
            query: query.map(str::to_owned),
            fragment: fragment.map(str::to_owned),
            opaque,
        })
    }

    fn resolve(mut self, module_uri: Option<&str>) -> Result<Self, RuntimeError> {
        if self.scheme.is_some() {
            return Ok(self);
        }
        if self.path == "..." || self.path.starts_with(".../") || self.path.starts_with('@') {
            return Err(unsupported(
                "Resource dependency and triple-dot resolution is not implemented.",
            ));
        }
        let base = module_uri.ok_or_else(|| {
            unsupported("Relative resource reads require declaration module URI metadata.")
        })?;
        let base = Self::parse(base).map_err(|_| {
            unsupported(
                "The declaration module URI is not supported for relative resource resolution.",
            )
        })?;
        if base.opaque || base.scheme.as_deref() != Some("file") {
            return Err(unsupported(
                "Relative resource resolution currently requires a hierarchical file module URI.",
            ));
        }
        if self.authority.is_none() {
            self.authority.clone_from(&base.authority);
            if !self.path.starts_with('/') {
                let directory = base
                    .path
                    .rfind('/')
                    .map_or("", |slash| &base.path[..=slash]);
                self.path = format!("{directory}{}", self.path);
            }
        }
        self.scheme = base.scheme;
        self.path = normalize_path(&self.path);
        // IoUtils.fixTripleSlashUri preserves a file module's empty authority.
        self.rebuild();
        Ok(self)
    }

    fn normalize(mut self) -> Self {
        if self.opaque {
            return self;
        }
        let path = normalize_path(&self.path);
        if path != self.path {
            self.path = path;
            // URI.normalize reconstructs the URI. An empty parsed authority is
            // absent in java.net.URI, even when the original text used ///.
            if self.authority.as_deref() == Some("") {
                self.authority = None;
            }
            self.rebuild();
        }
        self
    }

    fn rebuild(&mut self) {
        let mut text = String::new();
        if let Some(scheme) = &self.scheme {
            text.push_str(scheme);
            text.push(':');
        }
        if let Some(authority) = &self.authority {
            text.push_str("//");
            text.push_str(authority);
        }
        text.push_str(&self.path);
        if let Some(query) = &self.query {
            text.push('?');
            text.push_str(query);
        }
        if let Some(fragment) = &self.fragment {
            text.push('#');
            text.push_str(fragment);
        }
        self.text = text;
    }
}

#[derive(Debug)]
enum ParseError {
    Syntax(&'static str),
    Unsupported(&'static str),
}

#[derive(Clone, Copy)]
enum Component {
    Opaque,
    Authority,
    Path,
    Query,
    Fragment,
}

impl Component {
    fn illegal(self) -> &'static str {
        match self {
            Self::Opaque => "Illegal character in opaque part",
            Self::Authority => "Illegal character in authority",
            Self::Path => "Illegal character in path",
            Self::Query => "Illegal character in query",
            Self::Fragment => "Illegal character in fragment",
        }
    }
    fn permits(self, c: char) -> bool {
        if c.is_ascii_alphanumeric() || "_-!.~'()*;/:@&=+$,".contains(c) {
            return true;
        }
        if !c.is_ascii() {
            return !c.is_control() && !java_space(c);
        }
        matches!(self, Self::Opaque | Self::Query | Self::Fragment) && "?[]".contains(c)
    }
}

fn validate(text: &str, component: Component) -> Result<(), ParseError> {
    let mut chars = text.chars();
    while let Some(c) = chars.next() {
        if c == '%' {
            if !chars.next().is_some_and(|c| c.is_ascii_hexdigit())
                || !chars.next().is_some_and(|c| c.is_ascii_hexdigit())
            {
                return Err(ParseError::Syntax("Malformed escape pair"));
            }
        } else if !component.permits(c) {
            return Err(ParseError::Syntax(component.illegal()));
        }
    }
    Ok(())
}

fn validate_authority(text: &str) -> Result<(), ParseError> {
    if text.contains('[') {
        // Validate user-info before parsing an authority key. Escape folding
        // assumes syntactically complete percent pairs, including on failures.
        if let Some((user_info, _)) = text.split_once('@') {
            validate(user_info, Component::Authority)?;
        }
        if !text.contains(']') {
            return Err(ParseError::Syntax(
                "Expected closing bracket for IPv6 address",
            ));
        }
        // Scoped IPv6 address grammar has separate Java compatibility rules.
        // Keep it explicit until those rules are supported, rather than treating
        // a bracketed address as a case-sensitive registry authority.
        if server_authority(text).is_none() {
            return Err(ParseError::Unsupported(
                "This IPv6 resource URI authority is not implemented.",
            ));
        }
        let start = text.find('[').unwrap();
        validate(&text[..start], Component::Authority)?;
        return Ok(());
    }
    validate(text, Component::Authority)
}

fn is_uri_like(text: &str) -> bool {
    let Some((scheme, rest)) = text.split_once(':') else {
        return false;
    };
    !scheme.is_empty()
        && scheme
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || b"_+.-".contains(&c))
        && rest.chars().next().is_some_and(|c| c != '\\')
        && !rest
            .chars()
            .skip(1)
            .any(|c| matches!(c, '\n' | '\r' | '\u{85}' | '\u{2028}' | '\u{2029}'))
}

pub(crate) fn quote_path(text: &str) -> String {
    let mut result = String::new();
    for c in text.chars() {
        if Component::Path.permits(c) {
            result.push(c);
        } else {
            let mut buffer = [0; 4];
            for byte in c.encode_utf8(&mut buffer).bytes() {
                use std::fmt::Write;
                write!(result, "%{byte:02X}").expect("writing a String");
            }
        }
    }
    result
}

fn java_space(c: char) -> bool {
    matches!(
        c,
        '\u{a0}' | '\u{1680}' | '\u{2000}'
            ..='\u{200a}' | '\u{2028}' | '\u{2029}' | '\u{202f}' | '\u{205f}' | '\u{3000}'
    )
}

fn normalize_path(path: &str) -> String {
    let absolute = path.starts_with('/');
    let mut segments = Vec::new();
    let mut trailing = path.ends_with('/');
    for segment in path.split('/') {
        match segment {
            "" => {}
            "." => trailing = true,
            ".." if segments.last().is_some_and(|segment| *segment != "..") => {
                segments.pop();
                trailing = true;
            }
            segment => {
                segments.push(segment);
                trailing = false;
            }
        }
    }
    trailing |= path.ends_with('/');
    let mut result = if absolute { "/".into() } else { String::new() };
    result.push_str(&segments.join("/"));
    if trailing && !result.is_empty() && !result.ends_with('/') {
        result.push('/');
    }
    // Preserve a relative first segment containing ':' as a path, not a scheme.
    if !absolute
        && segments
            .first()
            .is_some_and(|segment| segment.contains(':'))
    {
        result.insert_str(0, "./");
    }
    result
}

fn unsupported(message: impl Into<String>) -> RuntimeError {
    RuntimeError::new(ErrorKind::Unsupported, message.into())
}

#[cfg(test)]
mod tests;
