//! URI components retain authored Java strings; ASCII syntax uses a source view.
//! This is the shared URI implementation, not a platform conversion or URI codec.
use super::*;
use crate::pkl_string::PklStringBuilder;
use crate::source::{SourceSlice, SourceText};
use crate::value::PklString;

#[derive(Clone, Debug)]
pub(super) struct ResourceUri {
    pub(super) text: PklString,
    pub(super) scheme: Option<String>,
    pub(super) authority: Option<PklString>,
    pub(super) path: PklString,
    pub(super) query: Option<PklString>,
    pub(super) fragment: Option<PklString>,
    pub(super) opaque: bool,
}

impl ResourceUri {
    pub(super) fn from_authored(authored: &PklString) -> Result<Self, RuntimeError> {
        let text = if is_uri_like(authored) {
            authored.clone()
        } else {
            quote_path(authored)
        };
        let uri = Self::parse(&text).map_err(|error| match error {
            ParseError::Syntax(reason) => {
                let mut error = RuntimeError::host_language(
                    PklString::from("Resource URI `")
                        .concat(authored)
                        .concat(&"` has invalid syntax.".into()),
                );
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
            return Err(unsupported(
                "Network-path resource authority parsing is not implemented.",
            ));
        }
        Ok(uri)
    }

    pub(super) fn has_supported_http_authority(&self) -> bool {
        if self.opaque {
            return false;
        }
        let Some(authority) = self.authority.as_ref().and_then(PklString::as_str) else {
            return false;
        };
        let (host, port) = authority
            .split_once(':')
            .map_or((authority, None), |(host, port)| (host, Some(port)));
        if port.is_some_and(|port| {
            !port.is_empty()
                && (!port.bytes().all(|c| c.is_ascii_digit()) || port.parse::<i32>().is_err())
        }) {
            return false;
        }
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

    pub(super) fn parse(text: &PklString) -> Result<Self, ParseError> {
        let source = SourceText::from(text.clone());
        let view = source.entire();
        let (body, fragment) = view
            .split_once("#")
            .map_or((view, None), |(a, b)| (a, Some(b)));
        let colon = body.find(":").filter(|colon| {
            [body.find("/"), body.find("?")]
                .into_iter()
                .flatten()
                .min()
                .is_none_or(|delimiter| *colon < delimiter)
        });
        let (scheme, rest) = if let Some(colon) = colon {
            let scheme = body.range(..colon);
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
            let rest = body.range(colon + 1..);
            if rest.is_empty() {
                return Err(ParseError::Syntax("Expected scheme-specific part"));
            }
            (
                Some(scheme.as_str().expect("ASCII scheme").to_owned()),
                rest,
            )
        } else {
            (None, body)
        };
        let opaque = scheme.is_some() && !rest.starts_with("/");
        let (authority, path, query) = if opaque {
            validate(rest, Component::Opaque)?;
            (None, rest, None)
        } else {
            let (hierarchy, query) = rest
                .split_once("?")
                .map_or((rest, None), |(a, b)| (a, Some(b)));
            let (authority, path) = if let Some(without_prefix) = hierarchy.strip_prefix("//") {
                let (authority, path) = without_prefix
                    .find("/")
                    .map_or((without_prefix, SourceSlice::from_utf8("")), |slash| {
                        (without_prefix.range(..slash), without_prefix.range(slash..))
                    });
                if authority.is_empty() && path.is_empty() && query.is_none() && fragment.is_none()
                {
                    return Err(ParseError::Syntax("Expected authority"));
                }
                validate_authority(authority)?;
                (Some(authority.to_pkl_string()), path)
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
            text: text.clone(),
            scheme,
            authority,
            path: path.to_pkl_string(),
            query: query.map(SourceSlice::to_pkl_string),
            fragment: fragment.map(SourceSlice::to_pkl_string),
            opaque,
        })
    }

    pub(super) fn resolve_module(
        mut self,
        module_uri: Option<&PklString>,
    ) -> Result<Self, RuntimeError> {
        if self.scheme.is_some() {
            return Ok(self);
        }
        let base_text = module_uri.ok_or_else(|| {
            unsupported("Relative module imports require declaration module URI metadata.")
        })?;
        let base =
            Self::parse(base_text).map_err(|_| unsupported("Invalid declaration module URI."))?;
        if base
            .scheme
            .as_deref()
            .is_some_and(|scheme| scheme.eq_ignore_ascii_case("jar"))
        {
            let source = SourceText::from(base_text.clone());
            let view = source.entire();
            let bang = view
                .find("!")
                .ok_or_else(|| unsupported("Invalid jar module URI (missing `!`)."))?;
            let inner = view.range(bang + 1..).to_pkl_string();
            let path = self.resolve_module(Some(&inner))?;
            return Self::parse(&view.range(..bang + 1).to_pkl_string().concat(&path.text))
                .map_err(|_| unsupported("Invalid resolved jar module URI."));
        }
        if base.opaque {
            return Ok(self);
        }
        if self.authority.is_none() {
            self.authority = base.authority;
            if self.path.is_empty() && self.fragment.is_some() && self.query.is_none() {
                self.path = base.path;
                self.query = base.query;
            } else if !self.path.starts_with("/") {
                let source = SourceText::from(base.path);
                let view = source.entire();
                let directory = view
                    .rfind("/")
                    .map_or(SourceSlice::from_utf8(""), |slash| view.range(..slash + 1));
                let mut path = PklStringBuilder::default();
                directory.append_to(&mut path);
                if directory.is_empty() && self.authority.is_some() {
                    path.push('/');
                }
                path.push_value(&self.path);
                self.path = normalize_path(&path.finish());
            }
        }
        self.scheme = base.scheme;
        self.rebuild();
        Ok(self)
    }

    pub(super) fn resolve(mut self, module_uri: Option<&PklString>) -> Result<Self, RuntimeError> {
        if self.scheme.is_some() {
            return Ok(self);
        }
        if self.path == "..." || self.path.starts_with(".../") || self.path.starts_with("@") {
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
            if !self.path.starts_with("/") {
                let source = SourceText::from(base.path);
                let view = source.entire();
                let directory = view
                    .rfind("/")
                    .map_or(SourceSlice::from_utf8(""), |slash| view.range(..slash + 1));
                self.path = directory.to_pkl_string().concat(&self.path);
            }
        }
        self.scheme = base.scheme;
        self.path = normalize_path(&self.path);
        self.rebuild();
        Ok(self)
    }

    pub(super) fn normalize(mut self) -> Self {
        if self.opaque {
            return self;
        }
        let path = normalize_path(&self.path);
        if path != self.path {
            self.path = path;
            if self.authority.as_ref().is_some_and(PklString::is_empty) {
                self.authority = None;
            }
            self.rebuild();
        }
        self
    }

    pub(super) fn rebuild(&mut self) {
        let mut text = PklStringBuilder::default();
        if let Some(scheme) = &self.scheme {
            text.push_str(scheme);
            text.push(':');
        }
        if let Some(authority) = &self.authority {
            text.push_str("//");
            text.push_value(authority);
        }
        text.push_value(&self.path);
        if let Some(query) = &self.query {
            text.push('?');
            text.push_value(query);
        }
        if let Some(fragment) = &self.fragment {
            text.push('#');
            text.push_value(fragment);
        }
        self.text = text.finish();
    }
}
