//! Language Regex carrier and immutable RegexMatch factory data.
//! Pkl and the M1 adapters use the same checked raw UTF-16 backend. Pkl's Uu
//! flags and M1's historical u flags remain separate protocol inputs.
use crate::pkl_string::PklString;
use crate::value_identity::Token;
use java_regex::utf16::{
    CompileErrorKind, Limits, MatchError, MatchRanges, OwnedMatcher, Utf16Regex,
};
use std::ops::Range;
use std::sync::Arc;

#[derive(Clone, Debug)]
pub(crate) enum RegexError {
    Syntax(PklString),
    Unsupported(PklString),
    Execution(MatchError),
    InvalidBackendRange,
}
#[derive(Clone, Copy, Debug)]
pub(crate) enum StringPredicate {
    Contains,
    Matches,
    StartsWith,
    EndsWith,
}
#[derive(Debug)]
struct Compiled {
    pattern: PklString,
    regex: Arc<Utf16Regex>,
    reference: Token,
}
#[derive(Clone, Debug)]
pub struct PklRegex(Arc<Compiled>);
impl PklRegex {
    pub(crate) fn compile(pattern: PklString) -> Result<Self, RegexError> {
        let units = pattern.encode_utf16().collect::<Vec<_>>();
        let regex =
            crate::regex::compile_utf16(&units, "Uu").map_err(|error| match error.kind {
                CompileErrorKind::Syntax => RegexError::Syntax(error.message),
                CompileErrorKind::Unsupported | CompileErrorKind::NestingBudgetExceeded => {
                    RegexError::Unsupported(error.message)
                }
            })?;
        Ok(Self(Arc::new(Compiled {
            pattern,
            regex: Arc::new(regex),
            reference: Token::fresh(),
        })))
    }
    pub(crate) fn is_valid_pattern(pattern: &PklString) -> Result<bool, RegexError> {
        let units = pattern.encode_utf16().collect::<Vec<_>>();
        match crate::regex::compile_utf16(&units, "u") {
            Ok(_) => Ok(true),
            Err(error) => match error.kind {
                CompileErrorKind::Syntax => Ok(false),
                CompileErrorKind::Unsupported | CompileErrorKind::NestingBudgetExceeded => {
                    Err(RegexError::Unsupported(error.message))
                }
            },
        }
    }
    pub(crate) fn literal(pattern: PklString, unicode_case: bool) -> Self {
        let units = pattern.encode_utf16().collect::<Vec<_>>();
        let regex = Arc::new(Utf16Regex::literal(&units, unicode_case));
        Self(Arc::new(Compiled {
            pattern,
            regex,
            reference: Token::fresh(),
        }))
    }
    pub(crate) fn cursor(&self, input: &PklString) -> RegexCursor {
        let text = MatchText::new(input);
        RegexCursor {
            matcher: OwnedMatcher::new(self.0.regex.clone(), text.units.clone(), Limits::default()),
            text,
            pattern: self.pattern().clone(),
        }
    }
    pub(crate) fn pattern(&self) -> &PklString {
        &self.0.pattern
    }
    pub(crate) fn group_count(&self) -> usize {
        self.0.regex.group_count()
    }
    pub(crate) fn reference(&self) -> Token {
        self.0.reference
    }
    pub(crate) fn java_hash(&self) -> i32 {
        self.pattern()
            .encode_utf16()
            .fold(0i32, |h, u| h.wrapping_mul(31).wrapping_add(i32::from(u)))
    }
    pub(crate) fn string_predicate(
        &self,
        input: &PklString,
        operation: StringPredicate,
    ) -> Result<bool, RegexError> {
        let units = input.encode_utf16().collect::<Vec<_>>();
        let limits = Limits::default();
        match operation {
            StringPredicate::Contains => self
                .0
                .regex
                .find(&units, limits)
                .map(|m| m.is_some())
                .map_err(RegexError::Execution),
            StringPredicate::Matches => self
                .0
                .regex
                .captures_entire(&units, limits)
                .map(|m| m.is_some())
                .map_err(RegexError::Execution),
            StringPredicate::StartsWith => self
                .0
                .regex
                .looking_at(&units, limits)
                .map(|m| m.is_some())
                .map_err(RegexError::Execution),
            StringPredicate::EndsWith => {
                let mut matcher = self.0.regex.matcher(&units, limits);
                let mut end = None;
                while let Some(m) = matcher.find().map_err(RegexError::Execution)? {
                    end = Some(m.end);
                }
                Ok(end == Some(units.len()))
            }
        }
    }
    /// String indices count code points in the raw prefix; RegexMatch offsets
    /// themselves remain UTF-16 units, including a position inside a pair.
    pub(crate) fn find_index(
        &self,
        input: &PklString,
        last: bool,
    ) -> Result<Option<usize>, RegexError> {
        let units = input.encode_utf16().collect::<Vec<_>>();
        let mut matcher = self.0.regex.matcher(&units, Limits::default());
        let mut start = None;
        while let Some(m) = matcher.find().map_err(RegexError::Execution)? {
            if m.start > units.len() {
                return Err(RegexError::InvalidBackendRange);
            }
            start = Some(m.start);
            if !last {
                break;
            }
        }
        Ok(start.map(|p| PklString::from_utf16(units[..p].to_vec()).code_point_len()))
    }
    pub(crate) fn find_matches_in(
        &self,
        input: &PklString,
    ) -> Result<Vec<RegexMatchData>, RegexError> {
        let text = MatchText::new(input);
        let mut matcher = self.0.regex.matcher(&text.units, Limits::default());
        let mut results = Vec::new();
        while let Some(m) = matcher.find().map_err(RegexError::Execution)? {
            results.push(text.snapshot(m, self.group_count())?);
        }
        Ok(results)
    }
    pub(crate) fn match_entire(
        &self,
        input: &PklString,
    ) -> Result<Option<RegexMatchData>, RegexError> {
        let text = MatchText::new(input);
        self.0
            .regex
            .captures_entire(&text.units, Limits::default())
            .map_err(RegexError::Execution)?
            .map(|m| text.snapshot(m, self.group_count()))
            .transpose()
    }
}
/// Exact generated BaseNodesFactory.RegexNodeGen cache transition. Compilation
/// failure below the three-pattern limit does not publish an entry; a miss at
/// the limit disables caching before attempting the generic compilation.
#[derive(Clone, Default)]
pub(crate) struct ConstructorCache {
    patterns: Vec<PklRegex>,
    generic: bool,
}
impl ConstructorCache {
    pub(crate) fn compile(&mut self, pattern: PklString) -> Result<PklRegex, RegexError> {
        if !self.generic {
            if let Some(cached) = self
                .patterns
                .iter()
                .find(|cached| cached.pattern() == &pattern)
            {
                return Ok(cached.clone());
            }
            if self.patterns.len() < 3 {
                let compiled = PklRegex::compile(pattern)?;
                self.patterns.push(compiled.clone());
                return Ok(compiled);
            }
            self.patterns.clear();
            self.generic = true;
        }
        PklRegex::compile(pattern)
    }
}

impl PartialEq for PklRegex {
    fn eq(&self, other: &Self) -> bool {
        self.pattern() == other.pattern()
    }
}
impl Eq for PklRegex {}

struct MatchText {
    units: Arc<[u16]>,
}
impl MatchText {
    fn new(text: &PklString) -> Self {
        Self {
            units: text.encode_utf16().collect(),
        }
    }
    fn range(&self, start: usize, end: usize) -> Result<Range<usize>, RegexError> {
        if start <= end && end <= self.units.len() {
            Ok(start..end)
        } else {
            Err(RegexError::InvalidBackendRange)
        }
    }
    fn snapshot(
        &self,
        matched: MatchRanges,
        group_count: usize,
    ) -> Result<RegexMatchData, RegexError> {
        if matched.groups.len() != group_count + 1
            || matched.groups.first() != Some(&Some((matched.start, matched.end)))
        {
            return Err(RegexError::InvalidBackendRange);
        }
        let groups = matched
            .groups
            .into_iter()
            .map(|g| g.map(|(a, b)| self.range(a, b)).transpose())
            .collect::<Result<Vec<_>, _>>()?;
        Ok(RegexMatchData {
            result: Arc::new(MatchResult {
                units: self.units.clone(),
                groups: groups.into(),
            }),
            group: None,
        })
    }
}
#[derive(Debug)]
struct MatchResult {
    units: Arc<[u16]>,
    /// Index zero is the whole match; None means a non-participating group.
    groups: Arc<[Option<Range<usize>>]>,
}

/// Immutable, heap-edge-free extra storage for an ordinary typed RegexMatch.
/// Factory properties must read their declaration owner's storage so inherited
/// properties on an amendment retain the original result.
#[derive(Clone, Debug)]
pub struct RegexMatchData {
    result: Arc<MatchResult>,
    /// None is a main match; Some(0) is its distinct group-zero match.
    group: Option<usize>,
}

impl RegexMatchData {
    fn range(&self) -> &Range<usize> {
        self.result.groups[self.group.unwrap_or(0)]
            .as_ref()
            .expect("factory creates only participating matches")
    }

    pub(crate) fn value(&self) -> PklString {
        PklString::from_utf16(self.result.units[self.range().clone()].to_vec())
    }

    pub(crate) fn start(&self) -> usize {
        self.range().start
    }

    pub(crate) fn end(&self) -> usize {
        self.range().end
    }

    /// The VM lazily publishes these in its usual cached strict List factory.
    /// Group matches have no children; unmatched groups become language null.
    pub(crate) fn groups(&self) -> impl Iterator<Item = Option<Self>> + '_ {
        let count = if self.group.is_none() {
            self.result.groups.len()
        } else {
            0
        };
        (0..count).map(|index| {
            self.result.groups[index].as_ref().map(|_| Self {
                result: self.result.clone(),
                group: Some(index),
            })
        })
    }
}

/// Safe owned match state across an arbitrary Host-suspended mapper call.
/// Capture payloads are immutable UTF-16 data; VM values are rooted by Run.
pub(crate) struct RegexCursor {
    matcher: OwnedMatcher,
    text: MatchText,
    pattern: PklString,
}
impl RegexCursor {
    pub(crate) fn pattern(&self) -> &PklString {
        &self.pattern
    }
    pub(crate) fn input(&self) -> &[u16] {
        &self.text.units
    }
    pub(crate) fn find(&mut self) -> Result<Option<RegexMatchData>, RegexError> {
        self.matcher
            .find()
            .map_err(RegexError::Execution)?
            .map(|m| self.text.snapshot(m, self.matcher.regex().group_count()))
            .transpose()
    }
    /// StringNodes.findLast re-evaluates the last region with opaque bounds.
    /// It is not equivalent to returning the last previously observed captures.
    pub(crate) fn last_region(&mut self) -> Result<Option<RegexMatchData>, RegexError> {
        let mut last = None;
        while let Some(m) = self.matcher.find().map_err(RegexError::Execution)? {
            last = Some((m.start, m.end));
        }
        let Some((start, end)) = last else {
            return Ok(None);
        };
        self.matcher
            .looking_at_region(start, end)
            .map_err(RegexError::Execution)?
            .map(|m| self.text.snapshot(m, self.matcher.regex().group_count()))
            .transpose()
    }
    pub(crate) fn expand(
        &self,
        matched: &RegexMatchData,
        replacement: &PklString,
    ) -> Result<Vec<u16>, String> {
        let ranges = MatchRanges {
            start: matched.start(),
            end: matched.end(),
            groups: matched
                .result
                .groups
                .iter()
                .map(|g| g.as_ref().map(|r| (r.start, r.end)))
                .collect(),
        };
        crate::regex::expand_replacement(
            &replacement.encode_utf16().collect::<Vec<_>>(),
            &ranges,
            self.matcher.regex(),
            &self.text.units,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constructor_cache_publication_and_generic_transition_match_generated_node() {
        let mut cache = ConstructorCache::default();
        let a = cache.compile("a".into()).unwrap();
        assert!(cache.compile("[".into()).is_err());
        assert_eq!(
            a.reference(),
            cache.compile("a".into()).unwrap().reference()
        );
        cache.compile("b".into()).unwrap();
        cache.compile("c".into()).unwrap();
        assert_eq!(
            a.reference(),
            cache.compile("a".into()).unwrap().reference()
        );
        // A fourth distinct pattern disables the cache before compilation.
        assert!(cache.compile("[".into()).is_err());
        let a2 = cache.compile("a".into()).unwrap();
        assert_ne!(a.reference(), a2.reference());
        assert_ne!(
            a2.reference(),
            cache.compile("a".into()).unwrap().reference()
        );
        assert_eq!(a, a2);
        assert_eq!(a.java_hash(), a2.java_hash());
    }
}
