//! Component ordering for static import graph keys, using the shared URI parser.
//! This is distinct from the representation order of the module cache key.
use super::*;
use std::cmp::Ordering;

/// Keep parser capability gaps distinct from Java URI syntax failures.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum UriFailure {
    Relative,
    Syntax {
        reason: PklString,
        utf16_index: Option<usize>,
    },
    Unsupported(PklString),
}

#[derive(Clone, Debug)]
pub(crate) struct Uri {
    pub(crate) text: PklString,
    scheme: String,
    opaque: bool,
    authority: Option<PklString>,
    server: Option<AuthorityKey>,
    path: PklString,
    query: Option<PklString>,
    fragment: Option<PklString>,
}
impl Uri {
    /// ModuleResolver normalizes only the URI passed to key factories. Graph
    /// keys retain the incoming URI spelling and component identity separately.
    pub(crate) fn factory_uri(&self) -> PklString {
        ResourceUri::parse(&self.text)
            .expect("validated analysis URI")
            .normalize()
            .text
    }

    pub(crate) fn parse(text: PklString) -> Result<Self, UriFailure> {
        let parsed = ResourceUri::parse(&text).map_err(|error| match error {
            ParseError::Syntax(reason) => UriFailure::Syntax {
                reason: reason.into(),
                utf16_index: lossless_uri::analysis_syntax_index(&text, reason),
            },
            ParseError::Unsupported(reason) => UriFailure::Unsupported(reason.into()),
        })?;
        let scheme = parsed.scheme.ok_or(UriFailure::Relative)?;
        if parsed
            .authority
            .as_ref()
            .is_some_and(|a| a.contains("[") && a.contains("%"))
        {
            return Err(UriFailure::Unsupported(
                "Scoped IPv6 URI comparison is not qualified.".into(),
            ));
        }
        let authority = parsed.authority.filter(|a| !a.is_empty());
        let server = authority.as_ref().and_then(server_authority);
        if matches!(
            server,
            Some(AuthorityKey::Server {
                port: Some(i32::MAX),
                ..
            })
        ) {
            return Err(UriFailure::Unsupported("Maximum Java URI port has non-total wrapped comparison; graph ordering is not qualified.".into()));
        }
        Ok(Self {
            text,
            scheme: scheme.to_ascii_lowercase(),
            opaque: parsed.opaque,
            authority: authority.as_ref().map(fold_escapes),
            server,
            path: fold_escapes(&parsed.path),
            query: parsed.query.as_ref().map(fold_escapes),
            fragment: parsed.fragment.as_ref().map(fold_escapes),
        })
    }
}
impl Ord for Uri {
    fn cmp(&self, other: &Self) -> Ordering {
        self.scheme
            .cmp(&other.scheme)
            .then_with(|| self.opaque.cmp(&other.opaque))
            .then_with(|| {
                if self.opaque {
                    self.path.cmp(&other.path)
                } else {
                    let authority = match (&self.server, &other.server) {
                        (Some(left), Some(right)) => left.cmp(right),
                        _ => self.authority.cmp(&other.authority),
                    };
                    authority
                        .then_with(|| self.path.cmp(&other.path))
                        .then_with(|| self.query.cmp(&other.query))
                }
            })
            .then_with(|| self.fragment.cmp(&other.fragment))
    }
}
impl PartialOrd for Uri {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl PartialEq for Uri {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other).is_eq()
    }
}
impl Eq for Uri {}
