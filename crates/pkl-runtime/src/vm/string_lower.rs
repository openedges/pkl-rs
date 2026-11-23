//! Locale.ROOT lowercase, using pinned JDK Character and word-break data.
use super::*;
const WORD: &[u8] = include_bytes!("string_word_break.bin");
fn u32_at(at: usize) -> usize {
    u32::from_be_bytes(WORD[at..at + 4].try_into().unwrap()) as usize
}
fn u16_at(at: usize) -> usize {
    u16::from_be_bytes(WORD[at..at + 2].try_into().unwrap()) as usize
}
struct WordTable {
    states: usize,
    backwards: usize,
    ends: usize,
    lookahead: usize,
    indices: usize,
    bmp: usize,
    supplementary: usize,
    supplementary_count: usize,
    categories: usize,
}
impl WordTable {
    fn new() -> Self {
        let states = 48;
        let end_count = u32_at(20);
        let ends = states + 2 * (u32_at(12) + u32_at(16));
        let lookahead = ends + end_count;
        let indices = lookahead + u32_at(24);
        let bmp = indices + 1024;
        let supplementary = bmp + u32_at(28);
        Self {
            states,
            backwards: states + 2 * u32_at(12),
            ends,
            lookahead,
            indices,
            bmp,
            supplementary,
            supplementary_count: u32_at(32),
            categories: u32_at(12) / end_count,
        }
    }
    fn category(&self, point: u32) -> u8 {
        if point < 0x10000 {
            let index = u16_at(self.indices + 2 * ((point as usize) >> 7));
            WORD[self.bmp + index + (point as usize & 127)]
        } else {
            let mut lo = 0;
            let mut hi = self.supplementary_count;
            while lo < hi {
                let mid = (lo + hi) / 2;
                if (u32_at(self.supplementary + 4 * mid) >> 8) <= point as usize {
                    lo = mid + 1;
                } else {
                    hi = mid;
                }
            }
            (u32_at(self.supplementary + 4 * (lo - 1)) & 255) as u8
        }
    }
    fn transition(&self, state: usize, category: u8, backwards: bool) -> usize {
        if category == 255 {
            state
        } else {
            u16_at(
                (if backwards {
                    self.backwards
                } else {
                    self.states
                }) + 2 * (state * self.categories + usize::from(category)),
            )
        }
    }
}
fn point_at(units: &[u16], index: usize) -> (u32, usize) {
    let Some(&first) = units.get(index) else {
        return (0xffff, 1);
    };
    if (0xd800..=0xdbff).contains(&first)
        && let Some(&second) = units.get(index + 1)
        && (0xdc00..=0xdfff).contains(&second)
    {
        (
            0x10000 + ((u32::from(first) - 0xd800) << 10) + u32::from(second) - 0xdc00,
            2,
        )
    } else {
        (u32::from(first), 1)
    }
}
fn point_before(units: &[u16], index: usize) -> (u32, usize) {
    let trailing = units[index - 1];
    if (0xdc00..=0xdfff).contains(&trailing)
        && index > 1
        && (0xd800..=0xdbff).contains(&units[index - 2])
    {
        point_at(units, index - 2)
    } else {
        (u32::from(trailing), 1)
    }
}
struct WordIterator<'a> {
    table: &'a WordTable,
    units: &'a [u16],
    index: usize,
    cached: Option<usize>,
}
impl WordIterator<'_> {
    fn next_index(&self) -> usize {
        (self.index + point_at(self.units, self.index).1).min(self.units.len())
    }
    fn next(&mut self) -> u32 {
        let index = self.next_index();
        if index == self.units.len() {
            0xffff
        } else {
            self.index = index;
            point_at(self.units, index).0
        }
    }
    fn previous(&mut self) -> u32 {
        if self.index == 0 {
            return 0xffff;
        }
        let (point, width) = point_before(self.units, self.index);
        self.index -= width;
        point
    }
    fn handle_next(&mut self) -> Option<usize> {
        if self.index == self.units.len() {
            return None;
        }
        let mut result = self.next_index();
        let mut lookahead = 0;
        let mut state = 1;
        let mut point = point_at(self.units, self.index).0;
        while point != 0xffff && state != 0 {
            state = self
                .table
                .transition(state, self.table.category(point), false);
            if WORD[self.table.lookahead + state] != 0 {
                if WORD[self.table.ends + state] != 0 {
                    result = lookahead;
                } else {
                    lookahead = self.next_index();
                }
            } else if WORD[self.table.ends + state] != 0 {
                result = self.next_index();
            }
            point = self.next();
        }
        if point == 0xffff && lookahead == self.units.len() {
            result = lookahead;
        }
        self.index = result;
        Some(result)
    }
    fn handle_previous(&mut self) -> usize {
        let mut state = 1;
        let mut category = 0;
        let mut last_category = 0;
        let mut point = point_at(self.units, self.index).0;
        while point != 0xffff && state != 0 {
            last_category = category;
            category = self.table.category(point);
            state = self.table.transition(state, category, true);
            point = self.previous();
        }
        if point != 0xffff {
            self.next();
            if last_category != 255 {
                self.next();
            }
        }
        self.index
    }
    fn boundary(&mut self, offset: usize) -> bool {
        if offset == 0 {
            return true;
        }
        let preceding = offset - 1;
        self.index = preceding;
        let result = if preceding == 0 {
            self.handle_next()
        } else {
            let mut result = if self.cached.is_none_or(|cached| cached >= preceding) {
                Some(self.handle_previous())
            } else {
                self.index = self.cached.unwrap();
                self.cached
            };
            while result.is_some_and(|result| result <= preceding) {
                result = self.handle_next();
            }
            result
        };
        self.cached = result;
        result == Some(offset)
    }
}
fn final_cased(table: &WordTable, units: &[u16], index: usize) -> bool {
    // Preserve isBoundary's backwards synchronization, cache and UTF-16 offsets,
    // including its CharacterIterator.DONE collision with literal U+FFFF.
    let mut boundary = WordIterator {
        table,
        units,
        index: 0,
        cached: None,
    };
    let mut cursor = index;
    while cursor > 0 && !boundary.boundary(cursor) {
        let (point, width) = point_before(units, cursor);
        if cased(point) {
            cursor = index + point_at(units, index).1;
            while cursor < units.len() && !boundary.boundary(cursor) {
                let (point, width) = point_at(units, cursor);
                if cased(point) {
                    return false;
                }
                cursor += width;
            }
            return true;
        }
        cursor -= width;
    }
    false
}

fn cased(point: u32) -> bool {
    crate::java_regex::character_record(point).is_some_and(|r| matches!(r.category, 1..=3))
        || matches!(point,0x02b0..=0x02b8|0x02c0..=0x02c1|0x02e0..=0x02e4|0x0345|0x037a|0x1d2c..=0x1d61|0x2160..=0x217f|0x24b6..=0x24e9)
}
pub(super) fn lowercase(text: &PklString) -> PklString {
    let units = text.encode_utf16().collect::<Vec<_>>();
    let table = WordTable::new();
    let mut output = Vec::with_capacity(units.len());
    // StringUTF16.toLowerCase enters toLowerCaseEx for every surrogate,
    // including an unchanged pair or isolated unit, and returns a fresh String.
    let mut changed = units.iter().any(|unit| (0xd800..=0xdfff).contains(unit));
    let mut index = 0;
    while index < units.len() {
        let (point, width) = point_at(&units, index);
        if point == 0x130 {
            output.extend([0x69, 0x307]);
            changed = true;
            index += width;
            continue;
        }
        let mapped = if point == 0x3a3 {
            if final_cased(&table, &units, index) {
                0x3c2
            } else {
                0x3c3
            }
        } else {
            crate::java_regex::character_record(point).map_or(point, |r| r.lower)
        };
        changed |= mapped != point;
        if let Some(character) = char::from_u32(mapped) {
            output.extend_from_slice(character.encode_utf16(&mut [0; 2]));
        } else {
            output.push(mapped as u16);
        }
        index += width;
    }
    if changed {
        PklString::from_utf16(output)
    } else {
        text.clone()
    }
}
