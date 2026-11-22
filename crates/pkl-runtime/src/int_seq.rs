//! Pinned VmIntSeq arithmetic, including Java's distinct generator length.
use crate::value_identity::Token;
use std::{fmt, sync::Arc};

#[derive(Clone, Debug)]
pub struct IntSeq(Arc<Fields>);
#[derive(Debug)]
struct Fields {
    start: i64,
    end: i64,
    step: i64,
    last: i64,
    reference: Token,
}
impl IntSeq {
    pub fn new(start: i64, end: i64, step: i64) -> Self {
        assert_ne!(step, 0);
        fn modulo(a: i64, b: i64) -> i64 {
            let value = a.wrapping_rem(b);
            if value >= 0 {
                value
            } else {
                value.wrapping_add(b)
            }
        }
        fn diff(a: i64, b: i64, c: i64) -> i64 {
            modulo(modulo(a, c).wrapping_sub(modulo(b, c)), c)
        }
        let last = if step > 0 {
            if start >= end {
                end
            } else {
                end.wrapping_sub(diff(end, start, step))
            }
        } else if start <= end {
            end
        } else {
            end.wrapping_add(diff(start, end, step.wrapping_neg()))
        };
        Self(Arc::new(Fields {
            start,
            end,
            step,
            last,
            reference: Token::fresh(),
        }))
    }
    pub fn start(&self) -> i64 {
        self.0.start
    }
    pub fn end(&self) -> i64 {
        self.0.end
    }
    pub fn step(&self) -> i64 {
        self.0.step
    }
    pub fn is_empty(&self) -> bool {
        if self.step() > 0 {
            self.start() > self.0.last
        } else {
            self.start() < self.0.last
        }
    }
    pub(crate) fn reference(&self) -> Token {
        self.0.reference
    }
    pub(crate) fn key(&self) -> Option<[i64; 3]> {
        (!self.is_empty()).then_some([self.start(), self.0.last, self.step()])
    }
    pub(crate) fn java_hash(&self) -> i32 {
        self.key().map_or(1, |parts| {
            parts.into_iter().fold(1i32, |hash, part| {
                hash.wrapping_mul(31)
                    .wrapping_add((part as u64 ^ (part as u64 >> 32)) as i32)
            })
        })
    }
    fn generator_length(&self) -> i64 {
        if self.is_empty() {
            0
        } else {
            self.end()
                .wrapping_sub(self.start())
                .wrapping_div(self.step())
                .wrapping_abs()
                .wrapping_add(1)
        }
    }
    pub(crate) fn len_hint(&self) -> usize {
        if self.is_empty() {
            return 0;
        }
        let span = self.0.last.abs_diff(self.start());
        let step = self.step().unsigned_abs();
        let count = (span / step).saturating_add(1);
        count.min(1 << 20) as usize
    }

    pub(crate) fn cursor(&self, generator: bool) -> Cursor {
        Cursor {
            next: self.start(),
            last: self.0.last,
            step: self.step(),
            index: 0,
            length: generator.then(|| self.generator_length()),
            done: self.is_empty(),
        }
    }
}
impl PartialEq for IntSeq {
    fn eq(&self, other: &Self) -> bool {
        self.key() == other.key()
    }
}
impl Eq for IntSeq {}
impl fmt::Display for IntSeq {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "IntSeq({}, {})", self.start(), self.end())?;
        if self.step() != 1 {
            write!(f, ".step({})", self.step())?;
        }
        Ok(())
    }
}

#[doc(hidden)]
#[derive(Clone, Debug)]
pub struct Cursor {
    next: i64,
    last: i64,
    step: i64,
    index: i64,
    length: Option<i64>,
    done: bool,
}
impl Cursor {
    pub(crate) fn next(&mut self) -> Option<(i64, i64)> {
        if self.done || self.length.is_some_and(|length| self.index >= length) {
            return None;
        }
        let result = (self.index, self.next);
        self.index = self.index.wrapping_add(1);
        if self.length.is_none() && self.next == self.last {
            self.done = true;
        } else {
            self.next = self.next.wrapping_add(self.step);
        }
        Some(result)
    }
}
