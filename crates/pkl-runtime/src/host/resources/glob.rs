//! URI operations used by shared glob traversal; never filesystem I/O.
use super::*;

pub(crate) fn parse_authored(text: &PklString) -> Result<PklString, RuntimeError> {
    Ok(ResourceUri::from_authored(text)?.text)
}

pub(crate) fn parse_base(text: &PklString) -> Result<PklString, RuntimeError> {
    ResourceUri::parse(text).map(|uri| uri.text).map_err(|_| {
        unsupported("Java URI.create failure in glob base resolution is not supported.")
    })
}

/// Validate the URI passed to the opaque reader after regex compilation.
/// This is the raw absolute pattern or the constructed relative dummy URI.
pub(crate) fn validate_opaque_list_uri(pattern: &PklString) -> Result<(), RuntimeError> {
    match ResourceUri::parse(pattern) {
        Ok(_) => Ok(()),
        Err(ParseError::Syntax(reason)) => {
            let index_utf16 = lossless_uri::opaque_syntax_index(pattern, reason);
            let mut error =
                unsupported("Java URI.create internal diagnostic rendering is not supported.");
            error.uri_create_cause = Some(crate::value::UriCreateCause {
                input: pattern.clone(),
                reason,
                index_utf16,
            });
            Err(error)
        }
        Err(ParseError::Unsupported(reason)) => Err(unsupported(reason)),
    }
}

pub(crate) fn normalize(uri: &PklString) -> Result<PklString, RuntimeError> {
    Ok(ResourceUri::parse(uri)
        .map_err(|_| unsupported("Invalid glob result URI."))?
        .normalize()
        .text)
}

pub(crate) fn absolute_pattern(pattern: &PklString) -> bool {
    let text = SourceText::from(pattern.clone());
    text.entire().split_once(":").is_some_and(|(scheme, rest)| {
        !scheme.is_empty()
            && scheme
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
            && !rest.characters().any(|(_, c)| {
                matches!(
                    c,
                    SourceCharacter::Scalar('\n' | '\r' | '\u{85}' | '\u{2028}' | '\u{2029}')
                )
            })
    })
}

pub(crate) fn split(
    pattern: &PklString,
    fragment_paths: bool,
) -> Result<(PklString, Vec<PklString>), RuntimeError> {
    let absolute = absolute_pattern(pattern);
    let mut base = PklStringBuilder::default();
    let effective = if absolute {
        let uri = ResourceUri::parse(pattern).map_err(|_| {
            unsupported("Java URI.create failure in glob pattern splitting is not supported.")
        })?;
        if fragment_paths {
            let source = SourceText::from(pattern.clone());
            source
                .entire()
                .split_once("#")
                .map_or(source.entire(), |(prefix, _)| prefix)
                .append_to(&mut base);
            base.push('#');
            decode_uri_component(
                uri.fragment.as_ref().ok_or_else(|| {
                    unsupported("Fragment-path glob without a fragment is not supported.")
                })?,
                false,
            )
        } else {
            let scheme = uri.scheme.as_deref().unwrap_or("");
            if uri.opaque {
                let mut error = RuntimeError::new(
                    ErrorKind::Type,
                    PklString::from("Invalid glob pattern `")
                        .concat(pattern)
                        .concat(&"`.".into()),
                );
                error.hint = Some(format!("Scheme `{scheme}` requires a hierarchical path (there must be a `/` after the first colon).").into());
                return Err(error);
            }
            base.push_str(scheme);
            base.push(':');
            decode_escapes(&uri.path)
        }
    } else {
        pattern.clone()
    };
    let effective_source = SourceText::from(effective);
    let mut parts = effective_source
        .entire()
        .split("/")
        .map(SourceSlice::to_pkl_string)
        .collect::<Vec<_>>();
    if !effective_source.is_empty() {
        while parts.last().is_some_and(PklString::is_empty) {
            parts.pop();
        }
    }
    let first = parts
        .iter()
        .position(|part| {
            part.encode_utf16()
                .any(|unit| matches!(unit, 0x5c | 0x7b | 0x5b | 0x3f | 0x2a))
        })
        .unwrap_or(parts.len());
    for part in &parts[..first] {
        base.push_value(part);
        base.push('/');
    }
    Ok((base.finish(), parts.split_off(first)))
}

pub(crate) fn resolve(
    base: &PklString,
    path: &PklString,
    fragment_paths: bool,
) -> Result<PklString, RuntimeError> {
    let target = ResourceUri::from_authored(path)?;
    if fragment_paths && target.scheme.is_none() {
        let base_uri =
            ResourceUri::parse(base).map_err(|_| unsupported("Invalid glob base URI."))?;
        let fragment = base_uri
            .fragment
            .ok_or_else(|| unsupported("Missing glob base URI fragment."))?;
        let resolved = target.resolve_module(Some(&fragment))?.text;
        let base_text = SourceText::from(base.clone());
        let prefix = base_text
            .entire()
            .split_once("#")
            .map_or(base_text.entire(), |(prefix, _)| prefix)
            .to_pkl_string();
        return Ok(prefix.concat(&"#".into()).concat(&resolved));
    }
    Ok(target.resolve_module(Some(base))?.text)
}

pub(crate) fn path(base: &PklString, name: &PklString, absolute: bool) -> PklString {
    if base.is_empty() {
        return name.clone();
    }
    let name = if absolute {
        quote_path(name)
    } else {
        name.clone()
    };
    let separator = if base.ends_with("/") { "" } else { "/" };
    base.concat(&separator.into()).concat(&name)
}

pub(crate) fn opaque_element(
    scheme: &PklString,
    name: &PklString,
) -> Result<PklString, RuntimeError> {
    // URI(scheme, ssp, null) quotes reserved syntax, not literal surrogates.
    let source = SourceText::from(name.clone());
    let mut quoted = PklStringBuilder::default();
    for (_, c) in source.characters() {
        match c {
            SourceCharacter::Surrogate(unit) => quoted.push_unit(unit),
            SourceCharacter::Scalar(c) if Component::Opaque.permits(c) && c != '%' => {
                quoted.push(c)
            }
            SourceCharacter::Scalar(c) => {
                let mut bytes = [0; 4];
                for byte in c.encode_utf8(&mut bytes).bytes() {
                    quoted.push_str(&format!("%{byte:02X}"));
                }
            }
        }
    }
    let uri = scheme.concat(&":".into()).concat(&quoted.finish());
    ResourceUri::parse(&uri)
        .map(|parsed| parsed.text)
        .map_err(|_| unsupported("Java opaque glob element URI construction is not supported."))
}

/// The scheme is an ASCII URI component; the remaining URI stays lossless.
pub(crate) fn scheme(uri: &PklString) -> Option<PklString> {
    let source = SourceText::from(uri.clone());
    source
        .entire()
        .split_once(":")
        .map(|(scheme, _)| scheme.to_pkl_string())
}
