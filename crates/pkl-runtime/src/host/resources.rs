//! Bounded preconfigured resource policy shared by the scalar development adapters.
//! Parsing, resolution, and authorization are separate from platform I/O. The
//! current profile has empty environment/property maps and no allowed-I/O reader.
use crate::pkl_string::PklStringBuilder;
use crate::source::{SourceCharacter, SourceSlice, SourceText};
use crate::value::{ErrorKind, PklString, RuntimeError, Value};
mod lossless_uri;
use crate::vm::ReadRequest;
use lossless_uri::ResourceUri;
pub(crate) mod glob;

pub(crate) fn default_result(request: &ReadRequest) -> Result<Value, RuntimeError> {
    configured_result(request, &[], &[])
}

pub(crate) fn resolved_uri(request: &ReadRequest) -> Result<PklString, RuntimeError> {
    if let Some(uri) = &request.resolved_uri {
        return Ok(uri.clone());
    }
    Ok(ResourceUri::from_authored(&request.uri)?
        .resolve(request.module_uri.as_ref())?
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

pub(crate) fn missing(authored: impl Into<PklString>) -> RuntimeError {
    RuntimeError::host_language(
        PklString::from("Cannot find resource `")
            .concat(&authored.into())
            .concat(&"`.".into()),
    )
}

pub(crate) fn configured_read(
    request: &ReadRequest,
    properties: &[(String, String)],
    environment: &[(String, String)],
) -> Result<crate::resources::ReadResult, RuntimeError> {
    let uri = ResourceUri::parse(&resolved_uri(request)?).expect("resolved resource URI");
    let scheme = uri.scheme.as_deref().expect("resolved URI is absolute");
    if matches!(scheme, "http" | "https") && !uri.has_supported_http_authority() {
        return Err(unsupported(
            "HTTP resource request construction for this URI form is not implemented.",
        ));
    }
    if !matches!(
        scheme,
        "prop" | "env" | "file" | "modulepath" | "package" | "projectpackage" | "https"
    ) {
        return Err(RuntimeError::host_language(PklString::from("Refusing to read resource `")
            .concat(&uri.text).concat(&"` because it does not match any entry in the resource allowlist (`--allowed-resources`).".into())));
    }
    if matches!(scheme, "env" | "prop") {
        let key = decoded_scheme_specific_part(&uri.text).expect("absolute URI");
        let entries = if scheme == "prop" {
            properties
        } else {
            environment
        };
        if let Some((_, value)) = entries.iter().rev().find(|(name, _)| key == name.as_str()) {
            return Ok(crate::resources::ReadResult::Text(value.as_str().into()));
        }
        return Ok(crate::resources::ReadResult::Missing);
    }
    Err(unsupported(
        PklString::from(format!(
            "The default resource Host does not yet implement allowed `{scheme}` reads (`"
        ))
        .concat(&uri.text)
        .concat(&"`).".into()),
    ))
}

/// Java URI equality is distinct from the spelling passed to a resource reader.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct CacheKey {
    scheme: Option<String>,
    opaque: bool,
    authority: Option<AuthorityKey>,
    path: PklString,
    query: Option<PklString>,
    fragment: Option<PklString>,
}
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum AuthorityKey {
    Registry(PklString),
    Server {
        user_info: Option<PklString>,
        host: String,
        port: Option<i32>,
    },
}

pub(crate) fn cache_key(uri: &PklString) -> CacheKey {
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

pub(crate) fn module_name_needs_fragment_paths(uri: &PklString) -> bool {
    let text = SourceText::from(uri.clone());
    !text
        .entire()
        .split_once(":")
        .and_then(|(scheme, _)| scheme.as_str())
        .is_some_and(|scheme| scheme.eq_ignore_ascii_case("jar"))
}

pub(crate) fn infer_module_name(
    uri: &PklString,
    fragment_paths: bool,
) -> Result<PklString, RuntimeError> {
    fn basename(path: &PklString) -> PklString {
        let text = SourceText::from(path.clone());
        let path = text.entire();
        let leaf = path.rsplit_once("/").map_or(path, |(_, leaf)| leaf);
        leaf.rsplit_once(".")
            .map_or(leaf, |(base, _)| base)
            .to_pkl_string()
    }
    let parsed = ResourceUri::parse(uri)
        .map_err(|_| RuntimeError::new(ErrorKind::Host, "Invalid selected ModuleKey URI."))?;
    if parsed
        .scheme
        .as_deref()
        .is_some_and(|scheme| scheme.eq_ignore_ascii_case("jar"))
    {
        let text = SourceText::from(uri.clone());
        return Ok(basename(
            &text
                .entire()
                .rsplit_once("!")
                .map_or(text.entire(), |(_, path)| path)
                .to_pkl_string(),
        ));
    }
    if fragment_paths {
        let fragment = parsed
            .fragment
            .as_ref()
            .ok_or_else(|| unsupported("ModuleKey fragment paths require a URI fragment."))?;
        return Ok(basename(&decode_uri_component(fragment, false)));
    }
    if parsed.opaque {
        let decoded = decoded_scheme_specific_part(uri)
            .ok_or_else(|| unsupported("Missing ModuleKey scheme specific part."))?;
        let text = SourceText::from(decoded);
        let view = text.entire();
        let unversioned = view.rsplit_once(":").map_or(view, |(name, _)| name);
        return Ok(unversioned
            .rsplit_once(".")
            .map_or(unversioned, |(_, name)| name)
            .to_pkl_string());
    }
    Ok(basename(&decode_escapes(&parsed.path)))
}

pub(crate) fn validate_module_key_uri(uri: &PklString) -> Result<(), RuntimeError> {
    if ResourceUri::parse(uri).is_ok_and(|parsed| parsed.scheme.is_some()) {
        Ok(())
    } else {
        Err(RuntimeError::new(
            ErrorKind::Host,
            "ModuleKey requires a valid absolute URI.",
        ))
    }
}

pub(crate) fn module_trust_level(uri: &PklString) -> Result<i32, crate::modules::ModuleHostError> {
    let mut text = uri.clone();
    loop {
        let parsed = ResourceUri::parse(&text).map_err(|_| {
            crate::modules::ModuleHostError::Unsupported(
                "Default module trust requires a valid absolute URI.".into(),
            )
        })?;
        return Ok(match parsed.scheme.as_deref() {
            Some("repl") => 40,
            Some("file") => {
                if parsed.authority.as_ref().is_some_and(|authority| {
                    !authority.is_empty() && server_authority(authority).is_some()
                }) {
                    10
                } else {
                    30
                }
            }
            Some("jar") => {
                let nested = decoded_scheme_specific_part(&text)
                    .is_some_and(|part| part.starts_with("nested:"));
                text = SourceText::from(text).entire().range(4..).to_pkl_string();
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

/// URI.getSchemeSpecificPart decodes escapes and excludes the fragment, while
/// the original spelling remains independent for trust operations and cache keys.
pub(crate) fn decoded_scheme_specific_part(uri: &PklString) -> Option<PklString> {
    ResourceUri::parse(uri).ok()?;
    let text = SourceText::from(uri.clone());
    let raw = text.entire().split_once(":")?.1;
    let raw = raw.split_once("#").map_or(raw, |(before, _)| before);
    Some(decode_escapes(&raw.to_pkl_string()))
}

/// URI.getPath is decoded once and absent for opaque URIs. A backslash in
/// authority, query, or fragment does not trigger ModuleCache's missing-path hint.
pub(crate) fn module_path_has_backslash(uri: &PklString) -> bool {
    ResourceUri::parse(uri)
        .is_ok_and(|parsed| !parsed.opaque && decode_escapes(&parsed.path).contains("\\"))
}

fn fold_escapes(text: &PklString) -> PklString {
    let source = SourceText::from(text.clone());
    let mut output = PklStringBuilder::default();
    let mut chars = source.characters();
    while let Some((_, c)) = chars.next() {
        match c {
            SourceCharacter::Scalar('%') => {
                output.push('%');
                for _ in 0..2 {
                    let (_, digit) = chars.next().expect("validated URI escape");
                    output.push(
                        digit
                            .scalar()
                            .expect("ASCII escape digit")
                            .to_ascii_uppercase(),
                    );
                }
            }
            SourceCharacter::Scalar(c) => output.push(c),
            SourceCharacter::Surrogate(unit) => output.push_unit(unit),
        }
    }
    output.finish()
}

fn server_authority(authority: &PklString) -> Option<AuthorityKey> {
    let source = SourceText::from(authority.clone());
    let authority = source.entire();
    let (user_info, host_port) = match authority.split_once("@") {
        Some((user, host)) if host.find("@").is_none() => {
            (Some(fold_escapes(&user.to_pkl_string())), host)
        }
        Some(_) => return None,
        None => (None, authority),
    };
    // A server host is ASCII/IPv6. Non-scalar authority data remains a registry
    // authority (and its complete units remain in the Registry cache key).
    let host_port = host_port.as_str()?;
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
pub(crate) fn read_triple_path(authored: &PklString) -> Result<Option<PklString>, RuntimeError> {
    triple_path(&ResourceUri::from_authored(authored)?, authored, "Resource")
}

pub(crate) fn import_triple_path(authored: &PklString) -> Result<Option<PklString>, RuntimeError> {
    let text = if is_uri_like(authored) {
        authored.clone()
    } else {
        quote_path(authored)
    };
    let parsed = ResourceUri::parse(&text)
        .map_err(|_| unsupported("Import URI must be parsed before preparation."))?;
    triple_path(&parsed, authored, "Module")
}

/// URI.relativize is applied after cutting the original receiver URI at its
/// final slash, including any query or fragment. Keep that ordering unchanged.
pub(crate) fn module_relative_path_elements(
    receiver: &PklString,
    other: &PklString,
) -> Result<Option<Vec<PklString>>, RuntimeError> {
    let receiver_text = SourceText::from(receiver.clone());
    let Some(slash) = receiver_text.entire().rfind("/") else {
        return Ok(None);
    };
    let base = ResourceUri::parse(&receiver_text.entire().range(..slash + 1).to_pkl_string())
        .map_err(|_| unsupported("Invalid original module key URI."))?;
    let child =
        ResourceUri::parse(other).map_err(|_| unsupported("Invalid other module key URI."))?;
    let authority =
        |value: Option<&PklString>| value.filter(|value| !value.is_empty()).map(fold_escapes);
    let same_scheme = match (&base.scheme, &child.scheme) {
        (Some(left), Some(right)) => left.eq_ignore_ascii_case(right),
        (None, None) => true,
        _ => false,
    };
    if base.opaque
        || child.opaque
        || !same_scheme
        || authority(base.authority.as_ref()) != authority(child.authority.as_ref())
    {
        return Ok(None);
    }
    let mut base_path = normalize_path(&base.path);
    let child_path = normalize_path(&child.path);
    if base_path != child_path && !base_path.ends_with("/") {
        base_path = base_path.concat(&"/".into());
    }
    let base_text = SourceText::from(base_path);
    let child_text = SourceText::from(child_path);
    if !child_text.as_bytes().starts_with(base_text.as_bytes()) {
        return Ok(None);
    }
    let remainder = child_text
        .entire()
        .range(base_text.len_bytes()..)
        .to_pkl_string();
    let decoded = SourceText::from(decode_escapes(&remainder));
    let mut elements = decoded
        .entire()
        .split("/")
        .map(SourceSlice::to_pkl_string)
        .collect::<Vec<_>>();
    if !decoded.is_empty() {
        while elements.last().is_some_and(PklString::is_empty) {
            elements.pop();
        }
    }
    Ok(Some(elements))
}

fn decode_escapes(text: &PklString) -> PklString {
    decode_uri_component(text, true)
}

// java.net.URI.decode copies literal Java chars unchanged and decodes each
// adjacent percent-byte run separately. SSP/path retain escapes in brackets;
// query/fragment getters explicitly pass false. Internal WTF-8 is never input
// to the UTF-8 decoder.
fn decode_uri_component(text: &PklString, ignore_percent_in_brackets: bool) -> PklString {
    let source = SourceText::from(text.clone());
    let view = source.entire();
    let mut output = PklStringBuilder::default();
    let mut offset = 0;
    let mut between_brackets = false;
    while offset < view.len_bytes() {
        let ch = view.range(offset..).first().expect("component character");
        match ch {
            SourceCharacter::Scalar('[') => between_brackets = true,
            SourceCharacter::Scalar(']') => between_brackets = false,
            _ => {}
        }
        if ch == SourceCharacter::Scalar('%') && !(between_brackets && ignore_percent_in_brackets) {
            let mut bytes = Vec::new();
            while view.as_bytes().get(offset) == Some(&b'%') {
                let pair = view
                    .range(offset + 1..offset + 3)
                    .as_str()
                    .expect("ASCII URI escape");
                bytes.push(u8::from_str_radix(pair, 16).expect("validated URI escape"));
                offset += 3;
            }
            output.push_value(&crate::resources::decode_utf8(&bytes));
        } else {
            match ch {
                SourceCharacter::Scalar(c) => output.push(c),
                SourceCharacter::Surrogate(unit) => output.push_unit(unit),
            }
            offset += ch.len_bytes();
        }
    }
    output.finish()
}

fn triple_path(
    uri: &ResourceUri,
    authored: &PklString,
    kind: &str,
) -> Result<Option<PklString>, RuntimeError> {
    if uri.scheme.is_some() {
        return Ok(None);
    }
    let text = SourceText::from(uri.text.clone());
    let raw = text
        .entire()
        .split_once("#")
        .map_or(text.entire(), |(raw, _)| raw);
    let ssp = decode_escapes(&raw.to_pkl_string());
    if !ssp.starts_with("...") {
        return Ok(None);
    }
    if ssp == "..." {
        return Ok(Some(PklString::default()));
    }
    let ssp_text = SourceText::from(ssp);
    if let Some(path) = ssp_text
        .entire()
        .strip_prefix(".../")
        .filter(|path| !path.is_empty())
    {
        return Ok(Some(path.to_pkl_string()));
    }
    let mut error = RuntimeError::host_language(
        PklString::from(format!("{kind} URI `"))
            .concat(authored)
            .concat(&"` has invalid syntax.".into()),
    );
    error.hint = Some("expected `...` or `.../path/to/my_module.pkl`".into());
    Err(error)
}

pub(crate) fn triple_candidates(
    module: &PklString,
    path: &PklString,
    capabilities: crate::modules::ModuleCapabilities,
) -> Result<Vec<PklString>, RuntimeError> {
    if !capabilities.local || !capabilities.hierarchical {
        let mut error = RuntimeError::host_language(
            PklString::from("Cannot resolve a triple-dot import from module URI `")
                .concat(module)
                .concat(&"`.".into()),
        );
        error.hint = Some("Triple-dot imports may only be resolved by module schemes that are considered local, and have hierarchical URIs.".into());
        error.unavailable_origin = true;
        return Err(error);
    }
    let base = ResourceUri::parse(module)
        .map_err(|_| unsupported("Unsupported triple-dot declaration URI."))?;
    let current = if capabilities.fragment_paths {
        decode_uri_component(
            base.fragment
                .as_ref()
                .ok_or_else(|| unsupported("Fragment-path ModuleKey has no fragment."))?,
            false,
        )
    } else {
        decode_escapes(&base.path)
    };
    let current_text = SourceText::from(current.clone());
    let view = current_text.entire();
    let effective = if path.is_empty() {
        view.rsplit_once("/")
            .map_or(view, |(_, leaf)| leaf)
            .to_pkl_string()
    } else {
        path.clone()
    };
    let mut index = view
        .rfind("/")
        .and_then(|last| view.range(..last).rfind("/"));
    let mut paths = Vec::new();
    while let Some(at) = index.filter(|at| *at > 0) {
        paths.push(view.range(..at + 1).to_pkl_string().concat(&effective));
        index = view.range(..at).rfind("/");
    }
    paths.push(PklString::from("/").concat(&effective));
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

pub(crate) fn prepare_module_uri(
    authored: &PklString,
    base: Option<&PklString>,
) -> Result<(PklString, PklString), RuntimeError> {
    let text = if is_uri_like(authored) {
        authored.clone()
    } else {
        quote_path(authored)
    };
    let parsed = ResourceUri::parse(&text).map_err(|error| match error {
        ParseError::Syntax(reason) => {
            let mut error = RuntimeError::host_language(
                PklString::from("Module URI `")
                    .concat(authored)
                    .concat(&"` has invalid syntax.".into()),
            );
            error.hint = Some(reason.into());
            error
        }
        ParseError::Unsupported(reason) => unsupported(reason),
    })?;
    if triple_path(&parsed, authored, "Module")?.is_some() {
        return Ok((parsed.text.clone(), parsed.text));
    }
    if parsed.scheme.is_none() && parsed.path.starts_with("@") {
        return Err(unsupported(
            "Module dependency and triple-dot resolution is not implemented.",
        ));
    }
    let parsed = if parsed.scheme.is_none() && base.is_none() {
        parsed
    } else {
        parsed.resolve_module(base)?
    };
    let absolute = parsed.text.clone();
    Ok((absolute, parsed.normalize().text))
}

pub(crate) fn resolve_module_key_uri(
    uri: &PklString,
    base: &PklString,
) -> Result<PklString, RuntimeError> {
    ResourceUri::parse(uri)
        .map_err(|_| unsupported("Invalid parsed module URI."))?
        .resolve_module(Some(base))
        .map(|uri| uri.text)
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

fn validate(text: SourceSlice<'_>, component: Component) -> Result<(), ParseError> {
    let mut chars = text.characters();
    while let Some((_, character)) = chars.next() {
        if character == SourceCharacter::Scalar('%') {
            if !chars
                .next()
                .is_some_and(|(_, c)| c.scalar().is_some_and(|c| c.is_ascii_hexdigit()))
                || !chars
                    .next()
                    .is_some_and(|(_, c)| c.scalar().is_some_and(|c| c.is_ascii_hexdigit()))
            {
                return Err(ParseError::Syntax("Malformed escape pair"));
            }
        } else if let SourceCharacter::Scalar(c) = character
            && !component.permits(c)
        {
            return Err(ParseError::Syntax(component.illegal()));
        }
        // URI.Parser.scanEscape admits a surrogate Java char: >128, neither
        // Character.isSpaceChar nor Character.isISOControl. Do not encode it.
    }
    Ok(())
}

fn validate_authority(text: SourceSlice<'_>) -> Result<(), ParseError> {
    if let Some(start) = text.find("[") {
        if let Some((user_info, _)) = text.split_once("@") {
            validate(user_info, Component::Authority)?;
        }
        if text.find("]").is_none() {
            return Err(ParseError::Syntax(
                "Expected closing bracket for IPv6 address",
            ));
        }
        if server_authority(&text.to_pkl_string()).is_none() {
            return Err(ParseError::Unsupported(
                "This IPv6 resource URI authority is not implemented.",
            ));
        }
        validate(text.range(..start), Component::Authority)?;
        return Ok(());
    }
    validate(text, Component::Authority)
}

fn is_uri_like(text: &PklString) -> bool {
    let source = SourceText::from(text.clone());
    let Some((scheme, rest)) = source.entire().split_once(":") else {
        return false;
    };
    !scheme.is_empty()
        && scheme
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || b"_+.-".contains(&c))
        && rest
            .first()
            .is_some_and(|c| c != SourceCharacter::Scalar('\\'))
        && !rest.characters().skip(1).any(|(_, c)| {
            matches!(
                c,
                SourceCharacter::Scalar('\n' | '\r' | '\u{85}' | '\u{2028}' | '\u{2029}')
            )
        })
}

pub(crate) fn quote_path(text: &PklString) -> PklString {
    let source = SourceText::from(text.clone());
    let mut result = PklStringBuilder::default();
    for (_, c) in source.characters() {
        match c {
            SourceCharacter::Surrogate(unit) => result.push_unit(unit),
            SourceCharacter::Scalar(c) if Component::Path.permits(c) => result.push(c),
            SourceCharacter::Scalar(c) => {
                let mut buffer = [0; 4];
                for byte in c.encode_utf8(&mut buffer).bytes() {
                    result.push_str(&format!("%{byte:02X}"));
                }
            }
        }
    }
    result.finish()
}

fn java_space(c: char) -> bool {
    matches!(
        c,
        '\u{a0}' | '\u{1680}' | '\u{2000}'
            ..='\u{200a}' | '\u{2028}' | '\u{2029}' | '\u{202f}' | '\u{205f}' | '\u{3000}'
    )
}

fn normalize_path(path: &PklString) -> PklString {
    let source = SourceText::from(path.clone());
    let path = source.entire();
    let absolute = path.starts_with("/");
    let mut segments: Vec<SourceSlice<'_>> = Vec::new();
    let mut trailing = path.ends_with("/");
    for segment in path.split("/") {
        match segment.as_str() {
            Some("") => {}
            Some(".") => trailing = true,
            Some("..")
                if segments
                    .last()
                    .is_some_and(|segment| segment.as_str() != Some("..")) =>
            {
                segments.pop();
                trailing = true;
            }
            _ => {
                segments.push(segment);
                trailing = false;
            }
        }
    }
    trailing |= path.ends_with("/");
    let mut result = PklStringBuilder::default();
    if !absolute
        && segments
            .first()
            .is_some_and(|segment| segment.find(":").is_some())
    {
        result.push_str("./");
    }
    if absolute {
        result.push('/');
    }
    for (index, segment) in segments.iter().enumerate() {
        if index > 0 {
            result.push('/');
        }
        segment.append_to(&mut result);
    }
    if trailing && !segments.is_empty() {
        result.push('/');
    }
    result.finish()
}

fn unsupported(message: impl Into<PklString>) -> RuntimeError {
    RuntimeError::new(ErrorKind::Unsupported, message.into())
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod lossless_contract;
