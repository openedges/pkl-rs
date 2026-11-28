use std::sync::Arc;

#[derive(Clone, Copy, Debug, Default)]
pub struct Flags {
    pub case_insensitive: bool, // i / CASE_INSENSITIVE
    pub multiline: bool,        // m / MULTILINE
    pub dotall: bool,           // s / DOTALL
    pub comments: bool,         // x / COMMENTS
    pub unicode_class: bool,    // U / UNICODE_CHARACTER_CLASS
    pub unix_lines: bool,       // d / UNIX_LINES
    pub unicode_case: bool,     // u / UNICODE_CASE
    pub canon_eq: bool,         // c / CANON_EQ
    pub literal: bool,          // LITERAL (no inline flag)
}

/// Precomputed AST metadata for a `Pattern` so matching/backtracking never
/// re-walks nested AST subtrees on every quantifier iteration or lookbehind check.
#[derive(Clone, Copy, Debug)]
pub struct PatternMeta {
    pub deterministic: bool,
    pub min_length: usize,
    pub max_length: Option<usize>,
    pub java_max: i32,
}

/// A pattern is an alternation of branches.
#[derive(Clone, Debug)]
pub struct Pattern {
    pub branches: Arc<[Vec<Node>]>,
    pub meta: PatternMeta,
}

impl Pattern {
    pub fn new(branches: Arc<[Vec<Node>]>) -> Self {
        let meta = compute_pattern_meta(&branches);
        Self { branches, meta }
    }
}

const JAVA_MAX_REPS: i32 = 0x7FFFFFFF;

fn compute_pattern_meta(branches: &[Vec<Node>]) -> PatternMeta {
    let deterministic = branches.len() == 1 && branches[0].iter().all(node_is_deterministic);
    let min_length = branches
        .iter()
        .map(|b| {
            b.iter()
                .fold(0usize, |acc, n| acc.saturating_add(node_min_length(n)))
        })
        .min()
        .unwrap_or(0);
    let mut max_len: Option<usize> = Some(0);
    for branch in branches {
        let mut total: Option<usize> = Some(0);
        for node in branch {
            total = total.and_then(|t| node_max_length(node).and_then(|n| t.checked_add(n)));
            if total.is_none() {
                break;
            }
        }
        match (max_len, total) {
            (Some(cur), Some(t)) => max_len = Some(cur.max(t)),
            _ => {
                max_len = None;
                break;
            }
        }
    }
    let mut jmax: Option<i32> = None;
    for branch in branches {
        let mut total: i32 = 0;
        for node in branch {
            total = total.wrapping_add(node_java_max(node));
        }
        jmax = Some(jmax.map_or(total, |m| m.max(total)));
    }
    PatternMeta {
        deterministic,
        min_length,
        max_length: max_len,
        java_max: jmax.unwrap_or(0),
    }
}

fn node_is_deterministic(n: &Node) -> bool {
    match n {
        Node::Group { inner, .. } | Node::FlagGroup { inner, .. } | Node::AtomicGroup { inner } => {
            inner.meta.deterministic
        }
        Node::Lookahead { .. } | Node::Lookbehind { .. } => true,
        Node::Quantified {
            inner, min, max, ..
        } => min == max && node_is_deterministic(inner),
        Node::NfcCharClass(_) => false,
        _ => true,
    }
}

fn node_min_length(n: &Node) -> usize {
    match n {
        Node::Literal(_)
        | Node::Dot
        | Node::CharClass(_)
        | Node::NfcCharClass(_)
        | Node::LinebreakMatcher => 1,
        Node::Group { inner, .. } | Node::FlagGroup { inner, .. } | Node::AtomicGroup { inner } => {
            inner.meta.min_length
        }
        Node::Quantified { inner, min, .. } => node_min_length(inner).saturating_mul(*min as usize),
        _ => 0,
    }
}

fn node_max_length(n: &Node) -> Option<usize> {
    match n {
        Node::Literal(_) | Node::Dot | Node::CharClass(_) | Node::NfcCharClass(_) => Some(1),
        Node::LinebreakMatcher => Some(2),
        Node::Anchor(_)
        | Node::GraphemeBoundary
        | Node::SetFlags(_)
        | Node::Lookahead { .. }
        | Node::Lookbehind { .. } => Some(0),
        Node::Group { inner, .. } | Node::FlagGroup { inner, .. } | Node::AtomicGroup { inner } => {
            inner.meta.max_length
        }
        Node::Quantified { inner, max, .. } => {
            if *max == 0 {
                return Some(0);
            }
            let inner_max = node_max_length(inner)?;
            if inner_max == 0 {
                Some(0)
            } else if *max == u32::MAX {
                None
            } else {
                inner_max.checked_mul(*max as usize)
            }
        }
        Node::Backreference(_) | Node::NamedBackreference(_) | Node::GraphemeCluster => None,
        Node::PositionCheck(_) => Some(0),
    }
}

fn node_java_max(n: &Node) -> i32 {
    match n {
        Node::Literal(_) | Node::Dot | Node::CharClass(_) => 1,
        Node::LinebreakMatcher => 2,
        Node::Group { inner, .. } | Node::FlagGroup { inner, .. } | Node::AtomicGroup { inner } => {
            inner.meta.java_max
        }
        Node::Quantified { inner, max, .. } => {
            if *max == 0 {
                return 0;
            }
            let inner_max = node_java_max(inner);
            if *max == 1 {
                inner_max.max(0)
            } else if *max == u32::MAX {
                inner_max.wrapping_mul(JAVA_MAX_REPS)
            } else {
                inner_max.wrapping_mul(*max as i32)
            }
        }
        _ => 0,
    }
}

#[derive(Clone, Debug)]
pub enum Node {
    Literal(u32),
    Dot,
    Anchor(AnchorKind),
    CharClass(Arc<CharClass>),
    /// OpenJDK `Pattern.NFCCharProperty`: matches a single code point or an
    /// NFC-composed grapheme prefix when `CANON_EQ` (`(?c)`) is active.
    NfcCharClass(Arc<CharClass>),
    Quantified {
        inner: Box<Node>,
        min: u32,
        max: u32,
        kind: QuantKind,
    },
    Group {
        index: Option<usize>,
        #[allow(dead_code)]
        name: Option<String>,
        inner: Pattern,
    },
    Lookahead {
        positive: bool,
        inner: Pattern,
    },
    Lookbehind {
        supplementary: bool,
        positive: bool,
        inner: Pattern,
    },
    AtomicGroup {
        inner: Pattern,
    },
    Backreference(usize),
    NamedBackreference(String),
    LinebreakMatcher, // \R
    GraphemeCluster,  // \X
    GraphemeBoundary, // \b{g}; syntax supported, matching remains explicit Unsupported
    SetFlags(Flags),  // inline flag change (?i) etc.
    FlagGroup {
        // (?i:...) scoped flag group
        flags: Flags,
        inner: Pattern,
    },

    // Engine-internal nodes (not produced by parser)
    #[allow(dead_code)]
    PositionCheck(usize), // engine-internal: assert current pos == target
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum AnchorKind {
    StartOfLine,                  // ^
    EndOfLine,                    // $
    StartOfInput,                 // \A
    EndOfInput,                   // \z
    EndOfInputBeforeFinalNewline, // \Z
    WordBoundary,                 // \b
    NonWordBoundary,              // \B
    PreviousMatchEnd,             // \G
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum QuantKind {
    Greedy,
    Reluctant,
    Possessive,
}

#[derive(Clone, Debug)]
pub struct CharClass {
    pub negated: bool,
    pub items: Vec<CharClassItem>,
}

#[derive(Debug)]
pub enum CharClassItem {
    Single(u32),
    Range(u32, u32),
    Predefined(PredefinedClass),
    UnicodeProperty {
        compiled: super::unicode::CompiledProperty,
        negated: bool,
        supplementary: bool,
    },
    Nested(CharClass),
    Intersection(Vec<CharClassItem>, Vec<CharClassItem>),
}

// Intersections can be arbitrarily deep without another '[' or group. Their
// lifetime and copies must not consume native stack proportional to AST depth.
impl Clone for CharClassItem {
    fn clone(&self) -> Self {
        enum Work<'a> {
            Item(&'a CharClassItem),
            Nested { negated: bool, count: usize },
            Intersection { left: usize, right: usize },
        }
        let mut work = vec![Work::Item(self)];
        let mut values: Vec<CharClassItem> = Vec::new();
        while let Some(next) = work.pop() {
            match next {
                Work::Item(item) => match item {
                    Self::Single(c) => values.push(Self::Single(*c)),
                    Self::Range(a, b) => values.push(Self::Range(*a, *b)),
                    Self::Predefined(class) => values.push(Self::Predefined(*class)),
                    Self::UnicodeProperty {
                        compiled,
                        negated,
                        supplementary,
                    } => values.push(Self::UnicodeProperty {
                        compiled: *compiled,
                        negated: *negated,
                        supplementary: *supplementary,
                    }),
                    Self::Nested(class) => {
                        work.push(Work::Nested {
                            negated: class.negated,
                            count: class.items.len(),
                        });
                        work.extend(class.items.iter().rev().map(Work::Item));
                    }
                    Self::Intersection(left, right) => {
                        work.push(Work::Intersection {
                            left: left.len(),
                            right: right.len(),
                        });
                        work.extend(right.iter().rev().map(Work::Item));
                        work.extend(left.iter().rev().map(Work::Item));
                    }
                },
                Work::Nested { negated, count } => {
                    let items = values.split_off(values.len() - count);
                    values.push(Self::Nested(CharClass { negated, items }));
                }
                Work::Intersection { left, right } => {
                    let right = values.split_off(values.len() - right);
                    let left = values.split_off(values.len() - left);
                    values.push(Self::Intersection(left, right));
                }
            }
        }
        values.pop().expect("one completed class item clone")
    }
}

impl CharClassItem {
    fn detach_children(&mut self, pending: &mut Vec<Self>) {
        match self {
            Self::Nested(class) => pending.extend(class.items.drain(..).rev()),
            Self::Intersection(left, right) => {
                // Reverse pushes preserve the original left-before-right,
                // first-item-before-later-item destruction traversal.
                pending.extend(right.drain(..).rev());
                pending.extend(left.drain(..).rev());
            }
            _ => {}
        }
    }
}
impl Drop for CharClassItem {
    fn drop(&mut self) {
        let mut pending = Vec::new();
        self.detach_children(&mut pending);
        while let Some(mut child) = pending.pop() {
            child.detach_children(&mut pending);
            // Its ordinary Drop now sees empty child lists; it cannot recurse
            // into the former AST, including during parser error cleanup.
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PredefinedClass {
    Digit,              // \d
    NonDigit,           // \D
    Word,               // \w
    NonWord,            // \W
    Whitespace,         // \s
    NonWhitespace,      // \S
    HorizWhitespace,    // \h
    NonHorizWhitespace, // \H
    VertWhitespace,     // \v
    NonVertWhitespace,  // \V
}
