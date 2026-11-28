//! Checked raw UTF-16 Java `java.util.regex.Pattern` matcher. Both pattern and
//! text remain lossless.
//!
//! Adapted from `java_regex` 0.1.0 (Copyright Mathieu Dutour Sikiric,
//! `MIT OR Apache-2.0`) and extended for raw UTF-16 code units, explicit
//! step/depth execution budgets, canonical equivalence (`CANON_EQ` / `(?c)`),
//! and pinned Unicode 15.0.0 tables generated from official Unicode Consortium
//! UCD 15.0.0 files (`Unicode-3.0`; see `UNICODE-15.0.0-PROVENANCE.md` and
//! `LICENSE-UNICODE`).
use std::collections::BTreeMap;
use std::fmt;
use std::sync::Arc;
mod canon_eq;
mod character_lookup;
mod character_names;
/// Character metadata captured from the pinned JDK 21 Unicode 15 provider.
pub use character_lookup::{CharacterRecord, character_record};
pub const UNICODE_VERSION: &str = "15.0.0";
mod engine;
mod input;
mod parser;
mod types;
mod unicode;
use engine::{Engine, State};
use input::Input;
use types::{AnchorKind, Flags, Node, Pattern};

/// Pattern errors retain the exact UTF-16 source. Consumers render this raw
/// payload, rather than relying on Display's human-readable escaped form.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CompileError {
    pub kind: CompileErrorKind,
    pub message: String,
    pub pattern: Vec<u16>,
    /// Parser code-point cursor, matching the index model used by Pattern.error.
    pub index: usize,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CompileErrorKind {
    Syntax,
    Unsupported,
    NestingBudgetExceeded,
}
impl CompileError {
    pub fn with_context(message: String, pattern: Vec<u16>, index: usize) -> Self {
        Self {
            kind: CompileErrorKind::Syntax,
            message,
            pattern,
            index,
        }
    }
}
impl fmt::Display for CompileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} near pattern index {}", self.message, self.index)
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Limits {
    pub steps: u64,
    pub depth: u32,
}
impl Default for Limits {
    fn default() -> Self {
        Self {
            steps: 5_000_000,
            depth: 500,
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MatchError {
    StepBudgetExceeded { limit: u64 },
    DepthBudgetExceeded { limit: u32 },
    Unsupported(&'static str),
    InvalidStart { start: usize, len: usize },
}
impl fmt::Display for MatchError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MatchRanges {
    pub start: usize,
    pub end: usize,
    /// Group zero followed by the declared capturing groups. None means the
    /// group did not participate; a zero-length range is an empty group.
    pub groups: Vec<Option<(usize, usize)>>,
}
#[derive(Clone, Debug)]
pub struct Utf16Regex {
    source: Vec<u16>,
    pattern: Pattern,
    flags: Flags,
    group_count: usize,
    named_groups: BTreeMap<String, usize>,
    start_s: bool,
}
impl Utf16Regex {
    pub fn new(pattern: &[u16]) -> Result<Self, CompileError> {
        Self::with_flags(pattern, "")
    }
    pub fn with_flags(source: &[u16], flags_str: &str) -> Result<Self, CompileError> {
        let mut flags = Flags::default();
        for flag in flags_str.chars() {
            match flag {
                'i' => flags.case_insensitive = true,
                'm' => flags.multiline = true,
                's' => flags.dotall = true,
                'x' => flags.comments = true,
                'U' => {
                    flags.unicode_class = true;
                    flags.unicode_case = true;
                }
                'd' => flags.unix_lines = true,
                'u' => flags.unicode_case = true,
                'c' => flags.canon_eq = true,
                _ => {
                    return Err(CompileError::with_context(
                        format!("Unsupported flag: {flag}"),
                        source.to_vec(),
                        0,
                    ));
                }
            }
        }
        let normalized = if flags.canon_eq && !flags.literal {
            Some(canon_eq::normalize_pattern(source)?)
        } else {
            None
        };
        let parse_source = normalized.as_deref().unwrap_or(source);
        let (pattern, group_count, named_groups, start_s) =
            parser::Parser::new(parse_source, flags).parse()?;
        Ok(Self {
            source: source.to_vec(),
            pattern,
            flags,
            group_count,
            named_groups,
            start_s,
        })
    }
    /// Convenience encoding adapter. It does not convert raw input back into a
    /// Rust String, so captures can still contain an isolated surrogate.
    pub fn from_str(pattern: &str, flags: &str) -> Result<Self, CompileError> {
        Self::with_flags(&pattern.encode_utf16().collect::<Vec<_>>(), flags)
    }
    /// Pattern.LITERAL without escaping/reparsing the pattern. Pkl's mapped
    /// String overload uses UNICODE_CASE; split's literal overload uses none.
    pub fn literal(source: &[u16], unicode_case: bool) -> Self {
        let input = Input(source);
        let mut pos = 0;
        let mut nodes = Vec::new();
        let mut start_s = false;
        while pos < source.len() {
            let cp = input.at(pos);
            start_s |= cp >= 0x10000 || (0xd800..=0xdfff).contains(&cp);
            nodes.push(Node::Literal(cp));
            pos = input.next(pos);
        }
        Self {
            source: source.to_vec(),
            pattern: Pattern::new(std::sync::Arc::from(vec![nodes])),
            flags: Flags {
                unicode_case,
                ..Flags::default()
            },
            group_count: 0,
            named_groups: BTreeMap::new(),
            start_s,
        }
    }

    pub fn source(&self) -> &[u16] {
        &self.source
    }
    pub fn group_count(&self) -> usize {
        self.group_count
    }
    pub fn named_groups(&self) -> &BTreeMap<String, usize> {
        &self.named_groups
    }
    pub fn matcher<'a>(&'a self, input: &'a [u16], limits: Limits) -> Matcher<'a> {
        let mut engine = Engine::new(input, self.flags, &self.named_groups);
        engine.set_limits(limits);
        Matcher {
            regex: self,
            engine,
            state: State::new(self.group_count),
            next: 0,
            previous_end: 0,
            done: false,
        }
    }
    pub fn find(&self, input: &[u16], limits: Limits) -> Result<Option<MatchRanges>, MatchError> {
        self.matcher(input, limits).find()
    }
    pub fn captures_entire(
        &self,
        input: &[u16],
        limits: Limits,
    ) -> Result<Option<MatchRanges>, MatchError> {
        self.matcher(input, limits).anchored(true)
    }
    pub fn looking_at(
        &self,
        input: &[u16],
        limits: Limits,
    ) -> Result<Option<MatchRanges>, MatchError> {
        self.matcher(input, limits).anchored(false)
    }
}
/// Stateful search with a budget across the entire matcher lifetime. It is
/// intentionally not Iterator<Item=MatchRanges>: failure cannot become None.
pub struct Matcher<'a> {
    regex: &'a Utf16Regex,
    engine: Engine<'a>,
    state: State,
    next: usize,
    previous_end: usize,
    done: bool,
}
impl Matcher<'_> {
    fn failure(&self) -> Result<(), MatchError> {
        match &self.engine.error {
            Some(e) => Err(e.clone()),
            None => Ok(()),
        }
    }
    fn result(&mut self, start: usize, end: usize) -> MatchRanges {
        self.state.captures[0] = Some((start, end));
        MatchRanges {
            start,
            end,
            groups: self.state.captures.clone(),
        }
    }
    /// Set the next search position exactly, including the middle of a pair.
    /// This does not refill a consumed budget or clear a terminal failure.
    pub fn find_from(&mut self, start: usize) -> Result<Option<MatchRanges>, MatchError> {
        if start > self.engine.input.0.len() {
            return Err(MatchError::InvalidStart {
                start,
                len: self.engine.input.0.len(),
            });
        }
        self.failure()?;
        self.next = start;
        self.previous_end = 0;
        self.done = false;
        self.state = State::new(self.regex.group_count);
        self.find()
    }
    pub fn find(&mut self) -> Result<Option<MatchRanges>, MatchError> {
        self.failure()?;
        if self.done {
            return Ok(None);
        }
        let len = self.engine.input.0.len();
        self.state = State::new(self.regex.group_count);
        while self.next <= len {
            self.engine.flags = self.regex.flags;
            self.engine.search_start = self.previous_end;
            let start = self.next;
            let found =
                self.engine
                    .try_match_at_persistent(&self.regex.pattern, start, &mut self.state);
            self.failure()?;
            if let Some(end) = found {
                let result = self.result(start, end);
                self.previous_end = end;
                // Matcher.find advances one *code unit* after a zero-width
                // match, regardless of the Start/StartS search-node choice.
                self.next = if end == start { end + 1 } else { end };
                return Ok(Some(result));
            }
            if self.next == len {
                break;
            }
            self.next = if self.regex.start_s {
                Input(self.engine.input.0).next(self.next)
            } else {
                self.next + 1
            };
        }
        self.done = true;
        Ok(None)
    }
    /// Pinned Matcher.region(start,end).lookingAt(), with opaque and anchoring
    /// bounds. The checked budget remains cumulative across this state change.
    pub fn looking_at_region(
        &mut self,
        start: usize,
        end: usize,
    ) -> Result<Option<MatchRanges>, MatchError> {
        self.failure()?;
        if start > end || end > self.engine.input.0.len() {
            return Err(MatchError::InvalidStart {
                start: end.max(start),
                len: self.engine.input.0.len(),
            });
        }
        self.engine.text_start = start;
        self.engine.text_end = end;
        self.engine.flags = self.regex.flags;
        self.engine.search_start = start;
        self.state = State::new(self.regex.group_count);
        let matched = self
            .engine
            .match_pattern(&self.regex.pattern, &[], start, &mut self.state);
        self.failure()?;
        Ok(if matched {
            Some(self.result(start, self.state.match_end))
        } else {
            None
        })
    }

    fn anchored(&mut self, entire: bool) -> Result<Option<MatchRanges>, MatchError> {
        self.failure()?;
        let tail = if entire {
            vec![Node::Anchor(AnchorKind::EndOfInput)]
        } else {
            Vec::new()
        };
        let matched = self
            .engine
            .match_pattern(&self.regex.pattern, &tail, 0, &mut self.state);
        self.failure()?;
        Ok(if matched {
            Some(self.result(0, self.state.match_end))
        } else {
            None
        })
    }
}

#[cfg(test)]
mod authority_tests;
#[cfg(test)]
mod class_tree_stack_tests;
#[cfg(test)]
mod parser_stack_tests;
#[cfg(test)]
mod syntax_tests;

/// Owned continuation for VM callbacks. No self-referential borrow or unsafe
/// code is needed: a find is synchronous, while its search/budget state survives
/// between calls. Every call delegates to the same checked Matcher algorithm.
pub struct OwnedMatcher {
    regex: Arc<Utf16Regex>,
    input: Arc<[u16]>,
    limits: Limits,
    progress: Option<Progress>,
}
struct Progress {
    next: usize,
    previous_end: usize,
    done: bool,
    state: State,
    steps: u64,
    error: Option<MatchError>,
    text_start: usize,
    text_end: usize,
}
impl OwnedMatcher {
    pub fn new(regex: Arc<Utf16Regex>, input: Arc<[u16]>, limits: Limits) -> Self {
        Self {
            regex,
            input,
            limits,
            progress: None,
        }
    }
    pub fn regex(&self) -> &Utf16Regex {
        &self.regex
    }
    pub fn input(&self) -> &[u16] {
        &self.input
    }
    fn advance(
        &mut self,
        region: Option<(usize, usize)>,
    ) -> Result<Option<MatchRanges>, MatchError> {
        let mut matcher = self.regex.matcher(&self.input, self.limits);
        if let Some(saved) = self.progress.take() {
            matcher.next = saved.next;
            matcher.previous_end = saved.previous_end;
            matcher.done = saved.done;
            matcher.state = saved.state;
            matcher.engine.restore_budget(saved.steps, saved.error);
            matcher.engine.text_start = saved.text_start;
            matcher.engine.text_end = saved.text_end;
        }
        let result = match region {
            Some((start, end)) => matcher.looking_at_region(start, end),
            None => matcher.find(),
        };
        let (steps, error) = matcher.engine.budget_state();
        self.progress = Some(Progress {
            next: matcher.next,
            previous_end: matcher.previous_end,
            done: matcher.done,
            state: matcher.state,
            steps,
            error,
            text_start: matcher.engine.text_start,
            text_end: matcher.engine.text_end,
        });
        result
    }
    pub fn find(&mut self) -> Result<Option<MatchRanges>, MatchError> {
        self.advance(None)
    }
    pub fn looking_at_region(
        &mut self,
        start: usize,
        end: usize,
    ) -> Result<Option<MatchRanges>, MatchError> {
        self.advance(Some((start, end)))
    }
}

#[cfg(test)]
mod round2_tests {
    use super::*;

    #[test]
    fn round2_regex_security_and_syntax_regressions() {
        // 1. Unclosed \x{...} and truncated \uXXXX must fail cleanly.
        assert!(Utf16Regex::from_str(r"\x{41", "").is_err());
        assert!(Utf16Regex::from_str(r"\u12", "").is_err());
        assert!(Utf16Regex::from_str(r"\u123", "").is_err());
        assert!(Utf16Regex::from_str(r"\x{41}", "").is_ok());

        // 2. Nested quantified groups must not cause exponential Arc/Vec cloning OOM.
        let pat = format!("{}a{}", "(".repeat(24), "+)".repeat(24));
        let re = Utf16Regex::from_str(&pat, "").expect("compile nested quantifiers");
        let input: Vec<u16> = format!("{}b", "a".repeat(30)).encode_utf16().collect();
        let mut m = re.matcher(&input, Limits::default());
        let _ = m.find();

        // 3. CANON_EQ with long combining-mark run finishes quickly without O(N^2) blowup
        //    (cleanly hitting NestingBudgetExceeded), and normal CANON_EQ still matches NFC/NFD.
        let mut marks = String::from("a");
        for _ in 0..400 {
            marks.push('\u{0301}');
        }
        assert!(Utf16Regex::from_str(&marks, "c").is_err());
        let canon = Utf16Regex::from_str("a\u{0301}", "c").expect("compile CANON_EQ");
        let input: Vec<u16> = "\u{00e1}".encode_utf16().collect();
        let mut m = canon.matcher(&input, Limits::default());
        assert!(m.find().expect("match CANON_EQ").is_some());
    }
}
