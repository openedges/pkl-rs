use std::collections::BTreeMap;

use super::types::*;
use super::unicode::CodePointExt;
use super::unicode::is_valid_unicode_property;
use super::{CompileError as PatternSyntaxError, input::Input};

/// Compute the maximum match length of a Pattern, or None if the engine cannot
/// prove a finite upper bound. Mirrors `TreeInfo.maxValid` / `maxLength`
/// propagation in OpenJDK `Pattern.java`. Used by lookbehind compile-time
/// validation: Java rejects lookbehind bodies for which this is None (the
/// "Look-behind group does not have an obvious maximum length" error).
fn pattern_max_length(pattern: &Pattern) -> Option<usize> {
    enum Work<'a> {
        Pattern(&'a Pattern),
        Branch(&'a [Node]),
        Node(&'a Node),
        Max(usize),
        Sum(usize),
        Repetition(u32),
    }
    let mut pending = vec![Work::Pattern(pattern)];
    let mut values: Vec<usize> = Vec::new();
    while let Some(work) = pending.pop() {
        match work {
            Work::Pattern(pattern) => {
                pending.push(Work::Max(pattern.branches.len()));
                pending.extend(pattern.branches.iter().rev().map(|b| Work::Branch(b)));
            }
            Work::Branch(branch) => {
                pending.push(Work::Sum(branch.len()));
                pending.extend(branch.iter().rev().map(Work::Node));
            }
            Work::Max(count) => {
                let start = values.len() - count;
                let max = values[start..].iter().copied().max().unwrap_or(0);
                values.truncate(start);
                values.push(max);
            }
            Work::Sum(count) => {
                let start = values.len() - count;
                let sum = values[start..]
                    .iter()
                    .try_fold(0usize, |a, &b| a.checked_add(b))?;
                values.truncate(start);
                values.push(sum);
            }
            Work::Repetition(max) => {
                let inner = values.pop().expect("quantified node length");
                // Retain the former TreeInfo approximation and overflow rule.
                let value = if max == u32::MAX {
                    if inner > 1 {
                        return None;
                    }
                    inner
                } else {
                    inner.checked_mul(max as usize)?
                };
                values.push(value);
            }
            Work::Node(node) => match node {
                Node::Literal(_) | Node::Dot | Node::CharClass(_) => values.push(1),
                Node::LinebreakMatcher => values.push(2),
                Node::Anchor(_)
                | Node::NfcCharClass(_)
                | Node::SetFlags(_)
                | Node::RestoreFlags(_)
                | Node::Lookahead { .. }
                | Node::Lookbehind { .. }
                | Node::GraphemeBoundary => values.push(0),
                Node::Group { inner, .. }
                | Node::FlagGroup { inner, .. }
                | Node::AtomicGroup { inner } => pending.push(Work::Pattern(inner)),
                Node::Quantified { inner, max, .. } => {
                    pending.push(Work::Repetition(*max));
                    pending.push(Work::Node(inner));
                }
                Node::Backreference(_) | Node::NamedBackreference(_) | Node::GraphemeCluster => {
                    return None;
                }
                node => unreachable!("parser length walk received engine-internal node {node:?}"),
            },
        }
    }
    values.pop()
}

fn is_lookbehind_bounded(p: &Pattern) -> bool {
    pattern_max_length(p).is_some()
}

enum GroupKind {
    Capture {
        index: Option<usize>,
        name: Option<String>,
    },
    Lookahead {
        positive: bool,
    },
    Lookbehind {
        supplementary: bool,
        positive: bool,
    },
    Atomic,
    Flags(Flags),
}

enum GroupOpening {
    Node(Node),
    Body(GroupKind, Flags),
}

struct PatternFrame {
    branches: Vec<Vec<Node>>,
    nodes: Vec<Node>,
    branch_start_flags: Flags,
    group: Option<(GroupKind, Flags)>,
}
impl PatternFrame {
    fn new(flags: Flags, group: Option<(GroupKind, Flags)>) -> Self {
        Self {
            branches: Vec::new(),
            nodes: Vec::new(),
            branch_start_flags: flags,
            group,
        }
    }
}

struct ClassBody {
    items: Vec<CharClassItem>,
    at_start: bool,
    consume_closing: bool,
}
impl ClassBody {
    fn new(consume_closing: bool) -> Self {
        Self {
            items: Vec::new(),
            at_start: true,
            consume_closing,
        }
    }
}

enum ClassTask {
    Bracket,
    FinishBracket { negated: bool },
    Body(ClassBody),
    Rhs(Vec<CharClassItem>),
    AppendBodyNested(ClassBody),
    AppendRhsNested(Vec<CharClassItem>),
    Intersect(ClassBody),
    ExtendRhs(Vec<CharClassItem>),
    RejectNestedRangeEnd,
}

enum ClassResult {
    Items(Vec<CharClassItem>),
    Class(CharClass),
}
impl ClassResult {
    fn items(self) -> Vec<CharClassItem> {
        match self {
            Self::Items(items) => items,
            Self::Class(_) => unreachable!("class grammar expected an item list"),
        }
    }
    fn class(self) -> CharClass {
        match self {
            Self::Class(class) => class,
            Self::Items(_) => unreachable!("class grammar expected a bracketed class"),
        }
    }
}

pub struct Parser {
    chars: Vec<u32>,
    source: Vec<u16>,
    start_s: bool,
    depth: usize,
    pos: usize,
    pub flags: Flags,
    pub group_count: usize,
    pub named_groups: BTreeMap<String, usize>,
    all_named_backrefs: Vec<String>,
}

impl Parser {
    pub fn new(pattern: &[u16], flags: Flags) -> Self {
        let input = Input(pattern);
        let mut chars = Vec::new();
        let mut pos = 0;
        while pos < pattern.len() {
            chars.push(input.at(pos));
            pos = input.next(pos);
        }
        let start_s = chars
            .iter()
            .any(|&c| c >= 0x10000 || (0xd800..=0xdfff).contains(&c));
        Self {
            start_s,
            depth: 0,
            chars,
            source: pattern.to_vec(),
            pos: 0,
            flags,
            group_count: 0,
            named_groups: BTreeMap::new(),
            all_named_backrefs: Vec::new(),
        }
    }

    pub fn parse(
        mut self,
    ) -> Result<(Pattern, usize, BTreeMap<String, usize>, bool), PatternSyntaxError> {
        let pattern = self.parse_pattern()?;
        if self.pos < self.chars.len() {
            return Err(self.error(format!(
                "Unexpected character '{}' at position {}",
                super::unicode::display_codepoint(self.chars[self.pos]),
                self.pos
            )));
        }
        for name in &self.all_named_backrefs {
            if !self.named_groups.contains_key(name) {
                return Err(self.error(format!("Unknown named group: {}", name)));
            }
        }
        let gc = self.group_count;
        let ng = self.named_groups;
        Ok((pattern, gc, ng, self.start_s))
    }

    /// Construct a PatternSyntaxError carrying the current pattern + cursor
    /// position. The Display impl formats this Java-style:
    ///     `<message> near index <N>\n<pattern>\n      ^`
    fn error(&self, message: String) -> PatternSyntaxError {
        PatternSyntaxError::with_context(message, self.source.clone(), self.pos)
    }

    fn error_at(&self, message: String, index: usize) -> PatternSyntaxError {
        PatternSyntaxError::with_context(message, self.source.clone(), index)
    }

    fn nesting_budget(&self, message: String) -> PatternSyntaxError {
        let mut error = self.error(message);
        error.kind = super::CompileErrorKind::NestingBudgetExceeded;
        error
    }

    fn peek(&self) -> Option<u32> {
        self.chars.get(self.pos).copied()
    }

    fn advance(&mut self) -> Option<u32> {
        let c = self.chars.get(self.pos).copied();
        if c.is_some() {
            self.pos += 1;
        }
        c
    }

    fn expect(&mut self, expected: u32) -> Result<(), PatternSyntaxError> {
        match self.advance() {
            Some(c) if c == expected => Ok(()),
            _ => Err(self.error(format!(
                "Expected '{}'",
                super::unicode::display_codepoint(expected)
            ))),
        }
    }

    fn parse_pattern(&mut self) -> Result<Pattern, PatternSyntaxError> {
        let previous_depth = self.depth;
        let result = self.parse_pattern_frames();
        self.depth = previous_depth;
        result
    }

    fn enter_pattern(&mut self) -> Result<(), PatternSyntaxError> {
        if self.depth >= 256 {
            return Err(self
                .nesting_budget("Pattern nesting exceeds backend compilation budget".to_string()));
        }
        self.depth += 1;
        Ok(())
    }

    /// Group continuations live on the heap. The same logical depth budget is
    /// checked after the opening prefix, before entering its body; it no longer
    /// requires one native call stack for every enclosing group.
    fn parse_pattern_frames(&mut self) -> Result<Pattern, PatternSyntaxError> {
        self.enter_pattern()?;
        let mut frames = vec![PatternFrame::new(self.flags, None)];
        loop {
            if self.flags.comments {
                self.skip_comments_whitespace();
            }
            match self.peek() {
                None | Some(0x29) => {
                    let mut frame = frames.pop().expect("active pattern frame");
                    self.finish_branch(&mut frame);
                    let inner = Pattern {
                        branches: frame.branches,
                    };
                    self.depth -= 1;
                    let Some((kind, saved_flags)) = frame.group else {
                        return Ok(inner);
                    };
                    // Restore before consuming ')' and parsing the quantifier,
                    // exactly as the former recursive group return did.
                    self.flags = saved_flags;
                    self.expect(0x29)?;
                    let node = self.finish_group(kind, inner)?;
                    let node = self.maybe_parse_quantifier(node)?;
                    frames
                        .last_mut()
                        .expect("enclosing pattern")
                        .nodes
                        .push(node);
                }
                Some(0x7c) => {
                    let frame = frames.last_mut().expect("active pattern frame");
                    self.finish_branch(frame);
                    self.advance();
                    frame.branch_start_flags = self.flags;
                }
                Some(0x28) => match self.parse_group_opening()? {
                    GroupOpening::Node(node) => {
                        let node = self.maybe_parse_quantifier(node)?;
                        frames
                            .last_mut()
                            .expect("active pattern frame")
                            .nodes
                            .push(node);
                    }
                    GroupOpening::Body(kind, saved_flags) => {
                        self.enter_pattern()?;
                        frames.push(PatternFrame::new(self.flags, Some((kind, saved_flags))));
                    }
                },
                _ => {
                    let frame = frames.last_mut().expect("active pattern frame");
                    // Quoted text has one quantifiable final atom, including
                    // the former empty-quote behavior and unterminated quotes.
                    if self.pos + 1 < self.chars.len()
                        && self.chars[self.pos] == 0x5c
                        && self.chars[self.pos + 1] == 0x51
                    {
                        self.pos += 2;
                        let mut quoted_chars = Vec::new();
                        while self.pos < self.chars.len() {
                            if self.pos + 1 < self.chars.len()
                                && self.chars[self.pos] == 0x5c
                                && self.chars[self.pos + 1] == 0x45
                            {
                                self.pos += 2;
                                break;
                            }
                            quoted_chars.push(self.chars[self.pos]);
                            self.pos += 1;
                        }
                        if let Some((&last, prefix)) = quoted_chars.split_last() {
                            frame
                                .nodes
                                .extend(prefix.iter().copied().map(Node::Literal));
                            frame
                                .nodes
                                .push(self.maybe_parse_quantifier(Node::Literal(last))?);
                        }
                    } else {
                        let node = self.parse_atom()?;
                        frame.nodes.push(self.maybe_parse_quantifier(node)?);
                    }
                }
            }
        }
    }

    fn finish_branch(&mut self, frame: &mut PatternFrame) {
        let mut nodes = core::mem::take(&mut frame.nodes);
        let mut atom_flags = frame.branch_start_flags;
        let mut first = 0;
        while first < nodes.len() {
            if matches!(nodes[first], Node::Literal(_)) {
                let mut end = first + 1;
                while end < nodes.len() && matches!(nodes[end], Node::Literal(_)) {
                    end += 1;
                }
                if end == first + 1 {
                    self.start_s |= node_starts_s(&nodes[first], atom_flags);
                }
                first = end;
            } else {
                self.start_s |= node_starts_s(&nodes[first], atom_flags);
                if let Node::SetFlags(flags) = &nodes[first] {
                    atom_flags = *flags;
                }
                first += 1;
            }
        }
        // Inline flags leak across later branches of this pattern, but the
        // runtime must start each branch in its parser-time flag state.
        nodes.insert(0, Node::SetFlags(frame.branch_start_flags));
        frame.branches.push(nodes);
    }

    fn finish_group(&self, kind: GroupKind, inner: Pattern) -> Result<Node, PatternSyntaxError> {
        Ok(match kind {
            GroupKind::Capture { index, name } => Node::Group { index, name, inner },
            GroupKind::Lookahead { positive } => Node::Lookahead { positive, inner },
            GroupKind::Lookbehind {
                supplementary,
                positive,
            } => {
                if !is_lookbehind_bounded(&inner) {
                    return Err(self.error(
                        "Look-behind group does not have an obvious maximum length".to_string(),
                    ));
                }
                Node::Lookbehind {
                    supplementary,
                    positive,
                    inner,
                }
            }
            GroupKind::Atomic => Node::AtomicGroup { inner },
            GroupKind::Flags(flags) => Node::FlagGroup { flags, inner },
        })
    }

    fn skip_comments_whitespace(&mut self) {
        loop {
            match self.peek() {
                Some(c) if c == 0x20 || c == 0x09 || c == 0x0a || c == 0x0d => {
                    self.advance();
                }
                Some(0x23) => {
                    self.advance();
                    while let Some(ch) = self.peek() {
                        if ch == 0x0a {
                            self.advance();
                            break;
                        }
                        self.advance();
                    }
                }
                _ => break,
            }
        }
    }

    fn parse_atom(&mut self) -> Result<Node, PatternSyntaxError> {
        let c = self
            .peek()
            .ok_or_else(|| self.error("Unexpected end of pattern".to_string()))?;

        match c {
            0x5c => self.parse_escape(),
            0x2e => {
                self.advance();
                Ok(Node::Dot)
            }
            0x5e => {
                self.advance();
                Ok(Node::Anchor(AnchorKind::StartOfLine))
            }
            0x24 => {
                self.advance();
                Ok(Node::Anchor(AnchorKind::EndOfLine))
            }
            0x5b => self.parse_char_class_node(),
            0x28 => unreachable!("group opening is handled by the pattern continuation"),
            0x2a | 0x2b | 0x3f => Err(self.error(format!(
                "Dangling meta character '{}'",
                super::unicode::display_codepoint(c)
            ))),
            0x7b => {
                let saved = self.pos;
                self.advance();
                match self.parse_quantifier_braces() {
                    Ok((min, max)) => {
                        let kind = match self.peek() {
                            Some(0x3f) => {
                                self.advance();
                                QuantKind::Reluctant
                            }
                            Some(0x2b) => {
                                self.advance();
                                QuantKind::Possessive
                            }
                            _ => QuantKind::Greedy,
                        };
                        let empty = Node::Group {
                            index: None,
                            name: None,
                            inner: Pattern {
                                branches: vec![vec![]],
                            },
                        };
                        Ok(Node::Quantified {
                            inner: Box::new(empty),
                            min,
                            max,
                            kind,
                        })
                    }
                    Err(_) => {
                        self.pos = saved;
                        Err(self.error(format!("Illegal repetition near index {}", self.pos)))
                    }
                }
            }
            _ => {
                self.advance();
                Ok(Node::Literal(c))
            }
        }
    }

    fn parse_escape(&mut self) -> Result<Node, PatternSyntaxError> {
        self.advance(); // consume '\'
        let c = self
            .advance()
            .ok_or_else(|| self.error("Unexpected end of pattern after \\".to_string()))?;

        match c {
            // Predefined character classes
            0x64 => Ok(self.predefined_node(PredefinedClass::Digit)),
            0x44 => Ok(self.predefined_node(PredefinedClass::NonDigit)),
            0x77 => Ok(self.predefined_node(PredefinedClass::Word)),
            0x57 => Ok(self.predefined_node(PredefinedClass::NonWord)),
            0x73 => Ok(self.predefined_node(PredefinedClass::Whitespace)),
            0x53 => Ok(self.predefined_node(PredefinedClass::NonWhitespace)),
            0x68 => Ok(self.predefined_node(PredefinedClass::HorizWhitespace)),
            0x48 => Ok(self.predefined_node(PredefinedClass::NonHorizWhitespace)),
            0x76 => Ok(self.predefined_node(PredefinedClass::VertWhitespace)),
            0x56 => Ok(self.predefined_node(PredefinedClass::NonVertWhitespace)),

            // Anchors
            0x41 => Ok(Node::Anchor(AnchorKind::StartOfInput)),
            0x7a => Ok(Node::Anchor(AnchorKind::EndOfInput)),
            0x5a => Ok(Node::Anchor(AnchorKind::EndOfInputBeforeFinalNewline)),
            0x62 => {
                if self.chars.get(self.pos..self.pos + 3) == Some(&[0x7b, 0x67, 0x7d]) {
                    self.pos += 3;
                    return Ok(Node::GraphemeBoundary);
                }
                if self.chars.get(self.pos..self.pos + 2) == Some(&[0x7b, 0x67]) {
                    return Err(
                        self.error_at("Illegal/unsupported escape sequence".into(), self.pos + 1)
                    );
                }
                Ok(Node::Anchor(AnchorKind::WordBoundary))
            }
            0x42 => Ok(Node::Anchor(AnchorKind::NonWordBoundary)),
            0x47 => Ok(Node::Anchor(AnchorKind::PreviousMatchEnd)),

            // Special escape sequences
            0x74 => Ok(Node::Literal(0x09)),
            0x6e => Ok(Node::Literal(0x0a)),
            0x72 => Ok(Node::Literal(0x0d)),
            0x66 => Ok(Node::Literal(0x0c)),
            0x61 => Ok(Node::Literal(0x07)),
            0x65 => Ok(Node::Literal(0x1b)),

            0x52 => Ok(Node::LinebreakMatcher),
            0x58 => Ok(Node::GraphemeCluster),
            // Note: top-level `\Q...\E` is pre-handled in parse_pattern_alt;
            // char-class `\Q...\E` is handled in parse_char_class_item. So
            // `\Q` never reaches parse_escape — no arm needed here.
            0x70 => self.parse_unicode_property_node(false),
            0x50 => self.parse_unicode_property_node(true),
            0x4e => self.parse_named_character().map(Node::Literal),
            0x78 => self.parse_hex_char().map(Node::Literal),
            0x75 => self.parse_unicode_char().map(Node::Literal),
            0x30 => self.parse_octal_char().map(Node::Literal),
            0x63 => self.parse_control_char().map(Node::Literal),

            // Backreference (numbered)
            0x31..=0x39 => {
                let mut num = (c - 0x30_u32) as usize;
                while let Some(d) = self.peek() {
                    if d.is_ascii_digit() {
                        let new_num = num * 10 + (d - 0x30_u32) as usize;
                        if new_num <= self.group_count {
                            num = new_num;
                            self.advance();
                        } else {
                            break;
                        }
                    } else {
                        break;
                    }
                }
                Ok(Node::Backreference(num))
            }

            // Named backreference
            0x6b => {
                self.expect(0x3c)?;
                let name = self.parse_group_name()?;
                self.expect(0x3e)?;
                self.all_named_backrefs.push(name.clone());
                Ok(Node::NamedBackreference(name))
            }

            // Note: there's no "bare" 3-digit octal (`\377`) arm here.
            // Java's spec requires the leading `0` (`\0nnn`). The `'1'..='9'`
            // arm above always catches the leading digit as a backreference,
            // matching OpenJDK behavior.

            // Escaped metacharacters
            0x5c | 0x2e | 0x2a | 0x2b | 0x3f | 0x28 | 0x29 | 0x5b | 0x5d | 0x7b | 0x7d | 0x7c
            | 0x5e | 0x24 | 0x2d | 0x21 | 0x3d | 0x3c | 0x3e | 0x2f | 0x23 | 0x20 | 0x26 | 0x7e
            | 0x40 | 0x60 | 0x27 | 0x22 | 0x2c | 0x3b | 0x3a => Ok(Node::Literal(c)),

            0x41..=0x5a | 0x61..=0x7a => {
                Err(self.error_at("Illegal/unsupported escape sequence".into(), self.pos - 1))
            }

            _ => Ok(Node::Literal(c)),
        }
    }

    // Shared escape-character parsers used by both parse_escape and parse_char_class_item

    fn parse_hex_char(&mut self) -> Result<u32, PatternSyntaxError> {
        if self.peek() == Some(0x7b) {
            self.advance();
            let mut hex = String::new();
            while let Some(c) = self.peek() {
                if c == 0x7d {
                    self.advance();
                    break;
                }
                hex.push(
                    core::char::from_u32(c)
                        .ok_or_else(|| self.error("Invalid syntax code point".to_string()))?,
                );
                self.advance();
            }
            let code = u32::from_str_radix(&hex, 16)
                .map_err(|_| self.error(format!("Invalid hex escape: {}", hex)))?;
            if code <= 0x10ffff {
                Ok(code)
            } else {
                Err(self.error("Hexadecimal code point exceeds Unicode range".to_string()))
            }
        } else {
            let mut hex = String::new();
            for _ in 0..2 {
                if let Some(c) = self.peek() {
                    if c.is_ascii_hexdigit() {
                        hex.push(
                            core::char::from_u32(c).ok_or_else(|| {
                                self.error("Invalid syntax code point".to_string())
                            })?,
                        );
                        self.advance();
                    } else {
                        break;
                    }
                }
            }
            if hex.len() != 2 {
                return Err(self.error("Invalid hex escape".to_string()));
            }
            let code = u32::from_str_radix(&hex, 16)
                .map_err(|_| self.error(format!("Invalid hex escape: {}", hex)))?;
            if code <= 0x10ffff {
                Ok(code)
            } else {
                Err(self.error("Hexadecimal code point exceeds Unicode range".to_string()))
            }
        }
    }

    fn parse_unicode_char(&mut self) -> Result<u32, PatternSyntaxError> {
        let code = self.parse_4hex()?;
        // Handle UTF-16 surrogate pairs: \uD800\uDC00 etc.
        if (0xD800..=0xDBFF).contains(&code) {
            // High surrogate — look for \uXXXX low surrogate
            let saved_pos = self.pos;
            if self.peek() == Some(0x5c) {
                self.advance();
                if self.peek() == Some(0x75) {
                    self.advance();
                    if let Ok(low) = self.parse_4hex()
                        && (0xDC00..=0xDFFF).contains(&low)
                    {
                        let cp = 0x10000 + ((code - 0xD800) << 10) + (low - 0xDC00);
                        return Ok(cp);
                    }
                }
            }
            self.pos = saved_pos;
        }
        if code <= 0x10ffff {
            Ok(code)
        } else {
            Err(self.error("Hexadecimal code point exceeds Unicode range".to_string()))
        }
    }

    fn parse_4hex(&mut self) -> Result<u32, PatternSyntaxError> {
        let mut hex = String::new();
        for _ in 0..4 {
            if let Some(c) = self.peek() {
                if c.is_ascii_hexdigit() {
                    hex.push(
                        core::char::from_u32(c)
                            .ok_or_else(|| self.error("Invalid syntax code point".to_string()))?,
                    );
                    self.advance();
                } else {
                    break;
                }
            }
        }
        u32::from_str_radix(&hex, 16)
            .map_err(|_| self.error(format!("Invalid unicode escape: {}", hex)))
    }

    fn parse_octal_char(&mut self) -> Result<u32, PatternSyntaxError> {
        // Java's octal: \0 already consumed, read up to 3 more digits.
        // If first digit is 0-3, read up to 2 more (total value ≤ 0377).
        // If first digit is 4-7, read only 1 more (total value ≤ 077).
        let first = match self.peek() {
            Some(c) if (0x30..=0x37).contains(&c) => {
                self.advance();
                c
            }
            _ => {
                return Err(self.error(format!(
                    "Illegal octal escape sequence near index {}",
                    self.pos
                )));
            }
        };
        let max_more = if (0x30..=0x33).contains(&first) { 2 } else { 1 };
        let mut oct = String::new();
        oct.push(
            core::char::from_u32(first)
                .ok_or_else(|| self.error("Invalid syntax code point".to_string()))?,
        );
        for _ in 0..max_more {
            if let Some(c) = self.peek() {
                if (0x30..=0x37).contains(&c) {
                    oct.push(
                        core::char::from_u32(c)
                            .ok_or_else(|| self.error("Invalid syntax code point".to_string()))?,
                    );
                    self.advance();
                } else {
                    break;
                }
            }
        }
        let code = u32::from_str_radix(&oct, 8).unwrap_or(0);
        if code <= 0x10ffff {
            Ok(code)
        } else {
            Err(self.error("Hexadecimal code point exceeds Unicode range".to_string()))
        }
    }

    fn parse_control_char(&mut self) -> Result<u32, PatternSyntaxError> {
        let ctrl = self
            .advance()
            .ok_or_else(|| self.error("Expected control character after \\c".to_string()))?;
        let code = ctrl ^ 0x40;
        if code <= 0x10ffff {
            Ok(code)
        } else {
            Err(self.error("Hexadecimal code point exceeds Unicode range".to_string()))
        }
    }

    fn predefined_node(&self, pc: PredefinedClass) -> Node {
        Node::CharClass(CharClass {
            negated: false,
            items: vec![CharClassItem::Predefined(pc)],
        })
    }

    fn parse_unicode_property_node(&mut self, negated: bool) -> Result<Node, PatternSyntaxError> {
        let (name, neg) = self.parse_property_name(negated)?;
        let cc = CharClass {
            negated: false,
            items: vec![CharClassItem::UnicodeProperty { name, negated: neg }],
        };
        if self.flags.canon_eq && !self.flags.literal {
            Ok(Node::NfcCharClass(cc))
        } else {
            Ok(Node::CharClass(cc))
        }
    }

    /// Parse a unicode property name (shared between node and char class contexts).
    /// Returns (name, negated).
    fn parse_property_name(&mut self, negated: bool) -> Result<(String, bool), PatternSyntaxError> {
        let mut name = String::new();
        let index;
        if self.peek() == Some(0x7b) {
            self.advance();
            let start = self.pos;
            while self.peek().is_some_and(|c| c != 0x7d) {
                let cp = self.advance().expect("checked current character");
                name.push_str(&super::unicode::display_codepoint(cp));
            }
            if self.peek().is_none() {
                return Err(self.error("Unclosed character family".into()));
            }
            index = self.pos;
            self.advance();
            if index == start {
                return Err(self.error_at("Empty character family".into(), index));
            }
        } else {
            let cp = self
                .advance()
                .ok_or_else(|| self.error("Unknown character property name".into()))?;
            name.push_str(&super::unicode::display_codepoint(cp));
            index = self.pos - 1;
        }
        if !is_valid_unicode_property(&name, self.flags.unicode_class) {
            let message = if let Some((key, value)) = name.split_once('=') {
                format!(
                    "Unknown Unicode property {{name=<{}>, value=<{}>}}",
                    key.to_ascii_lowercase(),
                    value
                )
            } else {
                format!("Unknown character property name {{{name}}}")
            };
            return Err(self.error_at(message, index));
        }
        Ok((name, negated))
    }

    fn parse_named_character(&mut self) -> Result<u32, PatternSyntaxError> {
        if self.advance() != Some(0x7b) {
            return Err(self.error_at(
                "Illegal character name escape sequence".into(),
                self.pos.saturating_sub(1),
            ));
        }
        let start = self.pos;
        while self.peek().is_some_and(|c| c != 0x7d) {
            self.advance();
        }
        if self.peek().is_none() {
            return Err(self.error_at(
                "Unclosed character name escape sequence".into(),
                self.pos.saturating_sub(1),
            ));
        }
        let end = self.pos;
        self.advance();
        super::character_names::code_point_of(&self.chars[start..end]).ok_or_else(|| {
            let name: String = self.chars[start..end]
                .iter()
                .map(|&cp| super::unicode::display_codepoint(cp))
                .collect();
            self.error_at(format!("Unknown character name [{name}]"), end)
        })
    }

    fn parse_group_name(&mut self) -> Result<String, PatternSyntaxError> {
        let mut name = String::new();
        while let Some(c) = self.peek() {
            if c.is_alphanumeric() || c == 0x5f {
                name.push(
                    core::char::from_u32(c)
                        .ok_or_else(|| self.error("Invalid syntax code point".to_string()))?,
                );
                self.advance();
            } else {
                break;
            }
        }
        if name.is_empty() {
            return Err(self.error("Empty group name".to_string()));
        }
        if name.starts_with(|c: char| c.is_ascii_digit()) {
            return Err(self.error(format!(
                "Group name must start with a letter, not '{}'",
                name.chars().next().unwrap()
            )));
        }
        Ok(name)
    }

    fn parse_group_opening(&mut self) -> Result<GroupOpening, PatternSyntaxError> {
        self.advance(); // consume '('
        let kind = if self.peek() == Some(0x3f) {
            self.advance();
            match self.peek() {
                Some(0x3a) => {
                    self.advance();
                    GroupKind::Capture {
                        index: None,
                        name: None,
                    }
                }
                Some(0x3c) => {
                    self.advance();
                    match self.peek() {
                        Some(c @ (0x3d | 0x21)) => {
                            self.advance();
                            let supplementary = self.chars[self.pos..]
                                .iter()
                                .any(|&c| c >= 0x10000 || (0xd800..=0xdfff).contains(&c));
                            GroupKind::Lookbehind {
                                supplementary,
                                positive: c == 0x3d,
                            }
                        }
                        _ => {
                            let name = self.parse_group_name()?;
                            self.expect(0x3e)?;
                            if self.named_groups.contains_key(&name) {
                                return Err(self.error(format!("Duplicate group name: {}", name)));
                            }
                            self.group_count += 1;
                            let index = self.group_count;
                            self.named_groups.insert(name.clone(), index);
                            GroupKind::Capture {
                                index: Some(index),
                                name: Some(name),
                            }
                        }
                    }
                }
                Some(c @ (0x3d | 0x21)) => {
                    self.advance();
                    GroupKind::Lookahead {
                        positive: c == 0x3d,
                    }
                }
                Some(0x3e) => {
                    self.advance();
                    GroupKind::Atomic
                }
                _ => return self.parse_inline_flags(),
            }
        } else {
            self.group_count += 1;
            GroupKind::Capture {
                index: Some(self.group_count),
                name: None,
            }
        };
        Ok(GroupOpening::Body(kind, self.flags))
    }

    fn parse_inline_flags(&mut self) -> Result<GroupOpening, PatternSyntaxError> {
        let mut set_flags = Flags::default();
        let mut clear_flags = Flags::default();
        let mut clearing = false;

        loop {
            match self.peek() {
                Some(ch @ (0x63 | 0x69 | 0x6d | 0x73 | 0x78 | 0x55 | 0x64 | 0x75)) => {
                    self.advance();
                    let target = if clearing {
                        &mut clear_flags
                    } else {
                        &mut set_flags
                    };
                    match ch {
                        0x63 => target.canon_eq = true,
                        0x69 => target.case_insensitive = true,
                        0x6d => target.multiline = true,
                        0x73 => target.dotall = true,
                        0x78 => target.comments = true,
                        0x55 => target.unicode_class = true,
                        0x64 => target.unix_lines = true,
                        0x75 => target.unicode_case = true,
                        // The outer arm binds `ch` from the exhaustive
                        // `'c' | 'i' | 'm' | 's' | 'x' | 'U' | 'd' | 'u'`
                        // pattern, so this catchall is structurally unreachable.
                        ch => unreachable!(
                            "inline-flag char {ch:?} not in the outer pattern \
                             'c' | 'i' | 'm' | 's' | 'x' | 'U' | 'd' | 'u'"
                        ),
                    }
                }
                Some(0x2d) => {
                    self.advance();
                    clearing = true;
                }
                Some(0x3a) => {
                    self.advance();
                    let saved = self.flags;
                    self.apply_flags(set_flags, clear_flags);
                    return Ok(GroupOpening::Body(GroupKind::Flags(self.flags), saved));
                }
                Some(0x29) => {
                    self.advance();
                    self.apply_flags(set_flags, clear_flags);
                    return Ok(GroupOpening::Node(Node::SetFlags(self.flags)));
                }
                _ => {
                    return Err(self.error("Invalid inline flag".to_string()));
                }
            }
        }
    }

    fn apply_flags(&mut self, set: Flags, clear: Flags) {
        if set.case_insensitive {
            self.flags.case_insensitive = true;
        }
        if set.multiline {
            self.flags.multiline = true;
        }
        if set.dotall {
            self.flags.dotall = true;
        }
        if set.comments {
            self.flags.comments = true;
        }
        if set.unicode_class {
            self.flags.unicode_class = true;
            self.flags.unicode_case = true;
        }
        if set.unix_lines {
            self.flags.unix_lines = true;
        }
        if set.unicode_case {
            self.flags.unicode_case = true;
        }
        if set.canon_eq {
            self.flags.canon_eq = true;
        }
        if clear.case_insensitive {
            self.flags.case_insensitive = false;
        }
        if clear.multiline {
            self.flags.multiline = false;
        }
        if clear.dotall {
            self.flags.dotall = false;
        }
        if clear.comments {
            self.flags.comments = false;
        }
        if clear.unicode_class {
            self.flags.unicode_class = false;
            self.flags.unicode_case = false;
        }
        if clear.unix_lines {
            self.flags.unix_lines = false;
        }
        if clear.unicode_case {
            self.flags.unicode_case = false;
        }
        if clear.canon_eq {
            self.flags.canon_eq = false;
        }
    }

    fn maybe_parse_quantifier(&mut self, node: Node) -> Result<Node, PatternSyntaxError> {
        if self.flags.comments {
            self.skip_comments_whitespace();
        }
        let (min, max) = match self.peek() {
            Some(0x2a) => {
                self.advance();
                (0, u32::MAX)
            }
            Some(0x2b) => {
                self.advance();
                (1, u32::MAX)
            }
            Some(0x3f) => {
                self.advance();
                (0, 1)
            }
            Some(0x7b) => {
                self.advance();
                match self.parse_quantifier_braces() {
                    Ok((min, max)) => (min, max),
                    Err(_) => {
                        return Err(
                            self.error(format!("Illegal repetition near index {}", self.pos))
                        );
                    }
                }
            }
            _ => return Ok(node),
        };

        let kind = match self.peek() {
            Some(0x3f) => {
                self.advance();
                QuantKind::Reluctant
            }
            Some(0x2b) => {
                self.advance();
                QuantKind::Possessive
            }
            _ => QuantKind::Greedy,
        };

        Ok(Node::Quantified {
            inner: Box::new(node),
            min,
            max,
            kind,
        })
    }

    fn parse_quantifier_braces(&mut self) -> Result<(u32, u32), PatternSyntaxError> {
        let mut min_str = String::new();
        while let Some(c) = self.peek() {
            if c.is_ascii_digit() {
                min_str.push(
                    core::char::from_u32(c)
                        .ok_or_else(|| self.error("Invalid syntax code point".to_string()))?,
                );
                self.advance();
            } else {
                break;
            }
        }
        if min_str.is_empty() {
            return Err(self.error("Invalid quantifier".to_string()));
        }
        let min: u32 = min_str
            .parse()
            .map_err(|_| self.error("Invalid quantifier number".to_string()))?;

        match self.peek() {
            Some(0x7d) => {
                self.advance();
                Ok((min, min))
            }
            Some(0x2c) => {
                self.advance();
                if self.peek() == Some(0x7d) {
                    self.advance();
                    Ok((min, u32::MAX))
                } else {
                    let mut max_str = String::new();
                    while let Some(c) = self.peek() {
                        if c.is_ascii_digit() {
                            max_str.push(core::char::from_u32(c).ok_or_else(|| {
                                self.error("Invalid syntax code point".to_string())
                            })?);
                            self.advance();
                        } else {
                            break;
                        }
                    }
                    self.expect(0x7d)?;
                    let max: u32 = max_str
                        .parse()
                        .map_err(|_| self.error("Invalid quantifier number".to_string()))?;
                    if min > max {
                        return Err(
                            self.error(format!("Illegal repetition range near index {}", self.pos))
                        );
                    }
                    Ok((min, max))
                }
            }
            _ => Err(self.error("Invalid quantifier".to_string())),
        }
    }

    // ==================== Character Class Parsing ====================

    fn parse_char_class_node(&mut self) -> Result<Node, PatternSyntaxError> {
        let cc = self.parse_char_class()?;
        if self.flags.canon_eq && !self.flags.literal {
            Ok(Node::NfcCharClass(cc))
        } else {
            Ok(Node::CharClass(cc))
        }
    }

    fn parse_char_class(&mut self) -> Result<CharClass, PatternSyntaxError> {
        let previous_depth = self.depth;
        let result = self.parse_class_tasks();
        self.depth = previous_depth;
        result
    }

    /// Explicit continuations preserve clazz(consume) and its intersection RHS
    /// scope without recursive Rust calls. Only real bracket entries count
    /// toward the shared 256 budget, just as before; an RHS body that stops at
    /// its parent's ']' is not another bracket entry.
    fn parse_class_tasks(&mut self) -> Result<CharClass, PatternSyntaxError> {
        let mut tasks = vec![ClassTask::Bracket];
        let mut result: Option<ClassResult> = None;
        while let Some(task) = tasks.pop() {
            match task {
                ClassTask::Bracket => {
                    if self.depth >= 256 {
                        return Err(self.nesting_budget(
                            "Character class nesting exceeds backend compilation budget"
                                .to_string(),
                        ));
                    }
                    self.depth += 1;
                    self.expect(0x5b)?;
                    let negated = if self.peek() == Some(0x5e) {
                        self.advance();
                        true
                    } else {
                        false
                    };
                    tasks.push(ClassTask::FinishBracket { negated });
                    tasks.push(ClassTask::Body(ClassBody::new(true)));
                }
                ClassTask::FinishBracket { negated } => {
                    let items = result.take().expect("completed class body").items();
                    self.depth -= 1;
                    result = Some(ClassResult::Class(CharClass { negated, items }));
                }
                ClassTask::AppendBodyNested(mut body) => {
                    body.items.push(CharClassItem::Nested(
                        result.take().expect("completed nested class").class(),
                    ));
                    body.at_start = false;
                    tasks.push(ClassTask::Body(body));
                }
                ClassTask::AppendRhsNested(mut items) => {
                    items.push(CharClassItem::Nested(
                        result.take().expect("completed nested RHS class").class(),
                    ));
                    tasks.push(ClassTask::Rhs(items));
                }
                ClassTask::Intersect(mut body) => {
                    let right = result.take().expect("completed intersection RHS").items();
                    body.items = vec![CharClassItem::Intersection(body.items, right)];
                    body.at_start = false;
                    tasks.push(ClassTask::Body(body));
                }
                ClassTask::ExtendRhs(mut items) => {
                    items.extend(result.take().expect("completed RHS body").items());
                    tasks.push(ClassTask::Rhs(items));
                }
                ClassTask::RejectNestedRangeEnd => {
                    // Fully parse the endpoint first: its syntax error must
                    // precede the outer range's non-single-character error.
                    let _class = result.take().expect("completed range endpoint").class();
                    return Err(self.error("Illegal character range".to_string()));
                }
                ClassTask::Body(mut body) => {
                    if self.flags.comments {
                        self.skip_comments_whitespace();
                    }
                    match self.peek() {
                        None => return Err(self.error("Unclosed character class".to_string())),
                        Some(0x5d) if body.at_start => {
                            self.advance();
                            body.items.push(CharClassItem::Single(0x5d));
                            body.at_start = false;
                            tasks.push(ClassTask::Body(body));
                        }
                        Some(0x5d) => {
                            if body.consume_closing {
                                self.advance();
                            }
                            result = Some(ClassResult::Items(body.items));
                        }
                        Some(0x5b) => {
                            tasks.push(ClassTask::AppendBodyNested(body));
                            tasks.push(ClassTask::Bracket);
                        }
                        Some(0x26) if self.chars.get(self.pos + 1) == Some(&0x26) => {
                            self.pos += 2;
                            tasks.push(ClassTask::Intersect(body));
                            tasks.push(ClassTask::Rhs(Vec::new()));
                        }
                        _ => {
                            body.at_start = false;
                            let item = self.parse_char_class_item()?;
                            if self.peek() == Some(0x2d)
                                && self.pos + 1 < self.chars.len()
                                && self.chars[self.pos + 1] != 0x5d
                                && let CharClassItem::Single(start) = item
                            {
                                self.advance();
                                if self.peek() == Some(0x5b) {
                                    tasks.push(ClassTask::RejectNestedRangeEnd);
                                    tasks.push(ClassTask::Bracket);
                                    continue;
                                }
                                let end_item = self.parse_char_class_item()?;
                                let CharClassItem::Single(end) = end_item else {
                                    return Err(self.error("Illegal character range".to_string()));
                                };
                                if start > end {
                                    return Err(self.error(format!(
                                        "Invalid range: {}-{}",
                                        super::unicode::display_codepoint(start),
                                        super::unicode::display_codepoint(end),
                                    )));
                                }
                                body.items.push(CharClassItem::Range(start, end));
                                tasks.push(ClassTask::Body(body));
                                continue;
                            }
                            body.items.push(item);
                            tasks.push(ClassTask::Body(body));
                        }
                    }
                }
                ClassTask::Rhs(items) => {
                    if self.flags.comments {
                        self.skip_comments_whitespace();
                    }
                    match self.peek() {
                        None => return Err(self.error("Unclosed character class".to_string())),
                        Some(0x5d) => {
                            if items.is_empty() {
                                return Err(self.error("Empty intersection operand".to_string()));
                            }
                            result = Some(ClassResult::Items(items));
                        }
                        Some(0x26) if self.chars.get(self.pos + 1) == Some(&0x26) => {
                            if items.is_empty() {
                                return Err(self.error("Empty intersection operand".to_string()));
                            }
                            result = Some(ClassResult::Items(items));
                        }
                        Some(0x5b) => {
                            tasks.push(ClassTask::AppendRhsNested(items));
                            tasks.push(ClassTask::Bracket);
                        }
                        _ => {
                            // The RHS literal starts a clazz(false) scope:
                            // further && belongs to it, not the outer body.
                            tasks.push(ClassTask::ExtendRhs(items));
                            tasks.push(ClassTask::Body(ClassBody::new(false)));
                        }
                    }
                }
            }
        }
        Ok(result.expect("completed outer class").class())
    }

    fn parse_char_class_item(&mut self) -> Result<CharClassItem, PatternSyntaxError> {
        match self.peek() {
            Some(0x5c) => {
                self.advance();
                let c = self
                    .advance()
                    .ok_or_else(|| self.error("Unexpected end in character class".to_string()))?;
                match c {
                    0x64 => Ok(CharClassItem::Predefined(PredefinedClass::Digit)),
                    0x44 => Ok(CharClassItem::Predefined(PredefinedClass::NonDigit)),
                    0x77 => Ok(CharClassItem::Predefined(PredefinedClass::Word)),
                    0x57 => Ok(CharClassItem::Predefined(PredefinedClass::NonWord)),
                    0x73 => Ok(CharClassItem::Predefined(PredefinedClass::Whitespace)),
                    0x53 => Ok(CharClassItem::Predefined(PredefinedClass::NonWhitespace)),
                    0x68 => Ok(CharClassItem::Predefined(PredefinedClass::HorizWhitespace)),
                    0x48 => Ok(CharClassItem::Predefined(
                        PredefinedClass::NonHorizWhitespace,
                    )),
                    0x76 => Ok(CharClassItem::Predefined(PredefinedClass::VertWhitespace)),
                    0x56 => Ok(CharClassItem::Predefined(
                        PredefinedClass::NonVertWhitespace,
                    )),
                    0x70 => {
                        let (name, negated) = self.parse_property_name(false)?;
                        Ok(CharClassItem::UnicodeProperty { name, negated })
                    }
                    0x50 => {
                        let (name, negated) = self.parse_property_name(true)?;
                        Ok(CharClassItem::UnicodeProperty { name, negated })
                    }
                    0x74 => Ok(CharClassItem::Single(0x09)),
                    0x6e => Ok(CharClassItem::Single(0x0a)),
                    0x72 => Ok(CharClassItem::Single(0x0d)),
                    0x66 => Ok(CharClassItem::Single(0x0c)),
                    0x61 => Ok(CharClassItem::Single(0x07)),
                    0x65 => Ok(CharClassItem::Single(0x1b)),
                    0x4e => self.parse_named_character().map(CharClassItem::Single),
                    0x78 => self.parse_hex_char().map(CharClassItem::Single),
                    0x75 => self.parse_unicode_char().map(CharClassItem::Single),
                    0x30 => self.parse_octal_char().map(CharClassItem::Single),
                    0x63 => self.parse_control_char().map(CharClassItem::Single),
                    0x51 => {
                        let mut items = Vec::new();
                        loop {
                            if self.pos >= self.chars.len() {
                                break;
                            }
                            if self.pos + 1 < self.chars.len()
                                && self.chars[self.pos] == 0x5c
                                && self.chars[self.pos + 1] == 0x45
                            {
                                self.pos += 2;
                                break;
                            }
                            items.push(CharClassItem::Single(self.chars[self.pos]));
                            self.pos += 1;
                        }
                        if items.len() == 1 {
                            return Ok(items.into_iter().next().unwrap());
                        }
                        Ok(CharClassItem::Nested(CharClass {
                            negated: false,
                            items,
                        }))
                    }
                    0x31..=0x39 | 0x41..=0x5a | 0x61..=0x7a => {
                        Err(self
                            .error_at("Illegal/unsupported escape sequence".into(), self.pos - 1))
                    }
                    _ => Ok(CharClassItem::Single(c)),
                }
            }
            Some(0x5b) => unreachable!("nested class is handled by a class continuation"),
            Some(c) => {
                self.advance();
                Ok(CharClassItem::Single(c))
            }
            None => Err(self.error("Unexpected end in character class".to_string())),
        }
    }
}

// Mirrors whether newCharProperty receives a non-Bmp predicate. Literal
// sequences become Slice/SliceS and do not themselves flip global StartS.
fn node_starts_s(mut node: &Node, flags: Flags) -> bool {
    while let Node::Quantified { inner, .. } = node {
        node = inner;
    }
    match node {
        Node::Literal(cp) => {
            *cp >= 0x10000
                || (0xd800..=0xdfff).contains(cp)
                || flags.case_insensitive
                    && flags.unicode_case
                    && super::unicode::simple_upper(*cp)
                        != super::unicode::simple_lower(super::unicode::simple_upper(*cp))
        }
        Node::CharClass(cc) => {
            cc.negated || cc.items.iter().any(|item| class_starts_s(item, flags))
        }
        // Group continuations update the same global field when their branches finish.
        _ => false,
    }
}
fn class_starts_s(item: &CharClassItem, flags: Flags) -> bool {
    let mut pending = vec![item];
    while let Some(item) = pending.pop() {
        let supplementary = match item {
            CharClassItem::Single(cp) => node_starts_s(&Node::Literal(*cp), flags),
            CharClassItem::Range(a, b) => {
                *b >= 0x10000 || (*a <= 0xdfff && *b >= 0xd800) || flags.case_insensitive
            }
            CharClassItem::UnicodeProperty { name, negated } => {
                *negated || super::unicode::property_is_supplementary(name, flags.unicode_class)
            }
            CharClassItem::Predefined(pc) => match pc {
                PredefinedClass::Digit | PredefinedClass::Word | PredefinedClass::Whitespace => {
                    flags.unicode_class
                }
                PredefinedClass::HorizWhitespace | PredefinedClass::VertWhitespace => false,
                _ => true,
            },
            CharClassItem::Nested(cc) => {
                if cc.negated {
                    return true;
                }
                pending.extend(cc.items.iter().rev());
                false
            }
            CharClassItem::Intersection(a, b) => {
                pending.extend(b.iter().rev());
                pending.extend(a.iter().rev());
                false
            }
        };
        if supplementary {
            return true;
        }
    }
    false
}
