//! Immutable source sections for instantiated type graphs and diagnostics.
use crate::source::{SourceMap, SourceText, Span};
use crate::value::RuntimeSource;
use std::sync::Arc;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TypeSourceOwner {
    /// The primary source has no external frame URI at the compile-only API.
    /// It still owns its complete text; it never inherits an alias source.
    Primary(SourceText),
    External(Arc<RuntimeSource>),
}

#[derive(Clone, Debug)]
pub struct TypeSourceSection(Arc<Section>);
#[derive(Debug, PartialEq, Eq)]
struct Section {
    owner: TypeSourceOwner,
    span: Span,
}
impl PartialEq for TypeSourceSection {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}
impl Eq for TypeSourceSection {}
impl TypeSourceSection {
    /// Whether two sections are the same source-node identity. Java union
    /// mismatch grouping uses identity rather than source-value equality.
    pub(crate) fn same_identity(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }

    pub fn new(owner: TypeSourceOwner, span: Span) -> Self {
        Self(Arc::new(Section { owner, span }))
    }
    pub fn span(&self) -> Span {
        self.0.span
    }
    pub fn owner(&self) -> &TypeSourceOwner {
        &self.0.owner
    }
    pub fn text(&self) -> &SourceText {
        match self.owner() {
            TypeSourceOwner::Primary(text) => text,
            TypeSourceOwner::External(source) => &source.text,
        }
    }
    pub fn external_source(&self) -> Option<Arc<RuntimeSource>> {
        match self.owner() {
            TypeSourceOwner::Primary(_) => None,
            TypeSourceOwner::External(source) => Some(source.clone()),
        }
    }
    pub fn with_source<T>(&self, use_source: impl FnOnce(&crate::source::Source) -> T) -> T {
        let mut sources = SourceMap::default();
        let id = sources.add("", self.text());
        use_source(sources.get(id).expect("type source inserted"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diagnostic::TypeDiagnostic;

    #[test]
    fn diagnostic_source_equality_is_distinct_from_node_identity() {
        let make = |uri: &str, start| {
            TypeSourceSection::new(
                TypeSourceOwner::External(Arc::new(RuntimeSource {
                    text: "x: Int".into(),
                    uri: uri.into(),
                    module_name: "module".into(),
                })),
                Span::new(start, 6),
            )
        };
        let first = make("file:///a.pkl", 3);
        let same_value = make("file:///a.pkl", 3);
        let cloned = first.clone();
        assert!(first.same_identity(&cloned));
        assert!(!first.same_identity(&same_value));
        let error = |section| TypeDiagnostic::AtSource {
            section,
            diagnostic: Box::new(TypeDiagnostic::Simple {
                span: Span::new(3, 6),
                message: "mismatch".into(),
            }),
        };
        assert_eq!(error(first.clone()), error(same_value));
        assert_ne!(error(first.clone()), error(make("file:///b.pkl", 3)));
        assert_ne!(error(first), error(make("file:///a.pkl", 4)));
    }
}
