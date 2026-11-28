//! Clean-room `Locale.ROOT` lowercase implementation using official Unicode 15.0.0
//! UCD metadata (`crates/pkl-runtime/src/java_regex/metadata-runs.bin`, `Unicode-3.0`)
//! and a clean-room rule-based word-boundary state machine for Greek Final_Sigma (`U+03A3`).
use super::*;

#[derive(Clone, Copy, PartialEq, Eq)]
enum WordClass {
    Other,
    Ignore,
    Mark,
    KanaMark,
    Letter,
    Digit,
    MidLetter,
    Quote,
    Period,
    MidNum,
    PreNum,
    PostNum,
    Danda,
    Space,
    Cr,
    Lf,
    Kanji,
    Hiragana,
    Katakana,
    KanaBoth,
}

fn classify_word_char(cp: u32) -> WordClass {
    match cp {
        0x0009 => return WordClass::Space,
        0x000A | 0x000C | 0x2028 | 0x2029 => return WordClass::Lf,
        0x000D => return WordClass::Cr,
        0x0022 | 0x0027 => return WordClass::Quote,
        0x0023 => return WordClass::PreNum,
        0x0025 | 0x0026 | 0x00A2 | 0x066A | 0x2030 | 0x2031 => return WordClass::PostNum,
        0x002C | 0x066B => return WordClass::MidNum,
        0x002E => return WordClass::Period,
        0x00AD | 0x2027 => return WordClass::MidLetter,
        0x0964 | 0x0965 => return WordClass::Danda,
        0x3005 | 0x4E00..=0x9FA5 | 0xF900..=0xFA2D => return WordClass::Kanji,
        0x3041..=0x3094 | 0x309D..=0x309E => return WordClass::Hiragana,
        0x3099 | 0x309A => return WordClass::KanaMark,
        0x309B | 0x309C | 0x30FB | 0x30FC => return WordClass::KanaBoth,
        0x30A1..=0x30FA | 0x30FD..=0x30FE => return WordClass::Katakana,
        // Unassigned code points inside supplementary CJK/Khitan blocks that Java SE 21
        // `BreakIterator.getWordInstance(Locale.ROOT)` groups with adjacent ideographs.
        0x18CD6..=0x18CFF
        | 0x2A6E0..=0x2A6FF
        | 0x2B73A..=0x2B73F
        | 0x2B81E..=0x2B81F
        | 0x2CEA2..=0x2CEAF
        | 0x2FA1E..=0x2FFFF
        | 0x3134B..=0x3134F => return WordClass::Letter,
        // Supplementary Cf characters treated as non-word boundaries by Java SE 21.
        0x110BD | 0x110CD | 0x1343F | 0x1BCA3 | 0x1D17A | 0xE0001 | 0xE007F => {
            return WordClass::Other;
        }
        _ => {}
    }
    let Some(rec) = crate::java_regex::character_record(cp) else {
        return WordClass::Other;
    };
    match rec.category {
        // Lu, Ll, Lt, Lm, Lo, Mc
        1..=5 | 8 => WordClass::Letter,
        // Mn, Me
        6 | 7 => WordClass::Mark,
        // Nd, Nl, No
        9..=11 => WordClass::Digit,
        // Zs
        12 => WordClass::Space,
        // Zl, Zp
        13 | 14 => WordClass::Lf,
        // Cf (except 0x00AD and the 7 supplementary Cf boundaries handled above)
        16 => WordClass::Ignore,
        // Pd, Pc
        20 | 23 => WordClass::MidLetter,
        // Sc (except 0x00A2 handled above)
        26 => WordClass::PreNum,
        _ => WordClass::Other,
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum FwdState {
    Stop,
    Start,
    InLetter,
    InDigit,
    AfterPreNum,
    AfterLeadPeriod,
    AfterMidLetter,
    AfterMidDigit,
    AfterDanda,
    InKanji,
    InHiragana,
    InKatakana,
    InKanaBoth,
    InSpace,
    AfterCr,
    InOther,
    DoneNoMark,
}

impl FwdState {
    fn is_end(self) -> bool {
        matches!(
            self,
            Self::InLetter
                | Self::InDigit
                | Self::AfterPreNum
                | Self::AfterLeadPeriod
                | Self::AfterDanda
                | Self::InKanji
                | Self::InHiragana
                | Self::InKatakana
                | Self::InKanaBoth
                | Self::InSpace
                | Self::AfterCr
                | Self::InOther
                | Self::DoneNoMark
        )
    }

    fn is_lookahead(self) -> bool {
        matches!(self, Self::AfterMidLetter | Self::AfterMidDigit)
    }

    fn step(self, class: WordClass) -> Self {
        use FwdState::*;
        use WordClass as C;
        if class == C::Ignore {
            return self;
        }
        match self {
            Stop | DoneNoMark => Stop,
            Start => match class {
                C::Letter => InLetter,
                C::Digit => InDigit,
                C::PreNum => AfterPreNum,
                C::Period => AfterLeadPeriod,
                C::Kanji => InKanji,
                C::Hiragana => InHiragana,
                C::Katakana => InKatakana,
                C::KanaBoth => InKanaBoth,
                C::Space => InSpace,
                C::Cr => AfterCr,
                C::Lf => DoneNoMark,
                C::Other
                | C::Mark
                | C::KanaMark
                | C::MidLetter
                | C::Quote
                | C::MidNum
                | C::PostNum
                | C::Danda => InOther,
                C::Ignore => unreachable!(),
            },
            InLetter => match class {
                C::Letter | C::Mark | C::KanaMark => InLetter,
                C::Digit => InDigit,
                C::MidLetter | C::Quote | C::Period => AfterMidLetter,
                C::Danda => AfterDanda,
                _ => Stop,
            },
            InDigit => match class {
                C::Digit | C::Mark | C::KanaMark => InDigit,
                C::Letter => InLetter,
                C::MidNum | C::Quote | C::Period => AfterMidDigit,
                C::PostNum => InOther,
                _ => Stop,
            },
            AfterPreNum | AfterLeadPeriod | AfterDanda => match class {
                C::Digit => InDigit,
                C::Mark | C::KanaMark => InOther,
                _ => Stop,
            },
            AfterMidLetter => match class {
                C::Letter => InLetter,
                _ => Stop,
            },
            AfterMidDigit => match class {
                C::Digit => InDigit,
                _ => Stop,
            },
            InKanji => match class {
                C::Kanji | C::Mark | C::KanaMark => InKanji,
                _ => Stop,
            },
            InHiragana => match class {
                C::Hiragana | C::KanaBoth | C::Mark | C::KanaMark => InHiragana,
                _ => Stop,
            },
            InKatakana => match class {
                C::Katakana | C::KanaBoth | C::Mark | C::KanaMark => InKatakana,
                _ => Stop,
            },
            InKanaBoth => match class {
                C::KanaBoth | C::Mark | C::KanaMark => InKanaBoth,
                C::Hiragana => InHiragana,
                C::Katakana => InKatakana,
                _ => Stop,
            },
            InSpace => match class {
                C::Space | C::Mark | C::KanaMark => InSpace,
                _ => Stop,
            },
            AfterCr => match class {
                C::Lf => DoneNoMark,
                C::Mark | C::KanaMark => InOther,
                _ => Stop,
            },
            InOther => match class {
                C::Mark | C::KanaMark => InOther,
                _ => Stop,
            },
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum RevState {
    Stop,
    Start,
    InLetter,
    InDigit,
    AfterMidLetter,
    AfterMidDigit,
    AfterMidBoth,
    AfterDanda,
    AfterPostNum,
    InMark,
    InKanji,
    InHiragana,
    InKatakana,
    InKanaBoth,
    InSpace,
    AfterLf,
    Done,
}

impl RevState {
    fn step(self, class: WordClass) -> Self {
        use RevState::*;
        use WordClass as C;
        if class == C::Ignore {
            return self;
        }
        match self {
            Stop | Done => Stop,
            Start => match class {
                C::Letter => InLetter,
                C::Digit => InDigit,
                C::MidLetter => AfterMidLetter,
                C::MidNum => AfterMidDigit,
                C::Quote | C::Period => AfterMidBoth,
                C::Danda => AfterDanda,
                C::PostNum => AfterPostNum,
                C::Mark | C::KanaMark => InMark,
                C::Kanji => InKanji,
                C::Hiragana => InHiragana,
                C::Katakana => InKatakana,
                C::KanaBoth => InKanaBoth,
                C::Space => InSpace,
                C::Lf => AfterLf,
                C::PreNum | C::Cr | C::Other => Done,
                C::Ignore => unreachable!(),
            },
            InLetter => match class {
                C::Letter | C::Mark | C::KanaMark => InLetter,
                C::Digit => InDigit,
                C::MidLetter | C::Quote | C::Period => AfterMidLetter,
                _ => Stop,
            },
            InDigit => match class {
                C::Digit | C::Mark | C::KanaMark => InDigit,
                C::Letter => InLetter,
                C::MidNum | C::Quote | C::Period => AfterMidDigit,
                C::Danda => AfterDanda,
                C::PreNum => Done,
                _ => Stop,
            },
            AfterMidLetter => match class {
                C::Letter | C::Mark | C::KanaMark => InLetter,
                _ => Stop,
            },
            AfterMidDigit => match class {
                C::Digit | C::Mark | C::KanaMark => InDigit,
                _ => Stop,
            },
            AfterMidBoth => match class {
                C::Letter => InLetter,
                C::Digit => InDigit,
                C::Mark | C::KanaMark => InMark,
                _ => Stop,
            },
            AfterDanda => match class {
                C::Letter | C::Mark | C::KanaMark => InLetter,
                _ => Stop,
            },
            AfterPostNum => match class {
                C::Digit | C::Mark | C::KanaMark => InDigit,
                _ => Stop,
            },
            InMark => match class {
                C::Mark | C::KanaMark => InMark,
                C::Letter => InLetter,
                C::Digit => InDigit,
                C::Danda => AfterDanda,
                C::PostNum => AfterPostNum,
                C::Kanji => InKanji,
                C::Hiragana => InHiragana,
                C::Katakana => InKatakana,
                C::KanaBoth => InKanaBoth,
                C::Space => InSpace,
                C::MidLetter | C::Quote | C::Period | C::MidNum | C::PreNum | C::Cr | C::Other => {
                    Done
                }
                C::Lf => Stop,
                C::Ignore => unreachable!(),
            },
            InKanji => match class {
                C::Kanji | C::Mark | C::KanaMark => InKanji,
                _ => Stop,
            },
            InHiragana => match class {
                C::Hiragana | C::KanaBoth | C::Mark | C::KanaMark => InHiragana,
                _ => Stop,
            },
            InKatakana => match class {
                C::Katakana | C::KanaBoth | C::Mark | C::KanaMark => InKatakana,
                _ => Stop,
            },
            InKanaBoth => match class {
                C::KanaBoth | C::Mark | C::KanaMark => InKanaBoth,
                C::Hiragana => InHiragana,
                C::Katakana => InKatakana,
                _ => Stop,
            },
            InSpace => match class {
                C::Space | C::Mark | C::KanaMark => InSpace,
                _ => Stop,
            },
            AfterLf => match class {
                C::Cr => Done,
                _ => Stop,
            },
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
    units: &'a [u16],
    index: usize,
    cached: Option<usize>,
    clean_span: Option<(usize, usize)>,
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
        let mut state = FwdState::Start;
        let mut point = point_at(self.units, self.index).0;
        while point != 0xffff && state != FwdState::Stop {
            let prev = state;
            let class = classify_word_char(point);
            state = state.step(class);
            if state.is_lookahead() {
                // Save the last accepted end before the tentative mid-separator.
                if !prev.is_lookahead() {
                    lookahead = result;
                }
            } else if state.is_end() {
                result = self.next_index();
            }
            point = self.next();
        }
        if point == 0xffff && state.is_lookahead() && lookahead == self.units.len() {
            result = lookahead;
        }
        self.index = result;
        Some(result)
    }

    fn handle_previous(&mut self) -> usize {
        let mut state = RevState::Start;
        let mut class = WordClass::Other;
        let mut last_class = WordClass::Other;
        let mut point = point_at(self.units, self.index).0;
        while point != 0xffff && state != RevState::Stop {
            last_class = class;
            class = classify_word_char(point);
            state = state.step(class);
            point = self.previous();
        }
        if point != 0xffff {
            self.next();
            if last_class != WordClass::Ignore {
                self.next();
            }
        }
        self.index
    }

    fn boundary(&mut self, offset: usize) -> bool {
        if offset == 0 {
            return true;
        }
        if let Some((span_start, span_end)) = self.clean_span {
            if offset > span_start && offset < span_end {
                return false;
            }
            if offset == span_end {
                return true;
            }
        }
        let preceding = offset - 1;
        self.index = preceding;
        let mut prev_start;
        let result = if preceding == 0 {
            prev_start = Some(0);
            self.handle_next()
        } else {
            let mut result = if self.cached.is_none_or(|cached| cached >= preceding) {
                let start = self.handle_previous();
                prev_start = Some(start);
                Some(start)
            } else {
                self.index = self.cached.unwrap();
                prev_start = self.cached;
                self.cached
            };
            while result.is_some_and(|result| result <= preceding) {
                prev_start = result;
                result = self.handle_next();
            }
            result
        };
        self.cached = result;
        if let (Some(s), Some(e)) = (prev_start, result)
            && e > s
            && !self.units[s..e]
                .iter()
                .any(|&u| (0xd800..=0xdfff).contains(&u) || u == 0xffff)
        {
            self.clean_span = Some((s, e));
        }
        result == Some(offset)
    }
}

fn final_cased(boundary: &mut WordIterator<'_>, units: &[u16], index: usize) -> bool {
    // Preserve isBoundary's backwards synchronization, cache and UTF-16 offsets,
    // including its CharacterIterator.DONE collision with literal U+FFFF.
    boundary.index = 0;
    boundary.cached = None;
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
        || matches!(
            point,
            0x02b0..=0x02b8
                | 0x02c0..=0x02c1
                | 0x02e0..=0x02e4
                | 0x0345
                | 0x037a
                | 0x1d2c..=0x1d61
                | 0x2160..=0x217f
                | 0x24b6..=0x24e9
        )
}

struct SigmaWordCache<'a> {
    iter: WordIterator<'a>,
    word_start: usize,
    word_end: usize,
    first_cased: Option<usize>,
    last_cased: Option<usize>,
}

impl<'a> SigmaWordCache<'a> {
    fn new(units: &'a [u16]) -> Self {
        Self {
            iter: WordIterator {
                units,
                index: 0,
                cached: None,
                clean_span: None,
            },
            word_start: 0,
            word_end: 0,
            first_cased: None,
            last_cased: None,
        }
    }

    fn is_final_sigma(&mut self, index: usize) -> bool {
        while index >= self.word_end && self.iter.index < self.iter.units.len() {
            let start = self.iter.index;
            let Some(end) = self.iter.handle_next() else {
                break;
            };
            self.word_start = start;
            self.word_end = end;
            self.first_cased = None;
            self.last_cased = None;
            if index < end {
                let mut cursor = start;
                while cursor < end {
                    let (pt, w) = point_at(self.iter.units, cursor);
                    if cased(pt) {
                        if self.first_cased.is_none() {
                            self.first_cased = Some(cursor);
                        }
                        self.last_cased = Some(cursor);
                    }
                    cursor += w;
                }
            }
        }
        if index <= self.word_start || index >= self.word_end {
            return false;
        }
        self.first_cased.is_some_and(|first| first < index)
            && self.last_cased.is_none_or(|last| last <= index)
    }
}

pub(super) fn lowercase(text: &PklString) -> PklString {
    let units = text.encode_utf16().collect::<Vec<_>>();
    // StringUTF16.toLowerCase enters toLowerCaseEx for every surrogate,
    // including an unchanged pair or isolated unit, and returns a fresh String.
    let has_surrogate = units.iter().any(|unit| (0xd800..=0xdfff).contains(unit));
    let has_special = has_surrogate || units.contains(&0xffff);
    let has_sigma = units.contains(&0x3a3);
    let mut sigma_cache = (!has_special && has_sigma).then(|| SigmaWordCache::new(&units));
    let mut fallback_boundary = (has_special && has_sigma).then(|| WordIterator {
        units: &units,
        index: 0,
        cached: None,
        clean_span: None,
    });
    let mut output = Vec::with_capacity(units.len());
    let mut changed = has_surrogate;
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
            let is_final = if let Some(cache) = sigma_cache.as_mut() {
                cache.is_final_sigma(index)
            } else if let Some(boundary) = fallback_boundary.as_mut() {
                final_cased(boundary, &units, index)
            } else {
                false
            };
            if is_final { 0x3c2 } else { 0x3c3 }
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

#[cfg(test)]
mod tests {
    use super::lowercase;
    use crate::value::PklString;

    fn lower_utf16(units: &[u16]) -> Vec<u16> {
        lowercase(&PklString::from_utf16(units.to_vec()))
            .encode_utf16()
            .collect()
    }

    #[test]
    fn clean_room_final_sigma_edge_cases() {
        // Basic Greek word-final vs medial sigma
        assert_eq!(
            lowercase(&PklString::from("\u{03A3}")).as_str(),
            Some("\u{03C3}")
        );
        assert_eq!(
            lowercase(&PklString::from("A\u{03A3}")).as_str(),
            Some("a\u{03C2}")
        );
        assert_eq!(
            lowercase(&PklString::from("A\u{03A3}B")).as_str(),
            Some("a\u{03C3}b")
        );
        assert_eq!(
            lowercase(&PklString::from("\u{039F}\u{0394}\u{039F}\u{03A3}")).as_str(),
            Some("\u{03BF}\u{03B4}\u{03BF}\u{03C2}")
        );
        // Punctuation/format/combining boundaries matching Java Locale.ROOT
        assert_eq!(
            lowercase(&PklString::from("A'\u{03A3}")).as_str(),
            Some("a'\u{03C2}")
        );
        assert_eq!(
            lowercase(&PklString::from("A\u{03A3}'B")).as_str(),
            Some("a\u{03C3}'b")
        );
        assert_eq!(
            lowercase(&PklString::from("A\u{03A3}.B")).as_str(),
            Some("a\u{03C3}.b")
        );
        assert_eq!(
            lowercase(&PklString::from("A\u{03A3}-B")).as_str(),
            Some("a\u{03C3}-b")
        );
        assert_eq!(
            lowercase(&PklString::from("A\u{03A3}\u{0301}")).as_str(),
            Some("a\u{03C2}\u{0301}")
        );
        assert_eq!(
            lowercase(&PklString::from("A\u{03A3}\u{0301}B")).as_str(),
            Some("a\u{03C3}\u{0301}b")
        );
        assert_eq!(
            lowercase(&PklString::from("A\u{03A3}\u{0964}")).as_str(),
            Some("a\u{03C2}\u{0964}")
        );
        assert_eq!(
            lowercase(&PklString::from("A\u{03A3}\u{0964}B")).as_str(),
            Some("a\u{03C2}\u{0964}b")
        );
        // Turkish dotted capital I -> i + combining dot above
        assert_eq!(
            lowercase(&PklString::from("\u{0130}\u{03A3}")).as_str(),
            Some("i\u{0307}\u{03C2}")
        );
        // Raw lone surrogates and U+FFFF must preserve raw UTF-16 and match fallback iterator
        assert_eq!(
            lower_utf16(&[0x0041, 0xD800, 0x03A3]),
            vec![0x0061, 0xD800, 0x03C3]
        );
        assert_eq!(
            lower_utf16(&[0x0041, 0xFFFF, 0x03A3]),
            vec![0x0061, 0xFFFF, 0x03C3]
        );
    }

    #[test]
    #[ignore = "exhaustive 8.9M-case differential check against /tmp/sigma_oracle.bin from Temurin 21"]
    fn exhaustive_temurin21_oracle_parity() {
        let oracle = std::fs::read("/tmp/sigma_oracle.bin").expect("read /tmp/sigma_oracle.bin");
        assert_eq!(oracle.len(), 0x11_0000);
        let mut mismatches = 0usize;
        let mut cp_buf = [0u16; 2];
        let mut input = Vec::with_capacity(8);

        let check_final = |prefix: &[u16], suffix: &[u16], buf: &mut Vec<u16>| -> bool {
            buf.clear();
            buf.extend_from_slice(prefix);
            buf.push(0x03A3);
            buf.extend_from_slice(suffix);
            let out = lower_utf16(buf);
            let mut sigma_pos = prefix.len();
            for &u in prefix {
                if u == 0x0130 {
                    sigma_pos += 1;
                }
            }
            out[sigma_pos] == 0x03C2
        };

        for (cp, &expected_mask) in oracle.iter().enumerate() {
            let cp = cp as u32;
            let s: &[u16] = if cp <= 0xFFFF {
                cp_buf[0] = cp as u16;
                &cp_buf[..1]
            } else {
                let c = char::from_u32(cp).unwrap();
                c.encode_utf16(&mut cp_buf)
            };
            let mut actual_mask = 0u8;
            let mut p = Vec::with_capacity(4);
            let mut sf = Vec::with_capacity(4);

            // 0: "A" + s + Σ
            p.clear();
            p.push(0x0041);
            p.extend_from_slice(s);
            if check_final(&p, &[], &mut input) {
                actual_mask |= 1 << 0;
            }
            // 1: "A" + Σ + s + "B"
            sf.clear();
            sf.extend_from_slice(s);
            sf.push(0x0042);
            if check_final(&[0x0041], &sf, &mut input) {
                actual_mask |= 1 << 1;
            }
            // 2: "A" + Σ + s
            if check_final(&[0x0041], s, &mut input) {
                actual_mask |= 1 << 2;
            }
            // 3: s + Σ
            if check_final(s, &[], &mut input) {
                actual_mask |= 1 << 3;
            }
            // 4: "A0" + s + Σ
            p.clear();
            p.extend_from_slice(&[0x0041, 0x0030]);
            p.extend_from_slice(s);
            if check_final(&p, &[], &mut input) {
                actual_mask |= 1 << 4;
            }
            // 5: "A" + s + "0" + Σ
            p.clear();
            p.push(0x0041);
            p.extend_from_slice(s);
            p.push(0x0030);
            if check_final(&p, &[], &mut input) {
                actual_mask |= 1 << 5;
            }
            // 6: "A'" + s + Σ
            p.clear();
            p.extend_from_slice(&[0x0041, 0x0027]);
            p.extend_from_slice(s);
            if check_final(&p, &[], &mut input) {
                actual_mask |= 1 << 6;
            }
            // 7: "A" + Σ + s + "0"
            sf.clear();
            sf.extend_from_slice(s);
            sf.push(0x0030);
            if check_final(&[0x0041], &sf, &mut input) {
                actual_mask |= 1 << 7;
            }

            if actual_mask != expected_mask {
                if mismatches < 30 {
                    eprintln!(
                        "MISMATCH cp=U+{cp:04X}: actual={actual_mask:08b} expected={expected_mask:08b}"
                    );
                }
                mismatches += 1;
            }
        }

        let multi =
            std::fs::read_to_string("/tmp/sigma_multi.txt").expect("read /tmp/sigma_multi.txt");
        let mut multi_mismatches = 0usize;
        for (idx, line) in multi.lines().enumerate() {
            let mut parts = line.split_whitespace();
            let in_hex = parts.next().unwrap();
            let exp_hex = parts.next().unwrap();
            let in_units: Vec<u16> = in_hex
                .split(',')
                .map(|h| u16::from_str_radix(h, 16).unwrap())
                .collect();
            let exp_units: Vec<u16> = exp_hex
                .split(',')
                .map(|h| u16::from_str_radix(h, 16).unwrap())
                .collect();
            let got = lower_utf16(&in_units);
            if got != exp_units {
                eprintln!("MULTI MISMATCH #{idx}: in={in_hex} got={got:04x?} exp={exp_units:04x?}");
                multi_mismatches += 1;
            }
        }
        assert_eq!(mismatches, 0, "single-cp oracle mismatches: {mismatches}");
        assert_eq!(
            multi_mismatches, 0,
            "multi-char oracle mismatches: {multi_mismatches}"
        );
    }
}
