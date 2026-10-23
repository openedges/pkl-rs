//! Bounded preconfigured resource policy shared by the scalar development adapters.
//! Parsing, resolution, and authorization are separate from platform I/O. The
//! current profile has empty environment/property maps and no allowed-I/O reader.
use crate::value::{ErrorKind, RuntimeError, Value};
use crate::vm::ReadRequest;

pub(crate) fn default_result(request: &ReadRequest) -> Result<Value, RuntimeError> {
    configured_result(request, &[], &[])
}

pub(crate) fn resolved_uri(request: &ReadRequest) -> Result<String, RuntimeError> {
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
    let uri = ResourceUri::from_authored(&request.uri)?
        .resolve(request.module_uri.as_deref())?
        .normalize();
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
        let key = String::from_utf8_lossy(&bytes);
        let entries = if scheme == "prop" {
            properties
        } else {
            environment
        };
        if let Some((_, value)) = entries.iter().rev().find(|(name, _)| name == key.as_ref()) {
            return Ok(Value::string(value.as_str()));
        }
        if request.nullable {
            return Ok(Value::Null);
        }
        return Err(RuntimeError::host_language(format!(
            "Cannot find resource `{}`.",
            request.uri
        )));
    }
    Err(unsupported(format!(
        "The default resource Host does not yet implement allowed `{scheme}` reads (`{}`).",
        uri.text
    )))
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
    if parsed.path == "..." || parsed.path.starts_with(".../") || parsed.path.starts_with('@') {
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
        if !text.contains(']') {
            return Err(ParseError::Syntax(
                "Expected closing bracket for IPv6 address",
            ));
        }
        // Do not authorize bracketed addresses without Java's IPv6/zone/port parser.
        return Err(ParseError::Unsupported(
            "IPv6 resource URI authority parsing is not implemented.",
        ));
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
