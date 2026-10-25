pub use crate::types::Flags;
use alloc::{boxed::Box, string::String, vec::Vec};
/// A pattern is an alternation of branches.
#[derive(Clone, Debug)]
pub struct Pattern {
    pub branches: Vec<Vec<Node>>,
}

#[derive(Clone, Debug)]
pub enum Node {
    Literal(u32),
    Dot,
    Anchor(AnchorKind),
    CharClass(CharClass),
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
    GroupEnd {
        index: usize,
        start: usize,
    },
    RestoreFlags(Flags), // engine-internal: restore flags after FlagGroup
    #[allow(dead_code)]
    PositionCheck(usize), // engine-internal: assert current pos == target
    GreedyCont {
        // engine-internal: continue greedy quantifier loop
        atom: Box<Node>,
        min: u32,
        max: u32,
        count: u32,
        rest: Vec<Node>,
        prev_pos: usize,
    },
    ReluctantCont {
        // engine-internal: continue reluctant quantifier loop
        atom: Box<Node>,
        min: u32,
        max: u32,
        count: u32,
        rest: Vec<Node>,
        prev_pos: usize,
    },
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
    UnicodeProperty { name: String, negated: bool },
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
        let mut work = alloc::vec![Work::Item(self)];
        let mut values: Vec<CharClassItem> = Vec::new();
        while let Some(next) = work.pop() {
            match next {
                Work::Item(item) => match item {
                    Self::Single(c) => values.push(Self::Single(*c)),
                    Self::Range(a, b) => values.push(Self::Range(*a, *b)),
                    Self::Predefined(class) => values.push(Self::Predefined(*class)),
                    Self::UnicodeProperty { name, negated } => values.push(Self::UnicodeProperty {
                        name: name.clone(),
                        negated: *negated,
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
