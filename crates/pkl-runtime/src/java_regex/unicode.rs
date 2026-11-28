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
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PosixKind {
    Lower,
    Upper,
    Alpha,
    Alnum,
    Digit,
    Blank,
    Cntrl,
    Space,
    Graph,
    Print,
    Punct,
    XDigit,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UnicodeBinaryKind {
    Alphabetic,
    Assigned,
    Control,
    Emoji,
    EmojiComponent,
    EmojiModifier,
    EmojiModifierBase,
    EmojiPresentation,
    ExtendedPictographic,
    Ideographic,
    Letter,
    Lowercase,
    Uppercase,
    Titlecase,
    Whitespace,
    Word,
    JoinControl,
    NoncharacterCodePoint,
    Posix(PosixKind),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CompiledProperty {
    Script(u16),
    Block(u16),
    CategoryMask(u32),
    JavaBit {
        bit: u32,
        ci: bool,
    },
    Posix {
        kind: PosixKind,
        unicode: bool,
        ci: bool,
    },
    UnicodeBinary {
        kind: UnicodeBinaryKind,
        ci: bool,
    },
    Ascii,
    Latin1,
    All,
}

fn posix_kind_by_canonical(name: &str) -> Option<PosixKind> {
    Some(match name {
        "Lower" => PosixKind::Lower,
        "Upper" => PosixKind::Upper,
        "Alpha" => PosixKind::Alpha,
        "Alnum" => PosixKind::Alnum,
        "Digit" => PosixKind::Digit,
        "Blank" => PosixKind::Blank,
        "Cntrl" => PosixKind::Cntrl,
        "Space" => PosixKind::Space,
        "Graph" => PosixKind::Graph,
        "Print" => PosixKind::Print,
        "Punct" => PosixKind::Punct,
        "XDigit" => PosixKind::XDigit,
        _ => return None,
    })
}

fn eval_posix(kind: PosixKind, cp: u32, unicode: bool, ci: bool) -> bool {
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
    match kind {
        PosixKind::Lower => lower || ci && (upper || unicode && bit(cp, 19)),
        PosixKind::Upper => upper || ci && (lower || unicode && bit(cp, 19)),
        PosixKind::Alpha => alpha,
        PosixKind::Alnum => alpha || digit,
        PosixKind::Digit => digit,
        PosixKind::Blank => blank,
        PosixKind::Cntrl => control,
        PosixKind::Space => {
            if unicode {
                unicode_space(cp)
            } else {
                matches!(cp, 9..=13 | 32)
            }
        }
        PosixKind::Graph => graph,
        PosixKind::Print => (graph || blank) && !control,
        PosixKind::Punct => {
            if unicode {
                category_mask("P", false).unwrap() & (1 << category(cp)) != 0
            } else {
                (33..=126).contains(&cp) && !cp.is_alphanumeric()
            }
        }
        PosixKind::XDigit => {
            cp.is_ascii_hexdigit()
                || unicode
                    && (digit || matches!(cp, 0xff10..=0xff19 | 0xff21..=0xff26 | 0xff41..=0xff46))
        }
    }
}

fn compile_unicode_binary(name: &str, ci: bool) -> Option<CompiledProperty> {
    let norm = super::character_names::uppercase_name(name)?;
    let kind = match norm.as_str() {
        "ALPHABETIC" => UnicodeBinaryKind::Alphabetic,
        "ASSIGNED" => UnicodeBinaryKind::Assigned,
        "CONTROL" => UnicodeBinaryKind::Control,
        "EMOJI" => UnicodeBinaryKind::Emoji,
        "EMOJI_COMPONENT" => UnicodeBinaryKind::EmojiComponent,
        "EMOJI_MODIFIER" => UnicodeBinaryKind::EmojiModifier,
        "EMOJI_MODIFIER_BASE" => UnicodeBinaryKind::EmojiModifierBase,
        "EMOJI_PRESENTATION" => UnicodeBinaryKind::EmojiPresentation,
        "EXTENDED_PICTOGRAPHIC" => UnicodeBinaryKind::ExtendedPictographic,
        "IDEOGRAPHIC" => UnicodeBinaryKind::Ideographic,
        "LETTER" => UnicodeBinaryKind::Letter,
        "LOWERCASE" => UnicodeBinaryKind::Lowercase,
        "UPPERCASE" => UnicodeBinaryKind::Uppercase,
        "TITLECASE" => UnicodeBinaryKind::Titlecase,
        "WHITESPACE" | "WHITE_SPACE" => UnicodeBinaryKind::Whitespace,
        "WORD" => UnicodeBinaryKind::Word,
        "JOINCONTROL" | "JOIN_CONTROL" => UnicodeBinaryKind::JoinControl,
        "NONCHARACTERCODEPOINT" | "NONCHARACTER_CODE_POINT" => {
            UnicodeBinaryKind::NoncharacterCodePoint
        }
        "HEXDIGIT" | "HEX_DIGIT" | "XDIGIT" => UnicodeBinaryKind::Posix(PosixKind::XDigit),
        "PUNCTUATION" | "PUNCT" => UnicodeBinaryKind::Posix(PosixKind::Punct),
        "ALPHA" => UnicodeBinaryKind::Posix(PosixKind::Alpha),
        "ALNUM" => UnicodeBinaryKind::Posix(PosixKind::Alnum),
        "DIGIT" => UnicodeBinaryKind::Posix(PosixKind::Digit),
        "SPACE" => UnicodeBinaryKind::Posix(PosixKind::Space),
        "BLANK" => UnicodeBinaryKind::Posix(PosixKind::Blank),
        "LOWER" => UnicodeBinaryKind::Posix(PosixKind::Lower),
        "UPPER" => UnicodeBinaryKind::Posix(PosixKind::Upper),
        "GRAPH" => UnicodeBinaryKind::Posix(PosixKind::Graph),
        "PRINT" => UnicodeBinaryKind::Posix(PosixKind::Print),
        "CNTRL" => UnicodeBinaryKind::Posix(PosixKind::Cntrl),
        _ => return None,
    };
    Some(CompiledProperty::UnicodeBinary { kind, ci })
}

fn compile_for_property(name: &str, ci: bool) -> Option<CompiledProperty> {
    if let Some(mask) = category_mask(name, ci) {
        return Some(CompiledProperty::CategoryMask(mask));
    }
    if let Some(bit) = java_bit(name) {
        return Some(CompiledProperty::JavaBit { bit, ci });
    }
    if let Some(kind) = posix_kind_by_canonical(name) {
        return Some(CompiledProperty::Posix {
            kind,
            unicode: false,
            ci,
        });
    }
    match name {
        "ASCII" => Some(CompiledProperty::Ascii),
        "L1" => Some(CompiledProperty::Latin1),
        "all" => Some(CompiledProperty::All),
        _ => None,
    }
}

pub fn compile_unicode_property(name: &str, uc: bool, ci: bool) -> Option<CompiledProperty> {
    if let Some((kind, value)) = name.split_once('=') {
        let kind = property_key(kind)?;
        let kind = kind.as_str();
        if matches!(kind, "sc" | "script") {
            let id = dictionary_id("script", value)?;
            return Some(CompiledProperty::Script(id));
        }
        if matches!(kind, "blk" | "block") {
            let id = dictionary_id("block", value)?;
            return Some(CompiledProperty::Block(id));
        }
        if matches!(kind, "gc" | "general_category") {
            return compile_for_property(value, ci);
        }
        return None;
    }
    if let Some(value) = name.strip_prefix("In") {
        let id = dictionary_id("block", value)?;
        return Some(CompiledProperty::Block(id));
    }
    if let Some(value) = name.strip_prefix("Is") {
        if let Some(prop) = compile_unicode_binary(value, ci) {
            return Some(prop);
        }
        if let Some(prop) = compile_for_property(value, ci) {
            return Some(prop);
        }
        let id = dictionary_id("script", value)?;
        return Some(CompiledProperty::Script(id));
    }
    if uc
        && let Some(canonical) = posix_name(name)
        && let Some(kind) = posix_kind_by_canonical(canonical)
    {
        return Some(CompiledProperty::Posix {
            kind,
            unicode: true,
            ci,
        });
    }
    compile_for_property(name, ci)
}

#[inline]
pub fn match_compiled_property(prop: CompiledProperty, cp: u32) -> bool {
    match prop {
        CompiledProperty::Script(id) => character_record(cp).is_some_and(|r| r.script_id == id),
        CompiledProperty::Block(id) => character_record(cp).is_some_and(|r| r.block_id == id),
        CompiledProperty::CategoryMask(mask) => mask & (1 << category(cp)) != 0,
        CompiledProperty::JavaBit { bit: b, ci } => {
            bit(cp, b)
                || (ci && matches!(b, 16 | 19 | 22) && (bit(cp, 16) || bit(cp, 19) || bit(cp, 22)))
        }
        CompiledProperty::Posix { kind, unicode, ci } => eval_posix(kind, cp, unicode, ci),
        CompiledProperty::UnicodeBinary { kind, ci } => match kind {
            UnicodeBinaryKind::Alphabetic => bit(cp, 0),
            UnicodeBinaryKind::Assigned => category(cp) != 0,
            UnicodeBinaryKind::Control => category(cp) == 15,
            UnicodeBinaryKind::Emoji => bit(cp, 3),
            UnicodeBinaryKind::EmojiComponent => bit(cp, 4),
            UnicodeBinaryKind::EmojiModifier => bit(cp, 5),
            UnicodeBinaryKind::EmojiModifierBase => bit(cp, 6),
            UnicodeBinaryKind::EmojiPresentation => bit(cp, 7),
            UnicodeBinaryKind::ExtendedPictographic => bit(cp, 8),
            UnicodeBinaryKind::Ideographic => bit(cp, 11),
            UnicodeBinaryKind::Letter => bit(cp, 14),
            UnicodeBinaryKind::Lowercase => bit(cp, 16) || ci && (bit(cp, 19) || bit(cp, 22)),
            UnicodeBinaryKind::Uppercase => bit(cp, 22) || ci && (bit(cp, 16) || bit(cp, 19)),
            UnicodeBinaryKind::Titlecase => bit(cp, 19) || ci && (bit(cp, 16) || bit(cp, 22)),
            UnicodeBinaryKind::Whitespace => unicode_space(cp),
            UnicodeBinaryKind::Word => is_word_char(cp, true),
            UnicodeBinaryKind::JoinControl => matches!(cp, 0x200c | 0x200d),
            UnicodeBinaryKind::NoncharacterCodePoint => {
                cp & 0xfffe == 0xfffe || (0xfdd0..=0xfdef).contains(&cp)
            }
            UnicodeBinaryKind::Posix(pk) => eval_posix(pk, cp, true, ci),
        },
        CompiledProperty::Ascii => cp < 128,
        CompiledProperty::Latin1 => cp <= 255,
        CompiledProperty::All => cp <= 0x10ffff,
    }
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
