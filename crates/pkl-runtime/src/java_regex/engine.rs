use std::collections::BTreeMap;

use super::types::*;
use super::unicode::*;
use super::{Limits, MatchError, input::Input};

/// Greediness mode shared by `match_greedy` / `match_reluctant` and their
/// `try_match_atom_*` dispatchers. Used to parameterize the small number of
/// behavioral diffs between the two: which continuation node to thread into
/// chain-based bodies (Path 2), and whether to try `rest` after a zero-width
/// Path-1 body match.
#[derive(Copy, Clone, PartialEq, Eq)]
enum Mode {
    Greedy,
    Reluctant,
}

/// Zero-allocation borrowed continuation stack for matching tails of groups,
/// alternations, quantifiers, and lookbehinds without cloning suffix `Node`s.
enum Cont<'a> {
    Empty,
    Slice {
        nodes: &'a [Node],
        next: &'a Cont<'a>,
    },
    GroupEnd {
        index: usize,
        start: usize,
        next: &'a Cont<'a>,
    },
    RestoreFlags {
        flags: Flags,
        next: &'a Cont<'a>,
    },
    PositionCheck {
        target: usize,
        next: &'a Cont<'a>,
    },
    Quant {
        mode: Mode,
        atom: &'a Node,
        min: u32,
        max: u32,
        count: u32,
        prev_pos: usize,
        rest: &'a [Node],
        rest_cont: &'a Cont<'a>,
    },
}

/// Whether a `Pattern` is "deterministic" in OpenJDK's sense — i.e., would be
/// compiled with `GroupCurly` (atomic) rather than `Loop` (with backtracking)
/// when used as a quantifier atom. Mirrors `TreeInfo.deterministic` propagation
/// in `java.util.regex.Pattern`.
///
/// The practical effect: when a single-branch deterministic body is quantified
/// like `(?:\R){2}` or `(?i:\R){2}`, the engine must not backtrack into the
/// atom's internal choices (e.g., `\R`'s `\r\n` vs single-char). Multi-branch
/// bodies (alternation) keep backtracking, matching OpenJDK's `Loop`.
#[inline]
fn is_deterministic_body(p: &Pattern) -> bool {
    p.meta.deterministic
}

#[inline]
fn pattern_min_length(p: &Pattern) -> usize {
    p.meta.min_length
}

#[inline]
fn pattern_max_length(p: &Pattern) -> Option<usize> {
    p.meta.max_length
}

#[inline]
fn pattern_java_max(p: &Pattern) -> i32 {
    p.meta.java_max
}

fn single_char_lowercase(c: u32) -> Option<u32> {
    Some(simple_lower(c))
}
fn single_char_uppercase(c: u32) -> Option<u32> {
    Some(simple_upper(c))
}

pub struct Engine<'a> {
    pub error: Option<MatchError>,
    pub input: Input<'a>,
    pub flags: Flags,
    pub named_groups: &'a BTreeMap<String, usize>,
    steps: u64,
    max_steps: u64,
    class_work: u64,
    depth: u32,
    max_depth: u32,
    pub search_start: usize,
    /// Inclusive lower bound for matching positions (default 0).
    /// Mirrors OpenJDK `Matcher.from`.
    pub text_start: usize,
    /// Exclusive upper bound for matching positions (default `input.len()`).
    /// Mirrors OpenJDK `Matcher.to`. Anchors and bounds checks use this
    /// instead of `input.len()` so the full input is available for
    /// context-dependent lookups (e.g. `\Z`'s "previous char is \r" check
    /// needs to see chars OUTSIDE the region — same as OpenJDK's Dollar).
    pub text_end: usize,
}

#[derive(Clone, Debug)]
pub struct State {
    pub captures: Vec<Option<(usize, usize)>>,
    pub match_end: usize,
}

impl State {
    pub fn new(group_count: usize) -> Self {
        State {
            captures: vec![None; group_count + 1],
            match_end: 0,
        }
    }
}

impl<'a> Engine<'a> {
    pub fn new(input: &'a [u16], flags: Flags, named_groups: &'a BTreeMap<String, usize>) -> Self {
        Engine {
            input: Input(input),
            flags,
            named_groups,
            error: None,
            steps: 0,
            max_steps: 5_000_000,
            class_work: 0,
            depth: 0,
            max_depth: 500,
            search_start: 0,
            text_start: 0,
            text_end: input.len(),
        }
    }

    /// Effective end-of-text for matching: `text_end`, mirroring `Matcher.to`.
    #[inline]
    pub fn text_len(&self) -> usize {
        self.text_end
    }

    pub fn set_limits(&mut self, limits: Limits) {
        self.max_steps = limits.steps;
        self.max_depth = limits.depth;
    }

    pub(super) fn budget_state(&self) -> (u64, Option<MatchError>) {
        (self.steps, self.error.clone())
    }
    pub(super) fn restore_budget(&mut self, steps: u64, error: Option<MatchError>) {
        self.steps = steps;
        self.error = error;
    }

    fn step(&mut self) -> bool {
        if self.error.is_some() {
            return false;
        }
        if self.steps >= self.max_steps {
            self.error = Some(MatchError::StepBudgetExceeded {
                limit: self.max_steps,
            });
            return false;
        }
        self.steps += 1;
        true
    }

    fn step_class_work(&mut self) -> bool {
        if self.error.is_some() {
            return false;
        }
        let limit = self.max_steps.max(65_536);
        if self.class_work >= limit {
            self.error = Some(MatchError::StepBudgetExceeded {
                limit: self.max_steps,
            });
            return false;
        }
        self.class_work += 1;
        true
    }

    /// Try to match the pattern at `pos`, using the caller-provided State so
    /// captures can persist across iterations. This mirrors OpenJDK's
    /// `Start.match` loop: between successive position attempts, `groups[]` is
    /// never cleared, so a failed attempt's captures (from internal paths that
    /// succeeded before the overall match failed — e.g. negative lookarounds
    /// whose inner matched) leak into subsequent attempts.
    ///
    /// Returns `Some(end_pos)` on success and `None` on failure. State is
    /// mutated either way.
    pub fn try_match_at_persistent(
        &mut self,
        pattern: &Pattern,
        pos: usize,
        state: &mut State,
    ) -> Option<usize> {
        for branch in pattern.branches.iter() {
            if self.error.is_some() {
                break;
            }
            if self.match_nodes(branch, &Cont::Empty, pos, state) {
                return Some(state.match_end);
            }
            // Intentionally NO save/restore: failed branch's mutations persist,
            // matching OpenJDK's `Branch.match` which similarly never resets
            // `matcher.groups[]` between alternatives.
        }
        None
    }

    pub fn match_pattern(
        &mut self,
        pattern: &Pattern,
        rest: &[Node],
        pos: usize,
        state: &mut State,
    ) -> bool {
        self.match_pattern_cont(pattern, rest, &Cont::Empty, pos, state)
    }

    fn match_pattern_cont(
        &mut self,
        pattern: &Pattern,
        rest: &[Node],
        cont: &Cont<'_>,
        pos: usize,
        state: &mut State,
    ) -> bool {
        let tail = Cont::Slice {
            nodes: rest,
            next: cont,
        };
        let next_cont = if rest.is_empty() { cont } else { &tail };
        for branch in pattern.branches.iter() {
            if self.error.is_some() {
                break;
            }
            if self.match_nodes(branch, next_cont, pos, state) {
                return true;
            }
            // No save/restore between branches — captures from failed branches
            // leak into subsequent branches, matching OpenJDK's `Branch.match`
            // which doesn't reset `matcher.groups[]` between alternatives. This
            // is what makes capture leaks visible across nested lookarounds:
            // `(?<!(?!(?<bar>.)))` on "\t\n" leaks `bar=\t` from the inner
            // capture that the inversion-then-inversion buried.
        }
        false
    }

    fn match_nodes<'b>(
        &mut self,
        nodes: &'b [Node],
        cont: &Cont<'b>,
        pos: usize,
        state: &mut State,
    ) -> bool {
        if pos > self.text_end || !self.step() {
            return false;
        }
        self.depth += 1;
        if self.depth > self.max_depth {
            self.depth -= 1;
            self.error = Some(MatchError::DepthBudgetExceeded {
                limit: self.max_depth,
            });
            return false;
        }
        let result = self.match_nodes_inner(nodes, cont, pos, state);
        self.depth -= 1;
        result && self.error.is_none()
    }

    fn match_nodes_inner<'b>(
        &mut self,
        mut nodes: &'b [Node],
        mut cont: &Cont<'b>,
        mut pos: usize,
        state: &mut State,
    ) -> bool {
        // Iteratively consume sequential non-branching nodes (`Literal`, `Dot`,
        // `Anchor`, `CharClass`, `PositionCheck`) and slice continuations so
        // long literal/class sequences (>500 chars) do not exhaust recursion depth.
        let mut stepped = false;
        loop {
            while nodes.is_empty() {
                match cont {
                    Cont::Empty => {
                        if pos > self.text_end {
                            return false;
                        }
                        state.match_end = pos;
                        return true;
                    }
                    Cont::Slice {
                        nodes: next_nodes,
                        next,
                    } => {
                        nodes = next_nodes;
                        cont = next;
                    }
                    Cont::PositionCheck { target, next } => {
                        if pos != *target {
                            return false;
                        }
                        cont = next;
                    }
                    Cont::GroupEnd { index, start, next } => {
                        let saved = state.captures[*index];
                        state.captures[*index] = Some((*start, pos));
                        if self.match_nodes(&[], next, pos, state) {
                            return true;
                        }
                        state.captures[*index] = saved;
                        return false;
                    }
                    Cont::RestoreFlags { flags, next } => {
                        let saved = self.flags;
                        self.flags = *flags;
                        if self.match_nodes(&[], next, pos, state) {
                            return true;
                        }
                        self.flags = saved;
                        return false;
                    }
                    Cont::Quant {
                        mode,
                        atom,
                        min,
                        max,
                        count,
                        prev_pos,
                        rest,
                        rest_cont,
                    } => {
                        if pos == *prev_pos {
                            if *count < *min {
                                return match mode {
                                    Mode::Greedy => self.match_greedy(
                                        atom, *min, *max, *count, rest, rest_cont, pos, *prev_pos,
                                        state,
                                    ),
                                    Mode::Reluctant => self.match_reluctant(
                                        atom, *min, *max, *count, rest, rest_cont, pos, *prev_pos,
                                        state,
                                    ),
                                };
                            }
                            return match mode {
                                Mode::Greedy => self.match_nodes(rest, rest_cont, pos, state),
                                Mode::Reluctant => {
                                    if *count == *min {
                                        self.match_nodes(rest, rest_cont, pos, state)
                                    } else {
                                        false
                                    }
                                }
                            };
                        }
                        return match mode {
                            Mode::Greedy => self.match_greedy(
                                atom, *min, *max, *count, rest, rest_cont, pos, *prev_pos, state,
                            ),
                            Mode::Reluctant => self.match_reluctant(
                                atom, *min, *max, *count, rest, rest_cont, pos, *prev_pos, state,
                            ),
                        };
                    }
                }
            }
            if stepped && (pos > self.text_end || !self.step()) {
                return false;
            }
            stepped = true;

            match &nodes[0] {
                Node::Literal(ch) => {
                    if pos < self.text_len() {
                        let cp = self.input.at_bounded(pos, self.text_end);
                        let matched = if self.flags.case_insensitive {
                            chars_eq_ci(cp, *ch, self.flags.unicode_case)
                        } else {
                            cp == *ch
                        };
                        if matched {
                            pos = self.input.next_bounded(pos, self.text_end);
                            nodes = &nodes[1..];
                            continue;
                        }
                    }
                    return false;
                }

                Node::Dot => {
                    if pos < self.text_len()
                        && (self.flags.dotall
                            || !self.is_lt(self.input.at_bounded(pos, self.text_end)))
                    {
                        pos = self.input.next_bounded(pos, self.text_end);
                        nodes = &nodes[1..];
                        continue;
                    }
                    return false;
                }

                Node::Anchor(kind) => {
                    if self.check_anchor(*kind, pos) {
                        nodes = &nodes[1..];
                        continue;
                    }
                    return false;
                }

                Node::CharClass(cc) => {
                    if pos < self.text_len()
                        && self.match_char_class(cc, self.input.at_bounded(pos, self.text_end))
                    {
                        pos = self.input.next_bounded(pos, self.text_end);
                        nodes = &nodes[1..];
                        continue;
                    }
                    return false;
                }

                Node::PositionCheck(target) => {
                    if pos == *target {
                        nodes = &nodes[1..];
                        continue;
                    }
                    return false;
                }

                _ => break,
            }
        }

        match &nodes[0] {
            Node::Literal(_)
            | Node::Dot
            | Node::Anchor(_)
            | Node::CharClass(_)
            | Node::PositionCheck(_) => unreachable!(),

            Node::NfcCharClass(cc) => {
                let limit = self.text_len();
                if pos < limit {
                    let ch0 = self.input.at(pos);
                    let n = if ch0 >= 0x10000 { 2 } else { 1 };
                    // No single Unicode NFC scalar decomposes to more than 4 code points
                    // (<= 8 UTF-16 units); cap prefix length to prevent O(N^3) on long combining runs.
                    let max_end = limit.min(pos.saturating_add(16));
                    let mut j = super::canon_eq::next_grapheme_boundary(self.input.0, pos, max_end);
                    if pos + n == j {
                        if self.match_char_class(cc, ch0) {
                            return self.match_nodes(&nodes[1..], cont, j, state);
                        }
                    } else {
                        while pos + n < j {
                            if let Some(cp) =
                                super::canon_eq::nfc_single_codepoint(&self.input.0[pos..j])
                                && self.match_char_class(cc, cp)
                                && self.match_nodes(&nodes[1..], cont, j, state)
                            {
                                return true;
                            }
                            j = self.input.previous(j);
                        }
                    }
                }
                false
            }

            Node::Group { index, inner, .. } => {
                // No per-branch save/restore on captures — mirrors OpenJDK's
                // Branch.match which doesn't reset groups[] between alternatives.
                // Per-group save/restore happens inside GroupEnd (the GroupTail
                // analog) for ONLY the slot owned by this Group.
                //
                // Flag scoping: thread RestoreFlags(saved) after each branch so
                // inline `(?mu)` setters inside don't leak past the group, and
                // restore `self.flags` after any failed branch so inline flags
                // from a failed branch never leak into later branches or callers.
                let start = pos;
                let saved_flags = self.flags;
                let tail = Cont::Slice {
                    nodes: &nodes[1..],
                    next: cont,
                };
                let restore = Cont::RestoreFlags {
                    flags: saved_flags,
                    next: if nodes.len() > 1 { &tail } else { cont },
                };
                let group_end;
                let next_cont: &Cont<'_> = if let Some(idx) = index {
                    group_end = Cont::GroupEnd {
                        index: *idx,
                        start,
                        next: &restore,
                    };
                    &group_end
                } else {
                    &restore
                };
                for branch in inner.branches.iter() {
                    if self.error.is_some() {
                        break;
                    }
                    self.flags = saved_flags;
                    if self.match_nodes(branch, next_cont, pos, state) {
                        return true;
                    }
                }
                self.flags = saved_flags;
                false
            }

            Node::Quantified {
                inner,
                min,
                max,
                kind,
            } => {
                let rest = &nodes[1..];
                if matches!(
                    inner.as_ref(),
                    Node::Literal(_) | Node::Dot | Node::CharClass(_)
                ) {
                    match kind {
                        QuantKind::Greedy => {
                            self.match_greedy_single(inner, *min, *max, rest, cont, pos, state)
                        }
                        QuantKind::Reluctant => {
                            self.match_reluctant_single(inner, *min, *max, rest, cont, pos, state)
                        }
                        QuantKind::Possessive => {
                            self.match_possessive_single(inner, *min, *max, rest, cont, pos, state)
                        }
                    }
                } else {
                    match kind {
                        QuantKind::Greedy => {
                            self.match_greedy(inner, *min, *max, 0, rest, cont, pos, pos, state)
                        }
                        QuantKind::Reluctant => {
                            self.match_reluctant(inner, *min, *max, 0, rest, cont, pos, pos, state)
                        }
                        QuantKind::Possessive => {
                            self.match_possessive(inner, *min, *max, rest, cont, pos, state)
                        }
                    }
                }
            }

            Node::Lookahead { positive, inner } => {
                // Shares State directly (no temp_state clone) so that captures
                // set by the inner pattern persist even when the surrounding
                // negative assertion inverts the result to failure. This is
                // what OpenJDK does: lookarounds don't save/restore groups[]
                // around the inner match, so captures from `(?=(\w))` leak
                // even when a later `\s` causes the overall attempt to fail.
                // Flag scoping: save/restore self.flags around the inner so
                // inline `(?ims)` setters inside don't leak past the lookaround.
                let saved_flags = self.flags;
                let matched = self.match_pattern_cont(inner, &[], &Cont::Empty, pos, state);
                self.flags = saved_flags;
                if matched == *positive {
                    self.match_nodes(&nodes[1..], cont, pos, state)
                } else {
                    false
                }
            }

            Node::Lookbehind {
                positive,
                inner,
                supplementary,
            } => {
                let saved_flags = self.flags;
                let found = self.check_lookbehind(inner, pos, state, *supplementary);
                self.flags = saved_flags;
                if found == *positive {
                    self.match_nodes(&nodes[1..], cont, pos, state)
                } else {
                    false
                }
            }

            Node::AtomicGroup { inner } => {
                // Shares State directly — captures from the inner's attempts
                // (including failed paths) persist into the caller. Mirrors
                // OpenJDK `Ques(X, INDEPENDENT)` which just does
                // `atom.match && next.match` without any save/restore.
                // Flag scoping: inline `(?ims)` inside doesn't leak past the
                // atomic group.
                let saved_flags = self.flags;
                let ok = self.match_pattern_cont(inner, &[], &Cont::Empty, pos, state);
                self.flags = saved_flags;
                if ok {
                    let end = state.match_end;
                    self.match_nodes(&nodes[1..], cont, end, state)
                } else {
                    false
                }
            }

            Node::Backreference(idx) => {
                self.match_backref_by_index(*idx, &nodes[1..], cont, pos, state)
            }

            Node::NamedBackreference(name) => {
                if let Some(&idx) = self.named_groups.get(name) {
                    self.match_backref_by_index(idx, &nodes[1..], cont, pos, state)
                } else {
                    false
                }
            }

            Node::LinebreakMatcher => {
                // \R is atomic: (?>\r\n|[\n\v\f\r\x85\u{2028}\u{2029}]) — if \r\n
                // matches, normally commit to it. Inside a lookbehind, though,
                // the engine must enumerate every possible match length so the
                // outer PositionCheck constraint can be satisfied. There, fall
                // back to the single-char alternative when the \r\n branch
                // fails downstream.
                if pos < self.text_len() {
                    if self.input.at(pos) == 0x0d
                        && pos + 1 < self.text_len()
                        && self.input.at(pos + 1) == 0x0a
                    {
                        if self.match_nodes(&nodes[1..], cont, pos + 2, state) {
                            return true;
                        }
                        // Backtrack to single-char branch; \R is not atomic.
                        return self.match_nodes(&nodes[1..], cont, self.input.next(pos), state);
                    }
                    if is_linebreak(self.input.at(pos)) {
                        return self.match_nodes(&nodes[1..], cont, self.input.next(pos), state);
                    }
                }
                false
            }

            Node::SetFlags(new_flags) => {
                // Java treats inline `(?s)` (and friends) as a compile-time
                // directive: the flag takes effect for the rest of the pattern,
                // permanently. In particular, when alternation backtracks past
                // a `(?s)` in one branch, the flag remains set for subsequent
                // branches. So we don't roll back on failure here. (Engine state
                // is reset per find() attempt, so this doesn't leak across
                // distinct search positions.) Scoped `(?s:…)` groups use the
                // separate FlagGroup/RestoreFlags pair and DO scope properly.
                self.flags = *new_flags;
                self.match_nodes(&nodes[1..], cont, pos, state)
            }

            Node::FlagGroup { flags, inner } => {
                // No per-branch save/restore on captures — matches OpenJDK's
                // Branch.match (FlagGroup is just a non-capturing group with
                // flag context; the branches don't reset groups[]).
                let old_flags = self.flags;
                self.flags = *flags;
                let tail = Cont::Slice {
                    nodes: &nodes[1..],
                    next: cont,
                };
                let restore = Cont::RestoreFlags {
                    flags: old_flags,
                    next: if nodes.len() > 1 { &tail } else { cont },
                };
                for branch in inner.branches.iter() {
                    if self.error.is_some() {
                        break;
                    }
                    if self.match_nodes(branch, &restore, pos, state) {
                        return true;
                    }
                }
                self.flags = old_flags;
                false
            }

            Node::GraphemeCluster | Node::GraphemeBoundary => {
                self.error = Some(MatchError::Unsupported("Unicode grapheme segmentation"));
                false
            }
        }
    }

    /// Shared backreference matching for both numbered and named backrefs.
    fn backref_end(&mut self, start: usize, end: usize, pos: usize) -> Option<usize> {
        let mut capture = start;
        let mut current = pos;
        while capture < end {
            if !self.step() {
                return None;
            }
            if current >= self.text_len() {
                return None;
            }
            if self.flags.case_insensitive {
                if !chars_eq_ci(
                    self.input.at(current),
                    self.input.at(capture),
                    self.flags.unicode_case,
                ) {
                    return None;
                }
                capture = self.input.next(capture);
                current = self.input.next(current);
                if capture > end || current > self.text_len() {
                    return None;
                }
            } else {
                if self.input.0[current] != self.input.0[capture] {
                    return None;
                }
                capture += 1;
                current += 1;
            }
        }
        Some(current)
    }

    fn match_backref_by_index(
        &mut self,
        idx: usize,
        rest: &[Node],
        cont: &Cont<'_>,
        pos: usize,
        state: &mut State,
    ) -> bool {
        if let Some(Some((start, end))) = state.captures.get(idx).copied()
            && let Some(end) = self.backref_end(start, end, pos)
        {
            return self.match_nodes(rest, cont, end, state);
        }
        false
    }

    #[inline]
    fn match_single_atom(&mut self, atom: &Node, pos: usize) -> Option<usize> {
        if pos >= self.text_len() {
            return None;
        }
        let ch = self.input.at_bounded(pos, self.text_end);
        let ok = match atom {
            Node::Literal(target) => {
                if self.flags.case_insensitive {
                    chars_eq_ci(ch, *target, self.flags.unicode_case)
                } else {
                    ch == *target
                }
            }
            Node::Dot => self.flags.dotall || !self.is_lt(ch),
            Node::CharClass(cc) => self.match_char_class(cc, ch),
            _ => false,
        };
        ok.then(|| self.input.next_bounded(pos, self.text_end))
    }

    /// Iterative greedy quantifier for single-codepoint atoms (`.*`, `.+`, `[a-z]+`, etc.),
    /// avoiding O(text_len) recursion depth and DepthBudgetExceeded on inputs > 500 chars.
    #[allow(clippy::too_many_arguments)]
    fn match_greedy_single(
        &mut self,
        atom: &Node,
        min: u32,
        max: u32,
        rest: &[Node],
        cont: &Cont<'_>,
        pos: usize,
        state: &mut State,
    ) -> bool {
        let mut cur = pos;
        let mut count = 0u32;
        while count < min {
            if !self.step() {
                return false;
            }
            let Some(next) = self.match_single_atom(atom, cur) else {
                return false;
            };
            cur = next;
            count += 1;
        }
        let mut positions = Vec::new();
        positions.push(cur);
        while count < max {
            if !self.step() {
                return false;
            }
            let Some(next) = self.match_single_atom(atom, cur) else {
                break;
            };
            cur = next;
            count += 1;
            positions.push(cur);
        }
        for &p in positions.iter().rev() {
            if self.match_nodes(rest, cont, p, state) {
                return true;
            }
            if self.error.is_some() {
                return false;
            }
        }
        false
    }

    /// Iterative reluctant quantifier for single-codepoint atoms (`.*?`, `.+?`, etc.).
    #[allow(clippy::too_many_arguments)]
    fn match_reluctant_single(
        &mut self,
        atom: &Node,
        min: u32,
        max: u32,
        rest: &[Node],
        cont: &Cont<'_>,
        pos: usize,
        state: &mut State,
    ) -> bool {
        let mut cur = pos;
        let mut count = 0u32;
        while count < min {
            if !self.step() {
                return false;
            }
            let Some(next) = self.match_single_atom(atom, cur) else {
                return false;
            };
            cur = next;
            count += 1;
        }
        loop {
            if self.match_nodes(rest, cont, cur, state) {
                return true;
            }
            if self.error.is_some() || count >= max || !self.step() {
                return false;
            }
            let Some(next) = self.match_single_atom(atom, cur) else {
                return false;
            };
            cur = next;
            count += 1;
        }
    }

    /// Iterative possessive quantifier for single-codepoint atoms (`.*+`, `[a-z]++`, etc.).
    #[allow(clippy::too_many_arguments)]
    fn match_possessive_single(
        &mut self,
        atom: &Node,
        min: u32,
        max: u32,
        rest: &[Node],
        cont: &Cont<'_>,
        pos: usize,
        state: &mut State,
    ) -> bool {
        let mut cur = pos;
        let mut count = 0u32;
        while count < min {
            if !self.step() {
                return false;
            }
            let Some(next) = self.match_single_atom(atom, cur) else {
                return false;
            };
            cur = next;
            count += 1;
        }
        while count < max {
            if !self.step() {
                return false;
            }
            let Some(next) = self.match_single_atom(atom, cur) else {
                break;
            };
            cur = next;
            count += 1;
        }
        self.match_nodes(rest, cont, cur, state)
    }

    #[allow(clippy::too_many_arguments)]
    fn match_greedy(
        &mut self,
        atom: &Node,
        min: u32,
        max: u32,
        count: u32,
        rest: &[Node],
        cont: &Cont<'_>,
        pos: usize,
        iter_start: usize,
        state: &mut State,
    ) -> bool {
        if !self.step() {
            return false;
        }

        if count < max {
            // No save/restore — captures from failed quantifier-atom attempts
            // leak (matching OpenJDK Curly/Loop, which never reset matcher.groups[]).
            if self.try_match_atom_mode(Mode::Greedy, atom, min, max, count, rest, cont, pos, state)
            {
                return true;
            }
        }

        if count >= min && self.match_nodes(rest, cont, pos, state) {
            if count > 0
                && let Node::Group {
                    index: Some(idx),
                    inner,
                    ..
                } = atom
            {
                let is_ques = min == 0 && max == 1;
                let chain_based = is_ques || !is_deterministic_body(inner);
                if !chain_based {
                    state.captures[*idx] = Some((iter_start, pos));
                }
            }
            return true;
        }

        false
    }

    /// Recurse back into the appropriate top-level wrapper for the given mode.
    #[allow(clippy::too_many_arguments)]
    fn match_quant(
        &mut self,
        mode: Mode,
        atom: &Node,
        min: u32,
        max: u32,
        count: u32,
        rest: &[Node],
        cont: &Cont<'_>,
        pos: usize,
        iter_start: usize,
        state: &mut State,
    ) -> bool {
        if !self.step() {
            return false;
        }
        self.depth += 1;
        if self.depth > self.max_depth {
            self.depth -= 1;
            self.error = Some(MatchError::DepthBudgetExceeded {
                limit: self.max_depth,
            });
            return false;
        }
        let result = match mode {
            Mode::Greedy => {
                self.match_greedy(atom, min, max, count, rest, cont, pos, iter_start, state)
            }
            Mode::Reluctant => {
                self.match_reluctant(atom, min, max, count, rest, cont, pos, iter_start, state)
            }
        };
        self.depth -= 1;
        result && self.error.is_none()
    }

    /// Unified atom-expansion dispatcher for both greedy and reluctant
    /// quantifiers.
    #[allow(clippy::too_many_arguments)]
    fn try_match_atom_mode(
        &mut self,
        mode: Mode,
        atom: &Node,
        min: u32,
        max: u32,
        count: u32,
        rest: &[Node],
        cont: &Cont<'_>,
        pos: usize,
        state: &mut State,
    ) -> bool {
        let try_rest_zw = |count: u32| -> bool {
            match mode {
                Mode::Greedy => true,
                Mode::Reluctant => count + 1 == min,
            }
        };

        match atom {
            Node::Literal(ch) => {
                if pos < self.text_len() {
                    let cp = self.input.at_bounded(pos, self.text_end);
                    let matched = if self.flags.case_insensitive {
                        chars_eq_ci(cp, *ch, self.flags.unicode_case)
                    } else {
                        cp == *ch
                    };
                    if matched {
                        return self.match_quant(
                            mode,
                            atom,
                            min,
                            max,
                            count + 1,
                            rest,
                            cont,
                            self.input.next_bounded(pos, self.text_end),
                            pos,
                            state,
                        );
                    }
                }
                false
            }
            Node::Dot => {
                if pos < self.text_len()
                    && (self.flags.dotall || !self.is_lt(self.input.at_bounded(pos, self.text_end)))
                {
                    self.match_quant(
                        mode,
                        atom,
                        min,
                        max,
                        count + 1,
                        rest,
                        cont,
                        self.input.next_bounded(pos, self.text_end),
                        pos,
                        state,
                    )
                } else {
                    false
                }
            }
            Node::CharClass(cc) => {
                if pos < self.text_len()
                    && self.match_char_class(cc, self.input.at_bounded(pos, self.text_end))
                {
                    self.match_quant(
                        mode,
                        atom,
                        min,
                        max,
                        count + 1,
                        rest,
                        cont,
                        self.input.next_bounded(pos, self.text_end),
                        pos,
                        state,
                    )
                } else {
                    false
                }
            }
            Node::Group { index, inner, .. } => {
                let start = pos;
                let deterministic = is_deterministic_body(inner);
                let is_ques = min == 0 && max == 1;
                let chain_based = is_ques || !deterministic;
                let saved_cap = index.map(|idx| state.captures[idx]);
                let saved_flags = self.flags;
                for branch in inner.branches.iter() {
                    if self.error.is_some() {
                        break;
                    }
                    self.flags = saved_flags;
                    if chain_based {
                        let quant_cont = Cont::Quant {
                            mode,
                            atom,
                            min,
                            max,
                            count: count + 1,
                            prev_pos: pos,
                            rest,
                            rest_cont: cont,
                        };
                        let restore = Cont::RestoreFlags {
                            flags: saved_flags,
                            next: &quant_cont,
                        };
                        let group_end;
                        let next_cont: &Cont<'_> = if let Some(idx) = index {
                            group_end = Cont::GroupEnd {
                                index: *idx,
                                start,
                                next: &restore,
                            };
                            &group_end
                        } else {
                            &restore
                        };
                        if self.match_nodes(branch, next_cont, pos, state) {
                            return true;
                        }
                        self.flags = saved_flags;
                    } else {
                        let restore = Cont::RestoreFlags {
                            flags: saved_flags,
                            next: &Cont::Empty,
                        };
                        let group_end;
                        let next_cont: &Cont<'_> = if let Some(idx) = index {
                            group_end = Cont::GroupEnd {
                                index: *idx,
                                start,
                                next: &restore,
                            };
                            &group_end
                        } else {
                            &restore
                        };
                        let saved_match_end = state.match_end;
                        let branch_ok = self.match_nodes(branch, next_cont, pos, state);
                        self.flags = saved_flags;
                        if branch_ok {
                            let new_pos = state.match_end;
                            if new_pos > pos {
                                if self.match_quant(
                                    mode,
                                    atom,
                                    min,
                                    max,
                                    count + 1,
                                    rest,
                                    cont,
                                    new_pos,
                                    pos,
                                    state,
                                ) {
                                    return true;
                                }
                                if let Some(idx) = index {
                                    state.captures[*idx] = saved_cap.flatten();
                                }
                            } else {
                                if count == 0
                                    && min == 0
                                    && max > 1
                                    && pattern_max_length(inner) == Some(0)
                                    && let Some(idx) = index
                                {
                                    state.captures[*idx] = saved_cap.flatten();
                                }
                                if (count + 1) < min {
                                    if self.match_quant(
                                        mode,
                                        atom,
                                        min,
                                        max,
                                        count + 1,
                                        rest,
                                        cont,
                                        pos,
                                        pos,
                                        state,
                                    ) {
                                        return true;
                                    }
                                    if let Some(idx) = index {
                                        state.captures[*idx] = saved_cap.flatten();
                                    }
                                } else if try_rest_zw(count)
                                    && self.match_nodes(rest, cont, pos, state)
                                {
                                    return true;
                                } else if let Some(idx) = index {
                                    state.captures[*idx] = saved_cap.flatten();
                                }
                            }
                            state.match_end = saved_match_end;
                            self.flags = saved_flags;
                        } else {
                            state.match_end = saved_match_end;
                        }
                    }
                }
                self.flags = saved_flags;
                false
            }
            Node::FlagGroup { flags, inner } => {
                let deterministic = is_deterministic_body(inner);
                let is_ques = min == 0 && max == 1;
                let chain_based = is_ques || !deterministic;
                let old_flags = self.flags;
                self.flags = *flags;
                for branch in inner.branches.iter() {
                    if self.error.is_some() {
                        break;
                    }
                    if chain_based {
                        let quant_cont = Cont::Quant {
                            mode,
                            atom,
                            min,
                            max,
                            count: count + 1,
                            prev_pos: pos,
                            rest,
                            rest_cont: cont,
                        };
                        let restore = Cont::RestoreFlags {
                            flags: old_flags,
                            next: &quant_cont,
                        };
                        if self.match_nodes(branch, &restore, pos, state) {
                            self.flags = old_flags;
                            return true;
                        }
                        self.flags = *flags;
                    } else {
                        let saved_match_end = state.match_end;
                        let branch_ok = self.match_nodes(branch, &Cont::Empty, pos, state);
                        if branch_ok {
                            let new_pos = state.match_end;
                            self.flags = old_flags;
                            if new_pos > pos {
                                if self.match_quant(
                                    mode,
                                    atom,
                                    min,
                                    max,
                                    count + 1,
                                    rest,
                                    cont,
                                    new_pos,
                                    pos,
                                    state,
                                ) {
                                    return true;
                                }
                            } else if (count + 1) < min {
                                if self.match_quant(
                                    mode,
                                    atom,
                                    min,
                                    max,
                                    count + 1,
                                    rest,
                                    cont,
                                    pos,
                                    pos,
                                    state,
                                ) {
                                    return true;
                                }
                            } else if try_rest_zw(count) && self.match_nodes(rest, cont, pos, state)
                            {
                                return true;
                            }
                            state.match_end = saved_match_end;
                            self.flags = *flags;
                        } else {
                            state.match_end = saved_match_end;
                        }
                    }
                }
                self.flags = old_flags;
                false
            }
            Node::LinebreakMatcher => {
                if pos < self.text_len() {
                    if self.input.at(pos) == 0x0d
                        && pos + 1 < self.text_len()
                        && self.input.at(pos + 1) == 0x0a
                    {
                        return self.match_quant(
                            mode,
                            atom,
                            min,
                            max,
                            count + 1,
                            rest,
                            cont,
                            pos + 2,
                            pos,
                            state,
                        );
                    }
                    if is_linebreak(self.input.at(pos)) {
                        return self.match_quant(
                            mode,
                            atom,
                            min,
                            max,
                            count + 1,
                            rest,
                            cont,
                            self.input.next(pos),
                            pos,
                            state,
                        );
                    }
                }
                false
            }
            Node::Backreference(idx) => {
                if let Some(Some((start, end))) = state.captures.get(*idx).cloned() {
                    let cap_len = end - start;
                    let Some(p) = self.backref_end(start, end, pos) else {
                        return false;
                    };
                    if cap_len == 0 {
                        if (count + 1) < min {
                            self.match_quant(
                                mode,
                                atom,
                                min,
                                max,
                                count + 1,
                                rest,
                                cont,
                                p,
                                pos,
                                state,
                            )
                        } else if try_rest_zw(count) {
                            self.match_nodes(rest, cont, p, state)
                        } else {
                            false
                        }
                    } else {
                        self.match_quant(mode, atom, min, max, count + 1, rest, cont, p, pos, state)
                    }
                } else {
                    false
                }
            }
            _ => {
                if self.match_nodes(core::slice::from_ref(atom), &Cont::Empty, pos, state) {
                    let new_pos = state.match_end;
                    if new_pos > pos {
                        self.match_quant(
                            mode,
                            atom,
                            min,
                            max,
                            count + 1,
                            rest,
                            cont,
                            new_pos,
                            pos,
                            state,
                        )
                    } else if (count + 1) < min {
                        self.match_quant(
                            mode,
                            atom,
                            min,
                            max,
                            count + 1,
                            rest,
                            cont,
                            pos,
                            pos,
                            state,
                        )
                    } else if try_rest_zw(count) {
                        self.match_nodes(rest, cont, pos, state)
                    } else {
                        false
                    }
                } else {
                    false
                }
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn match_reluctant(
        &mut self,
        atom: &Node,
        min: u32,
        max: u32,
        count: u32,
        rest: &[Node],
        cont: &Cont<'_>,
        pos: usize,
        iter_start: usize,
        state: &mut State,
    ) -> bool {
        if !self.step() {
            return false;
        }

        if count >= min && self.match_nodes(rest, cont, pos, state) {
            if count > 0
                && let Node::Group {
                    index: Some(idx),
                    inner,
                    ..
                } = atom
            {
                let is_ques = min == 0 && max == 1;
                let chain_based = is_ques || !is_deterministic_body(inner);
                if !chain_based {
                    state.captures[*idx] = Some((iter_start, pos));
                }
            }
            return true;
        }

        if count < max {
            self.try_match_atom_mode(
                Mode::Reluctant,
                atom,
                min,
                max,
                count,
                rest,
                cont,
                pos,
                state,
            )
        } else {
            false
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn match_possessive(
        &mut self,
        atom: &Node,
        min: u32,
        max: u32,
        rest: &[Node],
        cont: &Cont<'_>,
        pos: usize,
        state: &mut State,
    ) -> bool {
        let mut current_pos = pos;
        let mut count = 0u32;

        while count < min {
            if !self.match_nodes(
                core::slice::from_ref(atom),
                &Cont::Empty,
                current_pos,
                state,
            ) {
                return false;
            }
            current_pos = state.match_end;
            count += 1;
        }

        while count < max {
            if !self.match_nodes(
                core::slice::from_ref(atom),
                &Cont::Empty,
                current_pos,
                state,
            ) {
                break;
            }
            let new_pos = state.match_end;
            if new_pos == current_pos {
                break;
            }
            current_pos = new_pos;
            count += 1;
        }

        self.match_nodes(rest, cont, current_pos, state)
    }

    fn is_lt(&self, c: u32) -> bool {
        if self.flags.unix_lines {
            c == 0x0a
        } else {
            is_line_terminator(c)
        }
    }

    fn is_after_line_terminator(&self, pos: usize) -> bool {
        // With anchoring bounds (default), positions at/before text_start
        // have no "preceding char" for line-terminator purposes.
        if pos <= self.text_start {
            return false;
        }
        let prev = self.input.at(pos - 1);
        if self.flags.unix_lines {
            return prev == 0x0a;
        }
        if prev == 0x0a {
            true
        } else if prev == 0x0d {
            pos >= self.text_len() || self.input.at(pos) != 0x0a
        } else {
            is_line_terminator(prev)
        }
    }

    fn check_anchor(&mut self, kind: AnchorKind, pos: usize) -> bool {
        match kind {
            AnchorKind::StartOfLine => {
                if self.flags.multiline {
                    // Java/Perl quirk: in multiline mode, ^ never matches at end of input
                    // (even after a trailing line terminator). OpenJDK's Caret has an
                    // explicit `if (i == endIndex) return false;` for the same reason.
                    if pos == self.text_len() {
                        return false;
                    }
                    pos == self.text_start || self.is_after_line_terminator(pos)
                } else {
                    pos == self.text_start
                }
            }
            AnchorKind::EndOfLine => {
                if self.flags.multiline {
                    if pos == self.text_len() {
                        return true;
                    }
                    if pos < self.text_len() && self.is_lt(self.input.at(pos)) {
                        if !self.flags.unix_lines
                            && self.input.at(pos) == 0x0a
                            && pos > 0
                            && self.input.at(pos - 1) == 0x0d
                        {
                            return false;
                        }
                        return true;
                    }
                    false
                } else {
                    self.check_end_of_line(pos)
                }
            }
            AnchorKind::StartOfInput => pos == self.text_start,
            AnchorKind::EndOfInput => pos == self.text_len(),
            AnchorKind::EndOfInputBeforeFinalNewline => {
                if pos == self.text_len() {
                    return true;
                }
                self.check_before_final_newline(pos)
            }
            AnchorKind::WordBoundary => {
                let before = self.word_char_before(pos);
                let after = self.word_char_after(pos);
                before != after
            }
            AnchorKind::NonWordBoundary => {
                let before = self.word_char_before(pos);
                let after = self.word_char_after(pos);
                before == after
            }
            AnchorKind::PreviousMatchEnd => pos == self.search_start,
        }
    }

    /// Check non-multiline $ and \Z: before final newline or at end.
    fn check_before_final_newline(&self, pos: usize) -> bool {
        let len = self.text_len();
        if len == 0 {
            return false;
        }
        if self.flags.unix_lines {
            return pos == len - 1 && self.input.at(pos) == 0x0a;
        }
        // \r\n at end: match before the \r (pos == len-2)
        if pos + 2 == len && self.input.at(pos) == 0x0d && self.input.at(pos + 1) == 0x0a {
            return true;
        }
        // Single line terminator at end (pos == len-1), but not the \n of a \r\n pair
        pos + 1 == len
            && matches!(self.input.at(pos), 0x0a | 0x0d | 0x85 | 0x2028 | 0x2029)
            && !(self.input.at(pos) == 0x0a && pos > 0 && self.input.at(pos - 1) == 0x0d)
    }

    /// Check if the character before pos is a word character,
    /// treating combining marks as inheriting word-status from preceding char.
    fn has_base_character(&mut self, mut pos: usize) -> bool {
        loop {
            if !self.step() {
                return false;
            }
            let cp = self.input.at(pos);
            if is_letter_or_digit(cp) {
                return true;
            }
            if !is_nonspacing_mark(cp) || pos == self.text_start {
                return false;
            }
            // Pattern.hasBaseCharacter deliberately tests each UTF-16 unit.
            pos -= 1;
        }
    }
    fn word_char_before(&mut self, pos: usize) -> bool {
        if pos <= self.text_start {
            return false;
        }
        let cp = self.input.at(self.input.previous(pos));
        is_word_char(cp, self.flags.unicode_class)
            || (is_nonspacing_mark(cp) && self.has_base_character(pos - 1))
    }
    fn word_char_after(&mut self, pos: usize) -> bool {
        if pos >= self.text_len() {
            return false;
        }
        let cp = self.input.at(pos);
        is_word_char(cp, self.flags.unicode_class)
            || (is_nonspacing_mark(cp) && self.has_base_character(pos))
    }

    fn check_end_of_line(&self, pos: usize) -> bool {
        if pos == self.text_len() {
            return true;
        }
        self.check_before_final_newline(pos)
    }

    fn check_lookbehind(
        &mut self,
        inner: &Pattern,
        pos: usize,
        state: &mut State,
        supplementary: bool,
    ) -> bool {
        // Shares the caller's State directly instead of using a temp clone.
        // This faithfully replicates OpenJDK: when the inner pattern matches
        // (setting captures), those captures persist even if the surrounding
        // assertion is negative — `(?<!(a|bb))c` on "ac" leaves group 1 = "a"
        // from the inner success that flipped the outer to fail.
        let pos_check = Cont::PositionCheck {
            target: pos,
            next: &Cont::Empty,
        };
        let body_min = pattern_min_length(inner);
        if supplementary {
            let Some(high) = self.input.retreat(pos, body_min, self.text_start) else {
                return false;
            };
            let max = pattern_java_max(inner);
            if max < 0 {
                self.error = Some(MatchError::Unsupported(
                    "overflowed supplementary lookbehind bound",
                ));
                return false;
            }
            let low = self
                .input
                .retreat(pos, max as usize, self.text_start)
                .unwrap_or(self.text_start);
            let mut start = high;
            loop {
                if start < low {
                    break;
                }
                if self.match_pattern_cont(inner, &[], &pos_check, start, state) {
                    return true;
                }
                if self.error.is_some() || start == low {
                    break;
                }
                start = self.input.previous(start);
            }
            return false;
        }
        if body_min > pos {
            return false;
        }
        let start_high = pos - body_min;
        // Compute `rmax` using Java's i32 wrapping arithmetic so we replicate
        // `Behind.match`/`NotBehind.match`'s iteration range exactly:
        //   from = max(i - rmax, startIndex)
        //   j ∈ [from, i - rmin]
        // For overflowed (negative) `rmax`, `i - rmax` (signed) can be large-
        // positive for small `i` — `j >= from` is FALSE and body iteration is
        // skipped (which makes negative lookbehind succeed and positive fail).
        // For `i` past the threshold, the subtraction itself overflows back
        // and `from` clamps to startIndex.
        let rmax = pattern_java_max(inner);
        let pos_i32 = pos as i32;
        let from_i32 = pos_i32.wrapping_sub(rmax);
        let start_low = if from_i32 < 0 {
            self.text_start
        } else if from_i32 as usize > start_high {
            return false; // Java's loop body never runs
        } else {
            (from_i32 as usize).max(self.text_start)
        };
        if start_low > start_high {
            return false;
        }
        for start in (start_low..=start_high).rev() {
            if self.error.is_some() {
                break;
            }
            if self.match_pattern_cont(inner, &[], &pos_check, start, state) {
                return true;
            }
        }
        false
    }

    pub fn match_char_class(&mut self, cc: &CharClass, ch: u32) -> bool {
        let matched = self.match_char_class_items(&cc.items, ch);
        if self.error.is_some() {
            return false;
        }
        if cc.negated { !matched } else { matched }
    }

    fn match_char_class_items(&mut self, items: &[CharClassItem], ch: u32) -> bool {
        enum Work<'a> {
            Any(&'a [CharClassItem]),
            Item(&'a CharClassItem),
            OrRest(&'a [CharClassItem]),
            AndRight(&'a [CharClassItem]),
            Negate,
        }
        let mut pending = vec![Work::Any(items)];
        let mut matched = false;
        while let Some(work) = pending.pop() {
            if !self.step_class_work() {
                return false;
            }
            match work {
                Work::Any(items) => {
                    if let Some((first, rest)) = items.split_first() {
                        pending.push(Work::OrRest(rest));
                        pending.push(Work::Item(first));
                    } else {
                        matched = false;
                    }
                }
                Work::OrRest(rest) => {
                    if !matched {
                        pending.push(Work::Any(rest));
                    }
                }
                Work::AndRight(right) => {
                    if matched {
                        pending.push(Work::Any(right));
                    }
                }
                Work::Negate => matched = !matched,
                Work::Item(item) => match item {
                    CharClassItem::Single(c) => {
                        matched = if self.flags.case_insensitive {
                            chars_eq_ci(ch, *c, self.flags.unicode_case)
                        } else {
                            ch == *c
                        };
                    }
                    CharClassItem::Range(start, end) => {
                        matched = self.match_char_range(ch, *start, *end);
                    }
                    CharClassItem::Predefined(pc) => {
                        matched = match_predefined_class(*pc, ch, self.flags.unicode_class);
                    }
                    CharClassItem::UnicodeProperty {
                        compiled, negated, ..
                    } => {
                        matched = match_compiled_property(*compiled, ch) != *negated;
                    }
                    CharClassItem::Nested(nested) => {
                        if nested.negated {
                            pending.push(Work::Negate);
                        }
                        pending.push(Work::Any(&nested.items));
                    }
                    CharClassItem::Intersection(left, right) => {
                        pending.push(Work::AndRight(right));
                        pending.push(Work::Any(left));
                    }
                },
            }
        }
        matched
    }

    fn match_char_range(&self, ch: u32, start: u32, end: u32) -> bool {
        if ch >= start && ch <= end {
            return true;
        }
        if self.flags.case_insensitive {
            // Java treats `[start-end]` under CASE_INSENSITIVE as the set of input
            // characters whose own case variant lands inside the (unmodified) range.
            // The previous implementation folded the range endpoints as well, which
            // shrinks ASCII letter ranges (e.g. `[1-c]` becomes `[1-C]` after
            // uppercasing) and wrongly excludes characters like 'g' whose uppercase
            // 'G' (0x47) is actually inside the original range 0x31..0x63.
            if self.flags.unicode_case {
                if let Some(u) = single_char_uppercase(ch)
                    && u >= start
                    && u <= end
                {
                    return true;
                }
                if let Some(l) = single_char_lowercase(simple_upper(ch))
                    && l >= start
                    && l <= end
                {
                    return true;
                }
            } else {
                let u = ch.to_ascii_uppercase();
                let l = ch.to_ascii_lowercase();
                if u >= start && u <= end {
                    return true;
                }
                if l >= start && l <= end {
                    return true;
                }
            }
            false
        } else {
            ch >= start && ch <= end
        }
    }
}
