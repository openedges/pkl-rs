//! Version-pinned Unicode 15.0.0 character classification generated from
//! official UCD 15.0.0 (`UNICODE-15.0.0-PROVENANCE.md`). No host stdlib drift.
use super::{character_lookup::character_record, types::PredefinedClass};

pub trait CodePointExt {
    fn is_ascii_digit(&self) -> bool;
    fn is_ascii_hexdigit(&self) -> bool;
    fn is_alphanumeric(&self) -> bool;
    fn to_ascii_uppercase(self) -> Self;
    fn to_ascii_lowercase(self) -> Self;
}
impl CodePointExt for u32 {
    fn is_ascii_digit(&self) -> bool {
        (0x30..=0x39).contains(self)
    }
    fn is_ascii_hexdigit(&self) -> bool {
        self.is_ascii_digit() || (0x41..=0x46).contains(self) || (0x61..=0x66).contains(self)
    }
    fn is_alphanumeric(&self) -> bool {
        self.is_ascii_digit() || (0x41..=0x5a).contains(self) || (0x61..=0x7a).contains(self)
    }
    fn to_ascii_uppercase(self) -> Self {
        if (0x61..=0x7a).contains(&self) {
            self - 32
        } else {
            self
        }
    }
    fn to_ascii_lowercase(self) -> Self {
        if (0x41..=0x5a).contains(&self) {
            self + 32
        } else {
            self
        }
    }
}
pub fn display_codepoint(cp: u32) -> String {
    match core::char::from_u32(cp) {
        Some(c) => c.to_string(),
        None => format!("\\u{{{cp:04x}}}"),
    }
}
fn category(cp: u32) -> u8 {
    character_record(cp).map_or(0, |r| r.category)
}
fn bit(cp: u32, bit: u32) -> bool {
    character_record(cp).is_some_and(|r| r.predicate_bits & (1 << bit) != 0)
}
pub fn simple_upper(cp: u32) -> u32 {
    character_record(cp).map_or(cp, |r| r.upper)
}
pub fn simple_lower(cp: u32) -> u32 {
    character_record(cp).map_or(cp, |r| r.lower)
}
pub fn chars_eq_ci(a: u32, b: u32, unicode: bool) -> bool {
    a == b
        || if unicode {
            simple_lower(simple_upper(a)) == simple_lower(simple_upper(b))
        } else {
            a.to_ascii_lowercase() == b.to_ascii_lowercase()
        }
}
pub fn is_line_terminator(cp: u32) -> bool {
    matches!(cp, 0x0a | 0x0d | 0x85 | 0x2028 | 0x2029)
}
pub fn is_linebreak(cp: u32) -> bool {
    is_line_terminator(cp) || matches!(cp, 0x0b | 0x0c)
}
fn unicode_space(cp: u32) -> bool {
    matches!(category(cp), 12..=14) || matches!(cp, 9..=13 | 0x85)
}
pub fn is_word_char(cp: u32, unicode: bool) -> bool {
    if unicode {
        bit(cp, 0) || matches!(category(cp), 6..=9 | 23) || matches!(cp, 0x200c | 0x200d)
    } else {
        cp.is_alphanumeric() || cp == 0x5f
    }
}
pub fn match_predefined_class(kind: PredefinedClass, cp: u32, unicode: bool) -> bool {
    use PredefinedClass::*;
    let digit = if unicode {
        bit(cp, 2)
    } else {
        cp.is_ascii_digit()
    };
    let space = if unicode {
        unicode_space(cp)
    } else {
        matches!(cp, 9..=13 | 0x20)
    };
    match kind {
        Digit => digit,
        NonDigit => !digit,
        Word => is_word_char(cp, unicode),
        NonWord => !is_word_char(cp, unicode),
        Whitespace => space,
        NonWhitespace => !space,
        HorizWhitespace => matches!(
            cp,
            9 | 0x20 | 0xa0 | 0x1680 | 0x180e | 0x2000..=0x200a | 0x202f | 0x205f | 0x3000
        ),
        NonHorizWhitespace => !match_predefined_class(HorizWhitespace, cp, unicode),
        VertWhitespace => matches!(cp, 10..=13 | 0x85 | 0x2028 | 0x2029),
        NonVertWhitespace => !match_predefined_class(VertWhitespace, cp, unicode),
    }
}
fn category_mask(name: &str, ci: bool) -> Option<u32> {
    let mask = match name {
        "Cn" => 1,
        "Lu" if !ci => 1 << 1,
        "Ll" if !ci => 1 << 2,
        "Lt" if !ci => 1 << 3,
        "Lu" | "Ll" | "Lt" | "LC" => (1 << 1) | (1 << 2) | (1 << 3),
        "Lm" => 1 << 4,
        "Lo" => 1 << 5,
        "Mn" => 1 << 6,
        "Me" => 1 << 7,
        "Mc" => 1 << 8,
        "Nd" => 1 << 9,
        "Nl" => 1 << 10,
        "No" => 1 << 11,
        "Zs" => 1 << 12,
        "Zl" => 1 << 13,
        "Zp" => 1 << 14,
        "Cc" => 1 << 15,
        "Cf" => 1 << 16,
        "Co" => 1 << 18,
        "Cs" => 1 << 19,
        "Pd" => 1 << 20,
        "Ps" => 1 << 21,
        "Pe" => 1 << 22,
        "Pc" => 1 << 23,
        "Po" => 1 << 24,
        "Sm" => 1 << 25,
        "Sc" => 1 << 26,
        "Sk" => 1 << 27,
        "So" => 1 << 28,
        "Pi" => 1 << 29,
        "Pf" => 1 << 30,
        "L" => 0x3e,
        "M" => (1 << 6) | (1 << 7) | (1 << 8),
        "N" => (1 << 9) | (1 << 10) | (1 << 11),
        "Z" => (1 << 12) | (1 << 13) | (1 << 14),
        "C" => (1 << 0) | (1 << 15) | (1 << 16) | (1 << 18) | (1 << 19),
        "P" => (0x1f << 20) | (1 << 29) | (1 << 30),
        "S" => 0xf << 25,
        "LD" => 0x3e | (1 << 9),
        _ => return None,
    };
    Some(mask)
}
pub(super) fn block_name(id: u16) -> Option<&'static str> {
    for row in include_str!("character-dictionary.tsv").lines() {
        let mut fields = row.split('\t');
        if fields.next() == Some("block") && fields.next()?.parse::<u16>().ok() == Some(id) {
            return fields.next().filter(|name| *name != "null");
        }
    }
    None
}
fn dictionary_id(kind: &str, name: &str) -> Option<u16> {
    let name = super::character_names::uppercase_name(name)?;
    for row in include_str!("property-aliases.tsv").lines() {
        let mut fields = row.split('\t');
        if fields.next() == Some(kind) && fields.next() == Some(name.as_str()) {
            return fields.next()?.parse().ok();
        }
    }
    None
}
fn property_key(name: &str) -> Option<String> {
    let mut result = String::new();
    for cp in name.chars().map(u32::from) {
        // ROOT/ENGLISH full lowercase of I-with-dot is i + combining dot.
        // It cannot equal an ASCII property key; all other ASCII-producing
        // lowercase mappings are represented by the pinned Character table.
        if cp == 0x130 {
            return None;
        }
        let lower = simple_lower(cp);
        if lower >= 128 {
            return None;
        }
        result.push(lower as u8 as char);
    }
    Some(result)
}
fn posix_name(name: &str) -> Option<&'static str> {
    let name = super::character_names::uppercase_name(name)?;
    Some(match name.as_str() {
        "LOWER" => "Lower",
        "UPPER" => "Upper",
        "ASCII" => "ASCII",
        "ALPHA" => "Alpha",
        "DIGIT" => "Digit",
        "ALNUM" => "Alnum",
        "PUNCT" => "Punct",
        "GRAPH" => "Graph",
        "PRINT" => "Print",
        "BLANK" => "Blank",
        "CNTRL" => "Cntrl",
        "XDIGIT" => "XDigit",
        "SPACE" => "Space",
        _ => return None,
    })
}
fn java_bit(name: &str) -> Option<u32> {
    Some(match name {
        "javaAlphabetic" => 0,
        "javaDefined" => 1,
        "javaDigit" => 2,
        "javaISOControl" => 9,
        "javaIdentifierIgnorable" => 10,
        "javaIdeographic" => 11,
        "javaJavaIdentifierPart" => 12,
        "javaJavaIdentifierStart" => 13,
        "javaLetter" => 14,
        "javaLetterOrDigit" => 15,
        "javaLowerCase" => 16,
        "javaMirrored" => 17,
        "javaSpaceChar" => 18,
        "javaTitleCase" => 19,
        "javaUnicodeIdentifierPart" => 20,
        "javaUnicodeIdentifierStart" => 21,
        "javaUpperCase" => 22,
        "javaWhitespace" => 23,
        _ => return None,
    })
}
fn posix(name: &str, cp: u32, unicode: bool, ci: bool) -> Option<bool> {
    let lower = if unicode {
        bit(cp, 16)
    } else {
        (0x61..=0x7a).contains(&cp)
    };
    let upper = if unicode {
        bit(cp, 22)
    } else {
        (0x41..=0x5a).contains(&cp)
    };
    let alpha = if unicode {
        bit(cp, 0)
    } else {
        cp.is_alphanumeric() && !cp.is_ascii_digit()
    };
    let digit = if unicode {
        bit(cp, 2)
    } else {
        cp.is_ascii_digit()
    };
    let blank = if unicode {
        category(cp) == 12 || cp == 9
    } else {
        matches!(cp, 9 | 32)
    };
    let control = if unicode {
        category(cp) == 15
    } else {
        cp < 32 || cp == 127
    };
    let graph = if unicode {
        !matches!(category(cp), 0 | 12..=15 | 19)
    } else {
        (33..=126).contains(&cp)
    };
    Some(match name {
        "Lower" => lower || ci && (upper || unicode && bit(cp, 19)),
        "Upper" => upper || ci && (lower || unicode && bit(cp, 19)),
        "Alpha" => alpha,
        "Alnum" => alpha || digit,
        "Digit" => digit,
        "Blank" => blank,
        "Cntrl" => control,
        "Space" => {
            if unicode {
                unicode_space(cp)
            } else {
                matches!(cp, 9..=13 | 32)
            }
        }
        "Graph" => graph,
        "Print" => (graph || blank) && !control,
        "Punct" => {
            if unicode {
                category_mask("P", false).unwrap() & (1 << category(cp)) != 0
            } else {
                (33..=126).contains(&cp) && !cp.is_alphanumeric()
            }
        }
        "XDigit" => {
            cp.is_ascii_hexdigit()
                || unicode
                    && (digit || matches!(cp,0xff10..=0xff19|0xff21..=0xff26|0xff41..=0xff46))
        }
        _ => return None,
    })
}
fn unicode_binary(name: &str, cp: u32, ci: bool) -> Option<bool> {
    let norm = super::character_names::uppercase_name(name)?;
    Some(match norm.as_str() {
        "ALPHABETIC" => bit(cp, 0),
        "ASSIGNED" => category(cp) != 0,
        "CONTROL" => category(cp) == 15,
        "EMOJI" => bit(cp, 3),
        "EMOJI_COMPONENT" => bit(cp, 4),
        "EMOJI_MODIFIER" => bit(cp, 5),
        "EMOJI_MODIFIER_BASE" => bit(cp, 6),
        "EMOJI_PRESENTATION" => bit(cp, 7),
        "EXTENDED_PICTOGRAPHIC" => bit(cp, 8),
        "IDEOGRAPHIC" => bit(cp, 11),
        "LETTER" => bit(cp, 14),
        "LOWERCASE" => bit(cp, 16) || ci && (bit(cp, 19) || bit(cp, 22)),
        "UPPERCASE" => bit(cp, 22) || ci && (bit(cp, 16) || bit(cp, 19)),
        "TITLECASE" => bit(cp, 19) || ci && (bit(cp, 16) || bit(cp, 22)),
        "WHITESPACE" | "WHITE_SPACE" => unicode_space(cp),
        "WORD" => is_word_char(cp, true),
        "JOINCONTROL" | "JOIN_CONTROL" => matches!(cp, 0x200c | 0x200d),
        "NONCHARACTERCODEPOINT" | "NONCHARACTER_CODE_POINT" => {
            cp & 0xfffe == 0xfffe || (0xfdd0..=0xfdef).contains(&cp)
        }
        "HEXDIGIT" | "HEX_DIGIT" => posix("XDigit", cp, true, ci)?,
        "PUNCTUATION" => posix("Punct", cp, true, ci)?,
        "ALPHA" => posix("Alpha", cp, true, ci)?,
        "ALNUM" => posix("Alnum", cp, true, ci)?,
        "DIGIT" => posix("Digit", cp, true, ci)?,
        "SPACE" => posix("Space", cp, true, ci)?,
        "BLANK" => posix("Blank", cp, true, ci)?,
        "LOWER" => posix("Lower", cp, true, ci)?,
        "UPPER" => posix("Upper", cp, true, ci)?,
        "GRAPH" => posix("Graph", cp, true, ci)?,
        "PRINT" => posix("Print", cp, true, ci)?,
        "PUNCT" => posix("Punct", cp, true, ci)?,
        "CNTRL" => posix("Cntrl", cp, true, ci)?,
        "XDIGIT" => posix("XDigit", cp, true, ci)?,
        _ => return None,
    })
}
fn for_property(name: &str, cp: u32, ci: bool) -> Option<bool> {
    if let Some(mask) = category_mask(name, ci) {
        return Some(mask & (1 << category(cp)) != 0);
    }
    if let Some(b) = java_bit(name) {
        return Some(
            bit(cp, b)
                || (ci && matches!(b, 16 | 19 | 22) && (bit(cp, 16) || bit(cp, 19) || bit(cp, 22))),
        );
    }
    if let Some(v) = posix(name, cp, false, ci) {
        return Some(v);
    }
    match name {
        "ASCII" => Some(cp < 128),
        "L1" => Some(cp <= 255),
        "all" => Some(cp <= 0x10ffff),
        _ => None,
    }
}

fn property(name: &str, cp: u32, uc: bool, ci: bool) -> Option<bool> {
    if let Some((kind, value)) = name.split_once('=') {
        let kind = property_key(kind)?;
        let kind = kind.as_str();
        if matches!(kind, "sc" | "script") {
            let id = dictionary_id("script", value)?;
            return Some(character_record(cp)?.script_id == id);
        }
        if matches!(kind, "blk" | "block") {
            let id = dictionary_id("block", value)?;
            return Some(character_record(cp)?.block_id == id);
        }
        if matches!(kind, "gc" | "general_category") {
            return for_property(value, cp, ci);
        }
        return None;
    }
    if let Some(value) = name.strip_prefix("In") {
        let id = dictionary_id("block", value)?;
        return Some(character_record(cp)?.block_id == id);
    }
    if let Some(value) = name.strip_prefix("Is") {
        if let Some(v) = unicode_binary(value, cp, ci) {
            return Some(v);
        }
        if let Some(value) = for_property(value, cp, ci) {
            return Some(value);
        }
        let id = dictionary_id("script", value)?;
        return Some(character_record(cp)?.script_id == id);
    }
    if uc && let Some(value) = posix_name(name).and_then(|canonical| posix(canonical, cp, true, ci))
    {
        return Some(value);
    }
    for_property(name, cp, ci)
}

pub fn is_valid_unicode_property(name: &str, unicode_class: bool) -> bool {
    property(name, 0, unicode_class, false).is_some()
}
pub fn match_unicode_property_ext(name: &str, cp: u32, uc: bool, ci: bool) -> bool {
    // Parser admits only supported names. A missing internal branch is a bug,
    // not permission to report a successful negative predicate.
    property(name, cp, uc, ci).expect("property validated during compilation")
}

pub fn is_nonspacing_mark(cp: u32) -> bool {
    category(cp) == 6
}
pub fn is_letter_or_digit(cp: u32) -> bool {
    bit(cp, 15)
}

pub fn property_is_supplementary(name: &str, unicode: bool) -> bool {
    // These are the BmpCharPredicate cases in CharPredicates.forProperty.
    if matches!(name, "ASCII" | "L1") {
        return false;
    }
    if !unicode
        && matches!(
            name,
            "Alnum"
                | "Alpha"
                | "Blank"
                | "Cntrl"
                | "Digit"
                | "Graph"
                | "Lower"
                | "Print"
                | "Punct"
                | "Space"
                | "Upper"
                | "XDigit"
        )
    {
        return false;
    }
    true
}
