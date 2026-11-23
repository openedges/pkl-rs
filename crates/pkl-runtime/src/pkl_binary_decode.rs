//! Pure-Rust MessagePack decoder for Pkl's `pkl-binary` format (Pkl 0.32.0).
//!
//! Matches `org.pkl.core.PklBinaryDecoder` and `AbstractPklBinaryDecoder` across
//! all 19 `PklBinaryCode` tags (`0x01`–`0x0F`, `0x10`–`0x12`, `0x20` `Reference`).
//!
//! - [`decode`] / [`PklBinaryDecoder::decode`] matches upstream `PklBinaryDecoder.decode(byte[])`
//!   strictly, including non-exportable type errors (`IntSeq`, `Class`, `TypeAlias`, `Function`),
//!   unrecognized `REFERENCE` object code, blank metadata validation, URI and regex validation,
//!   and exact error offsets/paths.
//! - [`decode_lossless`] / [`PklBinaryDecoder::decode_lossless`] decodes all 19 wire tags
//!   including `IntSeq`, `Class`, `TypeAlias`, `Function`, and `0x20` `Reference`.

use crate::data_size::{DataSize, DataSizeUnit};
use crate::duration::{Duration, DurationUnit};
use crate::int_seq::IntSeq;
use crate::value::PklString;
use std::fmt;

/// All 19 `org.pkl.core.util.pklbinary.PklBinaryCode` wire tags.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum PklBinaryCode {
    Object = 0x01,
    Map = 0x02,
    Mapping = 0x03,
    List = 0x04,
    Listing = 0x05,
    Set = 0x06,
    Duration = 0x07,
    DataSize = 0x08,
    Pair = 0x09,
    IntSeq = 0x0A,
    Regex = 0x0B,
    Class = 0x0C,
    TypeAlias = 0x0D,
    Function = 0x0E,
    Bytes = 0x0F,
    Property = 0x10,
    Entry = 0x11,
    Element = 0x12,
    Reference = 0x20,
}

impl PklBinaryCode {
    pub const fn code(self) -> u8 {
        self as u8
    }

    pub const fn from_i32(value: i32) -> Option<Self> {
        match value {
            0x01 => Some(Self::Object),
            0x02 => Some(Self::Map),
            0x03 => Some(Self::Mapping),
            0x04 => Some(Self::List),
            0x05 => Some(Self::Listing),
            0x06 => Some(Self::Set),
            0x07 => Some(Self::Duration),
            0x08 => Some(Self::DataSize),
            0x09 => Some(Self::Pair),
            0x0A => Some(Self::IntSeq),
            0x0B => Some(Self::Regex),
            0x0C => Some(Self::Class),
            0x0D => Some(Self::TypeAlias),
            0x0E => Some(Self::Function),
            0x0F => Some(Self::Bytes),
            0x10 => Some(Self::Property),
            0x11 => Some(Self::Entry),
            0x12 => Some(Self::Element),
            0x20 => Some(Self::Reference),
            _ => None,
        }
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Object => "OBJECT",
            Self::Map => "MAP",
            Self::Mapping => "MAPPING",
            Self::List => "LIST",
            Self::Listing => "LISTING",
            Self::Set => "SET",
            Self::Duration => "DURATION",
            Self::DataSize => "DATASIZE",
            Self::Pair => "PAIR",
            Self::IntSeq => "INTSEQ",
            Self::Regex => "REGEX",
            Self::Class => "CLASS",
            Self::TypeAlias => "TYPEALIAS",
            Self::Function => "FUNCTION",
            Self::Bytes => "BYTES",
            Self::Property => "PROPERTY",
            Self::Entry => "ENTRY",
            Self::Element => "ELEMENT",
            Self::Reference => "REFERENCE",
        }
    }
}

impl fmt::Display for PklBinaryCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Decoded `PClassInfo` classification matching `PklBinaryDecoder.doDecodeObject`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DecodedClassInfo {
    pub module_name: PklString,
    pub simple_name: PklString,
    pub module_uri: PklString,
    pub is_module: bool,
    pub is_dynamic: bool,
}

fn cached_pkl_base_module() -> PklString {
    static S: std::sync::OnceLock<PklString> = std::sync::OnceLock::new();
    S.get_or_init(|| PklString::from("pkl.base")).clone()
}

fn cached_dynamic_class() -> PklString {
    static S: std::sync::OnceLock<PklString> = std::sync::OnceLock::new();
    S.get_or_init(|| PklString::from("Dynamic")).clone()
}

fn cached_module_class() -> PklString {
    static S: std::sync::OnceLock<PklString> = std::sync::OnceLock::new();
    S.get_or_init(|| PklString::from("ModuleClass")).clone()
}

impl DecodedClassInfo {
    pub fn from_wire(class_name: &PklString, module_uri: &PklString) -> Self {
        if module_uri.as_str() == Some("pkl:base") {
            if class_name.as_str() == Some("Dynamic") {
                return Self {
                    module_name: cached_pkl_base_module(),
                    simple_name: cached_dynamic_class(),
                    module_uri: module_uri.clone(),
                    is_module: false,
                    is_dynamic: true,
                };
            }
            if class_name.as_str() != Some("pkl.base") {
                return Self {
                    module_name: cached_pkl_base_module(),
                    simple_name: class_name.clone(),
                    module_uri: module_uri.clone(),
                    is_module: false,
                    is_dynamic: false,
                };
            }
        }

        if let Some(s) = class_name.as_str() {
            if let Some(hash_index) = s.rfind('#') {
                return Self {
                    module_name: PklString::from(&s[..hash_index]),
                    simple_name: PklString::from(&s[hash_index + 1..]),
                    module_uri: module_uri.clone(),
                    is_module: false,
                    is_dynamic: false,
                };
            }
            return Self {
                module_name: class_name.clone(),
                simple_name: cached_module_class(),
                module_uri: module_uri.clone(),
                is_module: true,
                is_dynamic: false,
            };
        }

        let units: Vec<u16> = class_name.encode_utf16().collect();
        if let Some(hash_index) = units.iter().rposition(|&u| u == u16::from(b'#')) {
            Self {
                module_name: PklString::from_utf16(units[..hash_index].to_vec()),
                simple_name: PklString::from_utf16(units[hash_index + 1..].to_vec()),
                module_uri: module_uri.clone(),
                is_module: false,
                is_dynamic: false,
            }
        } else {
            Self {
                module_name: class_name.clone(),
                simple_name: "ModuleClass".into(),
                module_uri: module_uri.clone(),
                is_module: true,
                is_dynamic: false,
            }
        }
    }

    pub fn qualified_name(&self) -> PklString {
        self.module_name
            .concat(&"#".into())
            .concat(&self.simple_name)
    }

    pub fn display_name(&self) -> PklString {
        if self.module_name == "pkl.base" {
            self.simple_name.clone()
        } else if self.is_module {
            self.module_name.clone()
        } else {
            self.qualified_name()
        }
    }
}

/// Kind of object member encoded inside an `OBJECT` (`0x01`) member array.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ObjectMemberKind {
    Property,
    Entry,
    Element,
}

/// One decoded member of a `DecodedObject`, preserving wire kind, key, and order.
#[derive(Clone, Debug, PartialEq)]
pub struct DecodedObjectMember {
    pub kind: ObjectMemberKind,
    pub key: DecodedValue,
    pub value: DecodedValue,
}

/// Decoded `OBJECT` (`0x01`) covering `Dynamic`, `Typed`, and `Module` values.
#[derive(Clone, Debug, PartialEq)]
pub struct DecodedObject {
    pub class_name: PklString,
    pub module_uri: PklString,
    pub class_info: DecodedClassInfo,
    pub members: Vec<DecodedObjectMember>,
}

impl DecodedObject {
    /// Returns string-keyed properties in insertion order (last write wins on
    /// duplicate stringified keys), matching `PklBinaryDecoder.doDecodeObject`'s
    /// `LinkedHashMap<String, Object>`.
    pub fn properties(&self) -> Vec<(PklString, &DecodedValue)> {
        let mut ordered: Vec<(PklString, &DecodedValue)> = Vec::with_capacity(self.members.len());
        for member in &self.members {
            let key = member.key.to_java_key_string();
            if let Some((_, slot)) = ordered.iter_mut().find(|(existing, _)| *existing == key) {
                *slot = &member.value;
            } else {
                ordered.push((key, &member.value));
            }
        }
        ordered
    }

    pub fn get_property(&self, name: &str) -> Option<&DecodedValue> {
        let mut found = None;
        for member in &self.members {
            if member.key.to_java_key_string() == name {
                found = Some(&member.value);
            }
        }
        found
    }
}

/// Decoded `INTSEQ` (`0x0A`) payload.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DecodedIntSeq {
    pub start: i64,
    pub end: i64,
    pub step: i64,
}

impl DecodedIntSeq {
    pub fn to_int_seq(self) -> Option<IntSeq> {
        (self.step != 0).then(|| IntSeq::new(self.start, self.end, self.step))
    }
}

/// Decoded `REFERENCE` (`0x20`) payload (`[0x20, domain, data, [access...]]`).
#[derive(Clone, Debug, PartialEq)]
pub struct DecodedReference {
    pub domain: Box<DecodedValue>,
    pub data: Box<DecodedValue>,
    pub path: Vec<DecodedValue>,
}

/// Decoded value produced by [`PklBinaryDecoder`].
#[derive(Clone, Debug, PartialEq)]
pub enum DecodedValue {
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    String(PklString),
    Object(DecodedObject),
    Map(Vec<(DecodedValue, DecodedValue)>),
    Mapping(Vec<(DecodedValue, DecodedValue)>),
    List(Vec<DecodedValue>),
    Listing(Vec<DecodedValue>),
    Set(Vec<DecodedValue>),
    Duration(Duration),
    DataSize(DataSize),
    Pair(Box<DecodedValue>, Box<DecodedValue>),
    IntSeq(DecodedIntSeq),
    Regex(PklString),
    Class {
        qualified_name: PklString,
        module_uri: PklString,
    },
    TypeAlias {
        qualified_name: PklString,
        module_uri: PklString,
    },
    Function,
    Bytes(Vec<u8>),
    Reference(DecodedReference),
}

impl DecodedValue {
    /// Java `Object.toString()` projection used by `AbstractPklBinaryDecoder`
    /// path formatting and `PklBinaryDecoder.doDecodeObject`'s `member.key().toString()`.
    pub fn to_java_key_string(&self) -> PklString {
        match self {
            Self::Null => "null".into(),
            Self::Bool(b) => if *b { "true" } else { "false" }.into(),
            Self::Int(n) => n.to_string().into(),
            Self::Float(f) => crate::numeric::double_to_string(*f).into(),
            Self::String(s) => s.clone(),
            Self::Duration(d) => d.to_string().into(),
            Self::DataSize(d) => d.to_string().into(),
            Self::Regex(p) => p.clone(),
            Self::Pair(first, second) => PklString::from("Pair(")
                .concat(&first.to_java_key_string())
                .concat(&", ".into())
                .concat(&second.to_java_key_string())
                .concat(&")".into()),
            Self::List(items) | Self::Listing(items) | Self::Set(items) => {
                let mut out = PklString::from("[");
                for (idx, item) in items.iter().enumerate() {
                    if idx > 0 {
                        out = out.concat(&", ".into());
                    }
                    out = out.concat(&item.to_java_key_string());
                }
                out.concat(&"]".into())
            }
            Self::Map(entries) | Self::Mapping(entries) => {
                let mut out = PklString::from("{");
                for (idx, (k, v)) in entries.iter().enumerate() {
                    if idx > 0 {
                        out = out.concat(&", ".into());
                    }
                    out = out
                        .concat(&k.to_java_key_string())
                        .concat(&"=".into())
                        .concat(&v.to_java_key_string());
                }
                out.concat(&"}".into())
            }
            Self::Object(obj) => {
                let prefix = if obj.class_info.is_module {
                    obj.class_info.module_name.clone()
                } else {
                    obj.class_info.display_name()
                };
                let mut out = prefix.concat(&" { ".into());
                for (idx, (k, v)) in obj.properties().into_iter().enumerate() {
                    if idx > 0 {
                        out = out.concat(&"; ".into());
                    }
                    out = out
                        .concat(&k)
                        .concat(&" = ".into())
                        .concat(&v.to_java_key_string());
                }
                out.concat(&" }".into())
            }
            Self::IntSeq(seq) => match seq.to_int_seq() {
                Some(int_seq) => int_seq.to_string().into(),
                None => format!("IntSeq({}, {}).step({})", seq.start, seq.end, seq.step).into(),
            },
            Self::Class { qualified_name, .. } | Self::TypeAlias { qualified_name, .. } => {
                qualified_name.clone()
            }
            Self::Function => "function".into(),
            Self::Bytes(bytes) => {
                let mut out = String::from("Bytes(");
                for (idx, b) in bytes.iter().enumerate() {
                    if idx > 0 {
                        out.push_str(", ");
                    }
                    out.push_str(&b.to_string());
                }
                out.push(')');
                out.into()
            }
            Self::Reference(reference) => PklString::from("Reference(")
                .concat(&reference.domain.to_java_key_string())
                .concat(&", ".into())
                .concat(&reference.data.to_java_key_string())
                .concat(&")".into()),
        }
    }
}

/// Decoder policy controlling strict `PklBinaryDecoder` compatibility vs full
/// lossless wire decoding.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DecodeOptions {
    pub allow_int_seq: bool,
    pub allow_class: bool,
    pub allow_type_alias: bool,
    pub allow_function: bool,
    pub allow_reference: bool,
    pub validate_uri: bool,
    pub validate_regex: bool,
}

impl DecodeOptions {
    /// Strict `org.pkl.core.PklBinaryDecoder` 0.32.0 compatibility mode.
    pub const fn strict() -> Self {
        Self {
            allow_int_seq: false,
            allow_class: false,
            allow_type_alias: false,
            allow_function: false,
            allow_reference: false,
            validate_uri: true,
            validate_regex: true,
        }
    }

    /// Lossless wire mode accepting all 19 `PklBinaryCode` tags including
    /// `IntSeq`, `Class`, `TypeAlias`, `Function`, and `0x20` `Reference`.
    pub const fn lossless() -> Self {
        Self {
            allow_int_seq: true,
            allow_class: true,
            allow_type_alias: true,
            allow_function: true,
            allow_reference: true,
            validate_uri: true,
            validate_regex: true,
        }
    }
}

impl Default for DecodeOptions {
    fn default() -> Self {
        Self::strict()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DecodeErrorKind {
    UnexpectedEof,
    InvalidFormat,
    InvalidType,
    IntegerOverflow,
    SizeOverflow,
    UnsupportedType,
    InvalidStructure,
    InvalidUnit,
    InvalidUri,
    InvalidRegex,
}

/// Structured error returned by [`PklBinaryDecoder`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DecodeError {
    pub kind: DecodeErrorKind,
    pub message: String,
    pub cause: String,
    pub offset: usize,
    pub path: Vec<PklString>,
}

impl fmt::Display for DecodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.message, self.cause)
    }
}

impl std::error::Error for DecodeError {}

/// Decoder for Pkl's `pkl-binary` MessagePack representation.
pub struct PklBinaryDecoder;

impl PklBinaryDecoder {
    /// Decodes a single value matching `org.pkl.core.PklBinaryDecoder.decode(byte[])`.
    pub fn decode(bytes: &[u8]) -> Result<DecodedValue, DecodeError> {
        decode_with_options(bytes, DecodeOptions::strict())
    }

    /// Decodes a single value accepting all 19 `PklBinaryCode` wire tags,
    /// including `IntSeq`, `Class`, `TypeAlias`, `Function`, and `0x20` `Reference`.
    pub fn decode_lossless(bytes: &[u8]) -> Result<DecodedValue, DecodeError> {
        decode_with_options(bytes, DecodeOptions::lossless())
    }

    /// Decodes a single value with explicit [`DecodeOptions`].
    pub fn decode_with_options(
        bytes: &[u8],
        options: DecodeOptions,
    ) -> Result<DecodedValue, DecodeError> {
        decode_with_options(bytes, options)
    }
}

/// Convenience function matching `org.pkl.core.PklBinaryDecoder.decode(byte[])`.
pub fn decode(bytes: &[u8]) -> Result<DecodedValue, DecodeError> {
    decode_with_options(bytes, DecodeOptions::strict())
}

/// Convenience function decoding all 19 `PklBinaryCode` wire tags losslessly.
pub fn decode_lossless(bytes: &[u8]) -> Result<DecodedValue, DecodeError> {
    decode_with_options(bytes, DecodeOptions::lossless())
}

/// Decodes `bytes` under the given [`DecodeOptions`].
pub fn decode_with_options(
    bytes: &[u8],
    options: DecodeOptions,
) -> Result<DecodedValue, DecodeError> {
    let mut decoder = Decoder::new(bytes, options);
    decoder.run()
}

// ==================== MessagePack Unpacker ====================

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum MsgPackValueType {
    Nil,
    Boolean,
    Integer,
    Float,
    String,
    Binary,
    Array,
    Map,
    Extension,
}

#[derive(Clone, Debug)]
struct RawError {
    kind: DecodeErrorKind,
    cause: String,
    wrapped: bool,
}

impl RawError {
    fn decode(kind: DecodeErrorKind, cause: impl Into<String>) -> Self {
        Self {
            kind,
            cause: cause.into(),
            wrapped: true,
        }
    }

    fn direct(kind: DecodeErrorKind, cause: impl Into<String>) -> Self {
        Self {
            kind,
            cause: cause.into(),
            wrapped: false,
        }
    }
}

struct Unpacker<'a> {
    bytes: &'a [u8],
    pos: usize,
}

impl<'a> Unpacker<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, pos: 0 }
    }

    fn offset(&self) -> usize {
        self.pos
    }

    fn has_next(&self) -> bool {
        self.pos < self.bytes.len()
    }

    fn peek_byte(&self) -> Result<u8, RawError> {
        self.bytes
            .get(self.pos)
            .copied()
            .ok_or_else(|| RawError::decode(DecodeErrorKind::UnexpectedEof, "Unexpected EOF"))
    }

    fn read_byte(&mut self) -> Result<u8, RawError> {
        let byte = self.peek_byte()?;
        self.pos += 1;
        Ok(byte)
    }

    fn read_exact<const N: usize>(&mut self) -> Result<[u8; N], RawError> {
        if self.bytes.len().saturating_sub(self.pos) < N {
            return Err(RawError::decode(
                DecodeErrorKind::UnexpectedEof,
                "Unexpected EOF",
            ));
        }
        let mut out = [0u8; N];
        out.copy_from_slice(&self.bytes[self.pos..self.pos + N]);
        self.pos += N;
        Ok(out)
    }

    fn read_slice(&mut self, len: usize) -> Result<&'a [u8], RawError> {
        if self.bytes.len().saturating_sub(self.pos) < len {
            return Err(RawError::decode(
                DecodeErrorKind::UnexpectedEof,
                "Unexpected EOF",
            ));
        }
        let slice = &self.bytes[self.pos..self.pos + len];
        self.pos += len;
        Ok(slice)
    }

    fn next_value_type(&self) -> Result<MsgPackValueType, RawError> {
        let b = self.peek_byte()?;
        format_value_type(b)
    }

    fn unpack_nil(&mut self) -> Result<(), RawError> {
        let b = self.read_byte()?;
        if b == 0xc0 {
            Ok(())
        } else {
            Err(type_mismatch("Nil", b))
        }
    }

    fn unpack_bool(&mut self) -> Result<bool, RawError> {
        let b = self.read_byte()?;
        match b {
            0xc2 => Ok(false),
            0xc3 => Ok(true),
            _ => Err(type_mismatch("Boolean", b)),
        }
    }

    fn unpack_long(&mut self) -> Result<i64, RawError> {
        let b = self.read_byte()?;
        if b <= 0x7f {
            return Ok(i64::from(b));
        }
        if b >= 0xe0 {
            return Ok(i64::from(b as i8));
        }
        match b {
            0xcc => Ok(i64::from(self.read_byte()?)),
            0xcd => Ok(i64::from(u16::from_be_bytes(self.read_exact::<2>()?))),
            0xce => Ok(i64::from(u32::from_be_bytes(self.read_exact::<4>()?))),
            0xcf => {
                let v = u64::from_be_bytes(self.read_exact::<8>()?);
                i64::try_from(v)
                    .map_err(|_| RawError::decode(DecodeErrorKind::IntegerOverflow, v.to_string()))
            }
            0xd0 => Ok(i64::from(self.read_byte()? as i8)),
            0xd1 => Ok(i64::from(i16::from_be_bytes(self.read_exact::<2>()?))),
            0xd2 => Ok(i64::from(i32::from_be_bytes(self.read_exact::<4>()?))),
            0xd3 => Ok(i64::from_be_bytes(self.read_exact::<8>()?)),
            _ => Err(type_mismatch("Integer", b)),
        }
    }

    fn unpack_int(&mut self) -> Result<i32, RawError> {
        let value = self.unpack_long()?;
        i32::try_from(value)
            .map_err(|_| RawError::decode(DecodeErrorKind::IntegerOverflow, value.to_string()))
    }

    fn unpack_double(&mut self) -> Result<f64, RawError> {
        let b = self.read_byte()?;
        match b {
            0xca => Ok(f64::from(f32::from_be_bytes(self.read_exact::<4>()?))),
            0xcb => Ok(f64::from_be_bytes(self.read_exact::<8>()?)),
            _ => Err(type_mismatch("Float", b)),
        }
    }

    fn unpack_raw_string_header(&mut self) -> Result<usize, RawError> {
        let b = self.read_byte()?;
        if (0xa0..=0xbf).contains(&b) {
            return Ok(usize::from(b & 0x1f));
        }
        match b {
            0xd9 => Ok(usize::from(self.read_byte()?)),
            0xda => Ok(usize::from(u16::from_be_bytes(self.read_exact::<2>()?))),
            0xdb => {
                let len = u32::from_be_bytes(self.read_exact::<4>()?);
                check_msgpack_len(len)
            }
            _ => Err(type_mismatch("String", b)),
        }
    }

    fn unpack_string(&mut self) -> Result<PklString, RawError> {
        let len = self.unpack_raw_string_header()?;
        let payload = self.read_slice(len)?;
        // Default MessagePack UnpackerConfig decodes UTF-8 with JDK 21's
        // UTF_8$Decoder (CodingErrorAction.REPLACE), which differs from Rust's
        // String::from_utf8_lossy on UTF-8 encoded surrogates (ED A0..BF 80..BF
        // -> single U+FFFD, truncated ED A0..BF -> single U+FFFD).
        match std::str::from_utf8(payload) {
            Ok(valid) => Ok(PklString::from(valid)),
            Err(_) => Ok(PklString::from(decode_java_utf8_replace(payload))),
        }
    }

    fn unpack_binary_header(&mut self) -> Result<usize, RawError> {
        let b = self.read_byte()?;
        match b {
            0xc4 => Ok(usize::from(self.read_byte()?)),
            0xc5 => Ok(usize::from(u16::from_be_bytes(self.read_exact::<2>()?))),
            0xc6 => {
                let len = u32::from_be_bytes(self.read_exact::<4>()?);
                check_msgpack_len(len)
            }
            _ => Err(type_mismatch("Binary", b)),
        }
    }

    fn unpack_array_header(&mut self) -> Result<usize, RawError> {
        let b = self.read_byte()?;
        if (0x90..=0x9f).contains(&b) {
            return Ok(usize::from(b & 0x0f));
        }
        match b {
            0xdc => Ok(usize::from(u16::from_be_bytes(self.read_exact::<2>()?))),
            0xdd => {
                let len = u32::from_be_bytes(self.read_exact::<4>()?);
                check_msgpack_len(len)
            }
            _ => Err(type_mismatch("Array", b)),
        }
    }

    fn unpack_map_header(&mut self) -> Result<usize, RawError> {
        let b = self.read_byte()?;
        if (0x80..=0x8f).contains(&b) {
            return Ok(usize::from(b & 0x0f));
        }
        match b {
            0xde => Ok(usize::from(u16::from_be_bytes(self.read_exact::<2>()?))),
            0xdf => {
                let len = u32::from_be_bytes(self.read_exact::<4>()?);
                check_msgpack_len(len)
            }
            _ => Err(type_mismatch("Map", b)),
        }
    }

    /// Iterative equivalent of `MessageUnpacker.skipValue(int count)`.
    fn skip_values(&mut self, count: usize) -> Result<(), RawError> {
        let mut remaining = count;
        while remaining > 0 {
            remaining -= 1;
            let b = self.read_byte()?;
            if b <= 0x7f || b >= 0xe0 || matches!(b, 0xc0 | 0xc2 | 0xc3) {
                continue;
            }
            if (0x80..=0x8f).contains(&b) {
                let pairs = usize::from(b & 0x0f);
                remaining = remaining
                    .checked_add(pairs.saturating_mul(2))
                    .ok_or_else(|| {
                        RawError::decode(DecodeErrorKind::SizeOverflow, "Map length overflow")
                    })?;
            } else if (0x90..=0x9f).contains(&b) {
                let items = usize::from(b & 0x0f);
                remaining = remaining.checked_add(items).ok_or_else(|| {
                    RawError::decode(DecodeErrorKind::SizeOverflow, "Array length overflow")
                })?;
            } else if (0xa0..=0xbf).contains(&b) {
                let len = usize::from(b & 0x1f);
                self.read_slice(len)?;
            } else {
                match b {
                    0xc1 => {
                        return Err(RawError::decode(
                            DecodeErrorKind::InvalidFormat,
                            "Encountered 0xC1 \"NEVER_USED\" byte",
                        ));
                    }
                    0xc4 | 0xd9 => {
                        let len = usize::from(self.read_byte()?);
                        self.read_slice(len)?;
                    }
                    0xc5 | 0xda => {
                        let len = usize::from(u16::from_be_bytes(self.read_exact::<2>()?));
                        self.read_slice(len)?;
                    }
                    0xc6 | 0xdb => {
                        let len = check_msgpack_len(u32::from_be_bytes(self.read_exact::<4>()?))?;
                        self.read_slice(len)?;
                    }
                    0xc7 => {
                        let len = usize::from(self.read_byte()?);
                        let _ext_type = self.read_byte()?;
                        self.read_slice(len)?;
                    }
                    0xc8 => {
                        let len = usize::from(u16::from_be_bytes(self.read_exact::<2>()?));
                        let _ext_type = self.read_byte()?;
                        self.read_slice(len)?;
                    }
                    0xc9 => {
                        let len = check_msgpack_len(u32::from_be_bytes(self.read_exact::<4>()?))?;
                        let _ext_type = self.read_byte()?;
                        self.read_slice(len)?;
                    }
                    0xca | 0xce | 0xd2 => {
                        self.read_exact::<4>()?;
                    }
                    0xcb | 0xcf | 0xd3 => {
                        self.read_exact::<8>()?;
                    }
                    0xcc | 0xd0 => {
                        self.read_byte()?;
                    }
                    0xcd | 0xd1 => {
                        self.read_exact::<2>()?;
                    }
                    0xd4 => {
                        self.read_exact::<2>()?;
                    }
                    0xd5 => {
                        self.read_exact::<3>()?;
                    }
                    0xd6 => {
                        self.read_exact::<5>()?;
                    }
                    0xd7 => {
                        self.read_exact::<9>()?;
                    }
                    0xd8 => {
                        self.read_exact::<17>()?;
                    }
                    0xdc => {
                        let len = usize::from(u16::from_be_bytes(self.read_exact::<2>()?));
                        remaining = remaining.checked_add(len).ok_or_else(|| {
                            RawError::decode(DecodeErrorKind::SizeOverflow, "Array length overflow")
                        })?;
                    }
                    0xdd => {
                        let len = check_msgpack_len(u32::from_be_bytes(self.read_exact::<4>()?))?;
                        remaining = remaining.checked_add(len).ok_or_else(|| {
                            RawError::decode(DecodeErrorKind::SizeOverflow, "Array length overflow")
                        })?;
                    }
                    0xde => {
                        let pairs = usize::from(u16::from_be_bytes(self.read_exact::<2>()?));
                        remaining =
                            remaining
                                .checked_add(pairs.saturating_mul(2))
                                .ok_or_else(|| {
                                    RawError::decode(
                                        DecodeErrorKind::SizeOverflow,
                                        "Map length overflow",
                                    )
                                })?;
                    }
                    0xdf => {
                        let pairs = check_msgpack_len(u32::from_be_bytes(self.read_exact::<4>()?))?;
                        let doubled = pairs.checked_mul(2).ok_or_else(|| {
                            RawError::decode(DecodeErrorKind::SizeOverflow, "Map length overflow")
                        })?;
                        remaining = remaining.checked_add(doubled).ok_or_else(|| {
                            RawError::decode(DecodeErrorKind::SizeOverflow, "Map length overflow")
                        })?;
                    }
                    _ => unreachable!("all 256 byte values covered"),
                }
            }
        }
        Ok(())
    }
}

fn check_msgpack_len(len: u32) -> Result<usize, RawError> {
    if len > i32::MAX as u32 {
        return Err(RawError::decode(
            DecodeErrorKind::SizeOverflow,
            format!("Size {len} exceeds maximum 31-bit MessagePack length"),
        ));
    }
    usize::try_from(len).map_err(|_| {
        RawError::decode(
            DecodeErrorKind::SizeOverflow,
            format!("Size {len} exceeds addressable length"),
        )
    })
}

fn format_value_type(b: u8) -> Result<MsgPackValueType, RawError> {
    if b <= 0x7f || b >= 0xe0 {
        return Ok(MsgPackValueType::Integer);
    }
    if (0x80..=0x8f).contains(&b) {
        return Ok(MsgPackValueType::Map);
    }
    if (0x90..=0x9f).contains(&b) {
        return Ok(MsgPackValueType::Array);
    }
    if (0xa0..=0xbf).contains(&b) {
        return Ok(MsgPackValueType::String);
    }
    match b {
        0xc0 => Ok(MsgPackValueType::Nil),
        0xc1 => Err(RawError::decode(
            DecodeErrorKind::InvalidFormat,
            "Cannot convert NEVER_USED to ValueType",
        )),
        0xc2 | 0xc3 => Ok(MsgPackValueType::Boolean),
        0xc4..=0xc6 => Ok(MsgPackValueType::Binary),
        0xc7..=0xc9 | 0xd4..=0xd8 => Ok(MsgPackValueType::Extension),
        0xca | 0xcb => Ok(MsgPackValueType::Float),
        0xcc..=0xd3 => Ok(MsgPackValueType::Integer),
        0xd9..=0xdb => Ok(MsgPackValueType::String),
        0xdc | 0xdd => Ok(MsgPackValueType::Array),
        0xde | 0xdf => Ok(MsgPackValueType::Map),
        _ => unreachable!("all 256 byte values covered"),
    }
}

fn value_type_name(b: u8) -> &'static str {
    match format_value_type(b) {
        Ok(MsgPackValueType::Nil) => "Nil",
        Ok(MsgPackValueType::Boolean) => "Boolean",
        Ok(MsgPackValueType::Integer) => "Integer",
        Ok(MsgPackValueType::Float) => "Float",
        Ok(MsgPackValueType::String) => "String",
        Ok(MsgPackValueType::Binary) => "Binary",
        Ok(MsgPackValueType::Array) => "Array",
        Ok(MsgPackValueType::Map) => "Map",
        Ok(MsgPackValueType::Extension) => "Extension",
        Err(_) => "NEVER_USED",
    }
}

fn type_mismatch(expected: &str, byte: u8) -> RawError {
    RawError::decode(
        DecodeErrorKind::InvalidType,
        format!(
            "Expected {expected}, but got {} ({byte:02x})",
            value_type_name(byte)
        ),
    )
}

// ==================== Iterative Pkl Binary Decoder ====================

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CollectionKind {
    List,
    Listing,
    Set,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum MapKind {
    Map,
    Mapping,
}

enum ObjectMemberStep {
    EntryKey,
    Value {
        kind: ObjectMemberKind,
        key: DecodedValue,
    },
}

enum MapStep {
    Key,
    Value(DecodedValue),
}

enum PairStep {
    First,
    Second(DecodedValue),
}

enum ReferenceStep {
    Domain,
    Data(Box<DecodedValue>),
    Path {
        domain: Box<DecodedValue>,
        data: Box<DecodedValue>,
        total: usize,
        items: Vec<DecodedValue>,
    },
}

enum Frame {
    Object {
        class_name: PklString,
        module_uri: PklString,
        total: usize,
        idx: usize,
        extra_slots: usize,
        members: Vec<DecodedObjectMember>,
        step: ObjectMemberStep,
        iter_active: bool,
    },
    Map {
        kind: MapKind,
        total: usize,
        idx: usize,
        extra_slots: usize,
        entries: Vec<(DecodedValue, DecodedValue)>,
        step: MapStep,
        iter_active: bool,
    },
    Collection {
        kind: CollectionKind,
        total: usize,
        idx: usize,
        extra_slots: usize,
        items: Vec<DecodedValue>,
        iter_active: bool,
    },
    Pair {
        extra_slots: usize,
        step: PairStep,
    },
    Reference {
        extra_slots: usize,
        step: ReferenceStep,
        iter_active: bool,
    },
}

impl Frame {
    fn iter_active(&self) -> bool {
        match self {
            Self::Object { iter_active, .. }
            | Self::Map { iter_active, .. }
            | Self::Collection { iter_active, .. }
            | Self::Reference { iter_active, .. } => *iter_active,
            Self::Pair { .. } => false,
        }
    }
}

enum PathSegment {
    Static(&'static str),
    Index(usize),
    Int(i64),
    Owned(PklString),
}

impl PathSegment {
    fn to_pkl_string(&self) -> PklString {
        match self {
            Self::Static(s) => PklString::from(*s),
            Self::Index(i) => PklString::from(i.to_string()),
            Self::Int(i) => PklString::from(i.to_string()),
            Self::Owned(s) => s.clone(),
        }
    }
}

struct Decoder<'a> {
    unpacker: Unpacker<'a>,
    options: DecodeOptions,
    curr_path: Vec<PathSegment>,
    frames: Vec<Frame>,
}

impl<'a> Decoder<'a> {
    fn new(bytes: &'a [u8], options: DecodeOptions) -> Self {
        Self {
            unpacker: Unpacker::new(bytes),
            options,
            curr_path: Vec::new(),
            frames: Vec::new(),
        }
    }

    fn run(&mut self) -> Result<DecodedValue, DecodeError> {
        match self.run_inner() {
            Ok(value) => Ok(value),
            Err(raw) => Err(self.finalize_error(raw)),
        }
    }

    fn finalize_error(&mut self, raw: RawError) -> DecodeError {
        // Mirror Java's DecodeIterator.next() `finally { currPath.pop(); }`:
        // as an exception unwinds through active iterators, each active iterator
        // pops one element from the top of `currPath`.
        for frame in self.frames.iter().rev() {
            if frame.iter_active() {
                self.curr_path.pop();
            }
        }
        // AbstractPklBinaryDecoder reverses bottom-to-top into top-to-bottom
        // (innermost first, outermost last).
        let path: Vec<PklString> = self
            .curr_path
            .iter()
            .rev()
            .map(PathSegment::to_pkl_string)
            .collect();
        let offset = self.unpacker.offset();
        let message = if raw.wrapped {
            let joined = path
                .iter()
                .map(PklString::to_java_utf8)
                .collect::<Vec<_>>()
                .join(", ");
            format!("Exception while decoding binary data at offset {offset}, path [{joined}]")
        } else {
            raw.cause.clone()
        };
        DecodeError {
            kind: raw.kind,
            message,
            cause: raw.cause,
            offset,
            path,
        }
    }

    fn run_inner(&mut self) -> Result<DecodedValue, RawError> {
        let mut produced: Option<DecodedValue> = None;
        loop {
            if let Some(value) = produced.take() {
                let Some(frame) = self.frames.pop() else {
                    return Ok(value);
                };
                match frame {
                    Frame::Object {
                        class_name,
                        module_uri,
                        total,
                        idx,
                        extra_slots,
                        mut members,
                        step,
                        iter_active: _,
                    } => match step {
                        ObjectMemberStep::EntryKey => {
                            self.curr_path
                                .push(PathSegment::Owned(value.to_java_key_string()));
                            self.frames.push(Frame::Object {
                                class_name,
                                module_uri,
                                total,
                                idx,
                                extra_slots,
                                members,
                                step: ObjectMemberStep::Value {
                                    kind: ObjectMemberKind::Entry,
                                    key: value,
                                },
                                iter_active: true,
                            });
                            produced = self.start_decode()?;
                        }
                        ObjectMemberStep::Value { kind, key } => {
                            // Pop key pushed for this member, then pop idx (DecodeIterator.next finally).
                            self.curr_path.pop();
                            self.curr_path.pop();
                            members.push(DecodedObjectMember { kind, key, value });
                            let next_idx = idx + 1;
                            if next_idx < total {
                                self.advance_object_member(
                                    class_name,
                                    module_uri,
                                    total,
                                    next_idx,
                                    extra_slots,
                                    members,
                                )?;
                                produced = self.start_decode()?;
                            } else {
                                self.unpacker.skip_values(extra_slots)?;
                                self.curr_path.pop(); // "'object"
                                let class_info =
                                    DecodedClassInfo::from_wire(&class_name, &module_uri);
                                produced = Some(DecodedValue::Object(DecodedObject {
                                    class_name,
                                    module_uri,
                                    class_info,
                                    members,
                                }));
                            }
                        }
                    },
                    Frame::Map {
                        kind,
                        total,
                        idx,
                        extra_slots,
                        mut entries,
                        step,
                        iter_active: _,
                    } => match step {
                        MapStep::Key => {
                            self.curr_path
                                .push(PathSegment::Owned(value.to_java_key_string()));
                            self.frames.push(Frame::Map {
                                kind,
                                total,
                                idx,
                                extra_slots,
                                entries,
                                step: MapStep::Value(value),
                                iter_active: true,
                            });
                            produced = self.start_decode()?;
                        }
                        MapStep::Value(key) => {
                            self.curr_path.pop(); // key
                            self.curr_path.pop(); // idx
                            entries.push((key, value));
                            let next_idx = idx + 1;
                            if next_idx < total {
                                self.curr_path.push(PathSegment::Index(next_idx));
                                self.frames.push(Frame::Map {
                                    kind,
                                    total,
                                    idx: next_idx,
                                    extra_slots,
                                    entries,
                                    step: MapStep::Key,
                                    iter_active: true,
                                });
                                produced = self.start_decode()?;
                            } else {
                                self.unpacker.skip_values(extra_slots)?;
                                self.curr_path.pop(); // "'map" / "'mapping"
                                produced = Some(match kind {
                                    MapKind::Map => DecodedValue::Map(entries),
                                    MapKind::Mapping => DecodedValue::Mapping(entries),
                                });
                            }
                        }
                    },
                    Frame::Collection {
                        kind,
                        total,
                        idx,
                        extra_slots,
                        mut items,
                        iter_active: _,
                    } => {
                        self.curr_path.pop(); // idx
                        items.push(value);
                        let next_idx = idx + 1;
                        if next_idx < total {
                            self.curr_path.push(PathSegment::Index(next_idx));
                            self.frames.push(Frame::Collection {
                                kind,
                                total,
                                idx: next_idx,
                                extra_slots,
                                items,
                                iter_active: true,
                            });
                            produced = self.start_decode()?;
                        } else {
                            // Mirror AbstractPklBinaryDecoder.decodeSet popping currPath
                            // before skipValue, while List/Listing pop after skipValue.
                            if kind == CollectionKind::Set {
                                self.curr_path.pop();
                                self.unpacker.skip_values(extra_slots)?;
                            } else {
                                self.unpacker.skip_values(extra_slots)?;
                                self.curr_path.pop();
                            }
                            produced = Some(match kind {
                                CollectionKind::List => DecodedValue::List(items),
                                CollectionKind::Listing => DecodedValue::Listing(items),
                                CollectionKind::Set => DecodedValue::Set(items),
                            });
                        }
                    }
                    Frame::Pair { extra_slots, step } => match step {
                        PairStep::First => {
                            self.curr_path.pop(); // "'first"
                            self.curr_path.push(PathSegment::Static("'second"));
                            self.frames.push(Frame::Pair {
                                extra_slots,
                                step: PairStep::Second(value),
                            });
                            produced = self.start_decode()?;
                        }
                        PairStep::Second(first) => {
                            self.curr_path.pop(); // "'second"
                            self.unpacker.skip_values(extra_slots)?;
                            self.curr_path.pop(); // "'pair"
                            produced = Some(DecodedValue::Pair(Box::new(first), Box::new(value)));
                        }
                    },
                    Frame::Reference {
                        extra_slots,
                        step,
                        iter_active: _,
                    } => match step {
                        ReferenceStep::Domain => {
                            self.curr_path.pop(); // "'domain"
                            self.curr_path.push(PathSegment::Static("'data"));
                            self.frames.push(Frame::Reference {
                                extra_slots,
                                step: ReferenceStep::Data(Box::new(value)),
                                iter_active: false,
                            });
                            produced = self.start_decode()?;
                        }
                        ReferenceStep::Data(domain) => {
                            self.curr_path.pop(); // "'data"
                            self.curr_path.push(PathSegment::Static("'path"));
                            let total = self.unpacker.unpack_array_header()?;
                            let data = Box::new(value);
                            if total == 0 {
                                self.curr_path.pop(); // "'path"
                                self.unpacker.skip_values(extra_slots)?;
                                self.curr_path.pop(); // "'reference"
                                produced = Some(DecodedValue::Reference(DecodedReference {
                                    domain,
                                    data,
                                    path: Vec::new(),
                                }));
                            } else {
                                self.curr_path.push(PathSegment::Index(0));
                                self.frames.push(Frame::Reference {
                                    extra_slots,
                                    step: ReferenceStep::Path {
                                        domain,
                                        data,
                                        total,
                                        items: Vec::with_capacity(total.min(1024)),
                                    },
                                    iter_active: true,
                                });
                                produced = self.start_decode()?;
                            }
                        }
                        ReferenceStep::Path {
                            domain,
                            data,
                            total,
                            mut items,
                        } => {
                            self.curr_path.pop(); // idx
                            items.push(value);
                            if items.len() < total {
                                self.curr_path.push(PathSegment::Index(items.len()));
                                self.frames.push(Frame::Reference {
                                    extra_slots,
                                    step: ReferenceStep::Path {
                                        domain,
                                        data,
                                        total,
                                        items,
                                    },
                                    iter_active: true,
                                });
                                produced = self.start_decode()?;
                            } else {
                                self.curr_path.pop(); // "'path"
                                self.unpacker.skip_values(extra_slots)?;
                                self.curr_path.pop(); // "'reference"
                                produced = Some(DecodedValue::Reference(DecodedReference {
                                    domain,
                                    data,
                                    path: items,
                                }));
                            }
                        }
                    },
                }
            } else {
                produced = self.start_decode()?;
            }
        }
    }

    fn start_decode(&mut self) -> Result<Option<DecodedValue>, RawError> {
        if !self.unpacker.has_next() {
            return Err(RawError::decode(
                DecodeErrorKind::UnexpectedEof,
                "Unexpected EOF",
            ));
        }

        match self.unpacker.next_value_type()? {
            MsgPackValueType::Nil => {
                self.unpacker.unpack_nil()?;
                Ok(Some(DecodedValue::Null))
            }
            MsgPackValueType::String => {
                Ok(Some(DecodedValue::String(self.unpacker.unpack_string()?)))
            }
            MsgPackValueType::Integer => Ok(Some(DecodedValue::Int(self.unpacker.unpack_long()?))),
            MsgPackValueType::Boolean => Ok(Some(DecodedValue::Bool(self.unpacker.unpack_bool()?))),
            MsgPackValueType::Float => {
                Ok(Some(DecodedValue::Float(self.unpacker.unpack_double()?)))
            }
            MsgPackValueType::Array => self.start_non_primitive(),
            MsgPackValueType::Binary => Err(RawError::decode(
                DecodeErrorKind::InvalidStructure,
                "Unexpected msgpack bin value",
            )),
            MsgPackValueType::Map => Err(RawError::decode(
                DecodeErrorKind::InvalidStructure,
                "Unexpected msgpack map value",
            )),
            MsgPackValueType::Extension => Err(RawError::decode(
                DecodeErrorKind::InvalidStructure,
                "Unexpected msgpack ext value",
            )),
        }
    }

    fn assert_length(
        &self,
        code: PklBinaryCode,
        len: usize,
        expected: usize,
    ) -> Result<(), RawError> {
        if len < expected {
            return Err(RawError::decode(
                DecodeErrorKind::InvalidStructure,
                format!(
                    "Expected {code} structure to have at least {} slots, found {len}",
                    expected + 1
                ),
            ));
        }
        Ok(())
    }

    fn start_non_primitive(&mut self) -> Result<Option<DecodedValue>, RawError> {
        let len = self.unpacker.unpack_array_header()?;
        if len < 1 {
            return Err(RawError::decode(
                DecodeErrorKind::InvalidStructure,
                "Unexpected empty object array value",
            ));
        }
        let code_int = self.unpacker.unpack_int()?;
        let Some(code) = PklBinaryCode::from_i32(code_int) else {
            return Err(RawError::decode(
                DecodeErrorKind::InvalidStructure,
                format!("Unrecognized code 0x{:x}", code_int as u8),
            ));
        };

        match code {
            PklBinaryCode::Object => self.start_object(len),
            PklBinaryCode::Map => self.start_map(MapKind::Map, PklBinaryCode::Map, "'map", len),
            PklBinaryCode::Mapping => {
                self.start_map(MapKind::Mapping, PklBinaryCode::Mapping, "'mapping", len)
            }
            PklBinaryCode::List => {
                self.start_collection(CollectionKind::List, PklBinaryCode::List, "'list", len)
            }
            PklBinaryCode::Listing => self.start_collection(
                CollectionKind::Listing,
                PklBinaryCode::Listing,
                "'listing",
                len,
            ),
            PklBinaryCode::Set => {
                self.start_collection(CollectionKind::Set, PklBinaryCode::Set, "'set", len)
            }
            PklBinaryCode::Duration => self.decode_duration(len).map(Some),
            PklBinaryCode::DataSize => self.decode_data_size(len).map(Some),
            PklBinaryCode::Pair => self.start_pair(len),
            PklBinaryCode::IntSeq => self.decode_int_seq(len).map(Some),
            PklBinaryCode::Regex => self.decode_regex(len).map(Some),
            PklBinaryCode::Class => self.decode_class(len).map(Some),
            PklBinaryCode::TypeAlias => self.decode_type_alias(len).map(Some),
            PklBinaryCode::Function => self.decode_function(len).map(Some),
            PklBinaryCode::Bytes => self.decode_bytes(len).map(Some),
            PklBinaryCode::Reference if self.options.allow_reference => self.start_reference(len),
            other => Err(RawError::decode(
                DecodeErrorKind::InvalidStructure,
                format!("Unrecognized object code {other}"),
            )),
        }
    }

    fn start_object(&mut self, len: usize) -> Result<Option<DecodedValue>, RawError> {
        self.assert_length(PklBinaryCode::Object, len, 3)?;
        self.curr_path.push(PathSegment::Static("'object"));

        let class_name = self.unpacker.unpack_string()?;
        if is_java_blank(&class_name) {
            return Err(RawError::decode(
                DecodeErrorKind::InvalidStructure,
                "Unexpected blank object class name",
            ));
        }
        let module_uri = self.unpacker.unpack_string()?;
        if is_java_blank(&module_uri) {
            return Err(RawError::decode(
                DecodeErrorKind::InvalidStructure,
                "Unexpected blank object module URI",
            ));
        }
        if self.options.validate_uri {
            validate_java_uri(&module_uri)?;
        }

        let member_count = self.unpacker.unpack_array_header()?;
        let extra_slots = len.saturating_sub(4);
        if member_count == 0 {
            self.unpacker.skip_values(extra_slots)?;
            self.curr_path.pop();
            let class_info = DecodedClassInfo::from_wire(&class_name, &module_uri);
            return Ok(Some(DecodedValue::Object(DecodedObject {
                class_name,
                module_uri,
                class_info,
                members: Vec::new(),
            })));
        }

        let members = Vec::with_capacity(member_count.min(1024));
        self.advance_object_member(
            class_name,
            module_uri,
            member_count,
            0,
            extra_slots,
            members,
        )?;
        Ok(None)
    }

    fn advance_object_member(
        &mut self,
        class_name: PklString,
        module_uri: PklString,
        total: usize,
        idx: usize,
        extra_slots: usize,
        members: Vec<DecodedObjectMember>,
    ) -> Result<(), RawError> {
        self.curr_path.push(PathSegment::Index(idx));
        // Push an active iterator frame before unpacking the member header so
        // any error during header/code unpacking mirrors DecodeIterator.next()'s
        // `finally { currPath.pop(); }`.
        self.frames.push(Frame::Object {
            class_name,
            module_uri,
            total,
            idx,
            extra_slots,
            members,
            step: ObjectMemberStep::EntryKey,
            iter_active: true,
        });

        let member_len = self.unpacker.unpack_array_header()?;
        if member_len != 3 {
            return Err(RawError::decode(
                DecodeErrorKind::InvalidStructure,
                format!("Expected 3 fields in object member, found {member_len}"),
            ));
        }
        let member_code_int = self.unpacker.unpack_int()?;
        let Some(member_code) = PklBinaryCode::from_i32(member_code_int) else {
            return Err(RawError::decode(
                DecodeErrorKind::InvalidStructure,
                format!("Unrecognized code 0x{:x}", member_code_int as u8),
            ));
        };

        let next_step = match member_code {
            PklBinaryCode::Property => {
                let property_name = self.unpacker.unpack_string()?;
                self.curr_path
                    .push(PathSegment::Owned(property_name.clone()));
                ObjectMemberStep::Value {
                    kind: ObjectMemberKind::Property,
                    key: DecodedValue::String(property_name),
                }
            }
            PklBinaryCode::Entry => ObjectMemberStep::EntryKey,
            PklBinaryCode::Element => {
                let element_index = self.unpacker.unpack_long()?;
                self.curr_path.push(PathSegment::Int(element_index));
                ObjectMemberStep::Value {
                    kind: ObjectMemberKind::Element,
                    key: DecodedValue::Int(element_index),
                }
            }
            other => {
                return Err(RawError::decode(
                    DecodeErrorKind::InvalidStructure,
                    format!("Unrecognized member code {other}"),
                ));
            }
        };

        if let Some(Frame::Object { step, .. }) = self.frames.last_mut() {
            *step = next_step;
        }
        Ok(())
    }

    fn start_map(
        &mut self,
        kind: MapKind,
        code: PklBinaryCode,
        path_label: &'static str,
        len: usize,
    ) -> Result<Option<DecodedValue>, RawError> {
        self.assert_length(code, len, 1)?;
        self.curr_path.push(PathSegment::Static(path_label));
        let total = self.unpacker.unpack_map_header()?;
        let extra_slots = len.saturating_sub(2);
        if total == 0 {
            self.unpacker.skip_values(extra_slots)?;
            self.curr_path.pop();
            return Ok(Some(match kind {
                MapKind::Map => DecodedValue::Map(Vec::new()),
                MapKind::Mapping => DecodedValue::Mapping(Vec::new()),
            }));
        }
        self.curr_path.push(PathSegment::Index(0));
        self.frames.push(Frame::Map {
            kind,
            total,
            idx: 0,
            extra_slots,
            entries: Vec::with_capacity(total.min(1024)),
            step: MapStep::Key,
            iter_active: true,
        });
        Ok(None)
    }

    fn start_collection(
        &mut self,
        kind: CollectionKind,
        code: PklBinaryCode,
        path_label: &'static str,
        len: usize,
    ) -> Result<Option<DecodedValue>, RawError> {
        self.assert_length(code, len, 1)?;
        self.curr_path.push(PathSegment::Static(path_label));
        let total = self.unpacker.unpack_array_header()?;
        let extra_slots = len.saturating_sub(2);
        if total == 0 {
            if kind == CollectionKind::Set {
                self.curr_path.pop();
                self.unpacker.skip_values(extra_slots)?;
            } else {
                self.unpacker.skip_values(extra_slots)?;
                self.curr_path.pop();
            }
            return Ok(Some(match kind {
                CollectionKind::List => DecodedValue::List(Vec::new()),
                CollectionKind::Listing => DecodedValue::Listing(Vec::new()),
                CollectionKind::Set => DecodedValue::Set(Vec::new()),
            }));
        }
        self.curr_path.push(PathSegment::Index(0));
        self.frames.push(Frame::Collection {
            kind,
            total,
            idx: 0,
            extra_slots,
            items: Vec::with_capacity(total.min(1024)),
            iter_active: true,
        });
        Ok(None)
    }

    fn start_pair(&mut self, len: usize) -> Result<Option<DecodedValue>, RawError> {
        self.assert_length(PklBinaryCode::Pair, len, 2)?;
        self.curr_path.push(PathSegment::Static("'pair"));
        self.curr_path.push(PathSegment::Static("'first"));
        self.frames.push(Frame::Pair {
            extra_slots: len.saturating_sub(3),
            step: PairStep::First,
        });
        Ok(None)
    }

    fn start_reference(&mut self, len: usize) -> Result<Option<DecodedValue>, RawError> {
        self.assert_length(PklBinaryCode::Reference, len, 3)?;
        self.curr_path.push(PathSegment::Static("'reference"));
        self.curr_path.push(PathSegment::Static("'domain"));
        self.frames.push(Frame::Reference {
            extra_slots: len.saturating_sub(4),
            step: ReferenceStep::Domain,
            iter_active: false,
        });
        Ok(None)
    }

    fn decode_duration(&mut self, len: usize) -> Result<DecodedValue, RawError> {
        self.assert_length(PklBinaryCode::Duration, len, 2)?;
        self.curr_path.push(PathSegment::Static("'duration"));
        let value = self.unpacker.unpack_double()?;
        let raw_unit = self.unpacker.unpack_string()?;
        let Some(unit) = raw_unit.as_str().and_then(DurationUnit::parse) else {
            return Err(RawError::decode(
                DecodeErrorKind::InvalidUnit,
                format!("Invalid Duration unit `{}`", raw_unit.to_java_utf8()),
            ));
        };
        self.unpacker.skip_values(len.saturating_sub(3))?;
        self.curr_path.pop();
        Ok(DecodedValue::Duration(Duration::new(value, unit)))
    }

    fn decode_data_size(&mut self, len: usize) -> Result<DecodedValue, RawError> {
        self.assert_length(PklBinaryCode::DataSize, len, 2)?;
        self.curr_path.push(PathSegment::Static("'datasize"));
        let value = self.unpacker.unpack_double()?;
        let raw_unit = self.unpacker.unpack_string()?;
        let Some(unit) = raw_unit.as_str().and_then(DataSizeUnit::parse) else {
            return Err(RawError::decode(
                DecodeErrorKind::InvalidUnit,
                format!("Invalid DataSize unit `{}`", raw_unit.to_java_utf8()),
            ));
        };
        self.unpacker.skip_values(len.saturating_sub(3))?;
        self.curr_path.pop();
        Ok(DecodedValue::DataSize(DataSize::new(value, unit)))
    }

    fn decode_int_seq(&mut self, len: usize) -> Result<DecodedValue, RawError> {
        self.assert_length(PklBinaryCode::IntSeq, len, 3)?;
        self.curr_path.push(PathSegment::Static("'intseq"));
        let start = self.unpacker.unpack_long()?;
        let end = self.unpacker.unpack_long()?;
        let step = self.unpacker.unpack_long()?;
        if !self.options.allow_int_seq {
            return Err(RawError::decode(
                DecodeErrorKind::UnsupportedType,
                "Cannot decode IntSeq value",
            ));
        }
        self.unpacker.skip_values(len.saturating_sub(4))?;
        self.curr_path.pop();
        Ok(DecodedValue::IntSeq(DecodedIntSeq { start, end, step }))
    }

    fn decode_regex(&mut self, len: usize) -> Result<DecodedValue, RawError> {
        self.assert_length(PklBinaryCode::Regex, len, 1)?;
        self.curr_path.push(PathSegment::Static("'regex"));
        let pattern = self.unpacker.unpack_string()?;
        if self.options.validate_regex {
            let units: Vec<u16> = pattern.encode_utf16().collect();
            if let Err(err) = crate::regex::compile_utf16(&units, "") {
                return Err(RawError::direct(
                    DecodeErrorKind::InvalidRegex,
                    err.message.to_java_utf8(),
                ));
            }
        }
        self.unpacker.skip_values(len.saturating_sub(2))?;
        self.curr_path.pop();
        Ok(DecodedValue::Regex(pattern))
    }

    fn decode_class(&mut self, len: usize) -> Result<DecodedValue, RawError> {
        self.assert_length(PklBinaryCode::Class, len, 2)?;
        self.curr_path.push(PathSegment::Static("'class"));
        let name = self.unpacker.unpack_string()?;
        if is_java_blank(&name) {
            return Err(RawError::decode(
                DecodeErrorKind::InvalidStructure,
                "Unexpected blank class name",
            ));
        }
        let module_uri = self.unpacker.unpack_string()?;
        if is_java_blank(&module_uri) {
            return Err(RawError::decode(
                DecodeErrorKind::InvalidStructure,
                "Unexpected blank class module URI",
            ));
        }
        if self.options.validate_uri {
            validate_java_uri(&module_uri)?;
        }
        if !self.options.allow_class {
            return Err(RawError::decode(
                DecodeErrorKind::UnsupportedType,
                "Cannot decode Class value",
            ));
        }
        self.unpacker.skip_values(len.saturating_sub(3))?;
        self.curr_path.pop();
        Ok(DecodedValue::Class {
            qualified_name: name,
            module_uri,
        })
    }

    fn decode_type_alias(&mut self, len: usize) -> Result<DecodedValue, RawError> {
        self.assert_length(PklBinaryCode::TypeAlias, len, 2)?;
        self.curr_path.push(PathSegment::Static("'typealias"));
        let name = self.unpacker.unpack_string()?;
        if is_java_blank(&name) {
            return Err(RawError::decode(
                DecodeErrorKind::InvalidStructure,
                "Unexpected blank typealias name",
            ));
        }
        let module_uri = self.unpacker.unpack_string()?;
        if is_java_blank(&module_uri) {
            return Err(RawError::decode(
                DecodeErrorKind::InvalidStructure,
                "Unexpected blank typealias module URI",
            ));
        }
        if self.options.validate_uri {
            validate_java_uri(&module_uri)?;
        }
        if !self.options.allow_type_alias {
            return Err(RawError::decode(
                DecodeErrorKind::UnsupportedType,
                "Cannot decode TypeAlias value",
            ));
        }
        self.unpacker.skip_values(len.saturating_sub(3))?;
        self.curr_path.pop();
        Ok(DecodedValue::TypeAlias {
            qualified_name: name,
            module_uri,
        })
    }

    fn decode_function(&mut self, len: usize) -> Result<DecodedValue, RawError> {
        self.assert_length(PklBinaryCode::Function, len, 0)?;
        self.curr_path.push(PathSegment::Static("'function"));
        if !self.options.allow_function {
            return Err(RawError::decode(
                DecodeErrorKind::UnsupportedType,
                "Cannot decode Function value",
            ));
        }
        self.unpacker.skip_values(len.saturating_sub(1))?;
        self.curr_path.pop();
        Ok(DecodedValue::Function)
    }

    fn decode_bytes(&mut self, len: usize) -> Result<DecodedValue, RawError> {
        self.assert_length(PklBinaryCode::Bytes, len, 1)?;
        self.curr_path.push(PathSegment::Static("'bytes"));
        let byte_len = self.unpacker.unpack_binary_header()?;
        let bytes = self.unpacker.read_slice(byte_len)?.to_vec();
        self.unpacker.skip_values(len.saturating_sub(2))?;
        self.curr_path.pop();
        Ok(DecodedValue::Bytes(bytes))
    }
}

/// Matches JDK 21 `StandardCharsets.UTF_8.newDecoder().onMalformedInput(REPLACE)`.
fn decode_java_utf8_replace(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len());
    let mut i = 0;
    let n = bytes.len();
    while i < n {
        let b1 = bytes[i];
        if b1 < 0x80 {
            out.push(char::from(b1));
            i += 1;
        } else if (b1 >> 5) == 0b110 {
            if (b1 & 0x1e) == 0 || i + 1 >= n || (bytes[i + 1] & 0xc0) != 0x80 {
                out.push('\u{fffd}');
                i += 1;
            } else {
                let cp = (u32::from(b1 & 0x1f) << 6) | u32::from(bytes[i + 1] & 0x3f);
                out.push(char::from_u32(cp).unwrap_or('\u{fffd}'));
                i += 2;
            }
        } else if (b1 >> 4) == 0b1110 {
            if i + 1 >= n {
                out.push('\u{fffd}');
                i += 1;
            } else {
                let b2 = bytes[i + 1];
                if (b1 == 0xe0 && (b2 & 0xe0) == 0x80) || (b2 & 0xc0) != 0x80 {
                    out.push('\u{fffd}');
                    i += 1;
                } else if i + 2 >= n || (bytes[i + 2] & 0xc0) != 0x80 {
                    out.push('\u{fffd}');
                    i += 2;
                } else {
                    let cp = (u32::from(b1 & 0x0f) << 12)
                        | (u32::from(b2 & 0x3f) << 6)
                        | u32::from(bytes[i + 2] & 0x3f);
                    // JDK 21 UTF_8$Decoder consumes 3-byte UTF-8 surrogates
                    // (U+D800..=U+DFFF) as one malformed sequence -> single U+FFFD.
                    out.push(char::from_u32(cp).unwrap_or('\u{fffd}'));
                    i += 3;
                }
            }
        } else if (b1 >> 3) == 0b11110 {
            if i + 1 >= n {
                out.push('\u{fffd}');
                i += 1;
            } else {
                let b2 = bytes[i + 1];
                if (b2 & 0xc0) != 0x80
                    || (b1 == 0xf0 && b2 < 0x90)
                    || (b1 == 0xf4 && b2 >= 0x90)
                    || b1 > 0xf4
                {
                    out.push('\u{fffd}');
                    i += 1;
                } else if i + 2 >= n || (bytes[i + 2] & 0xc0) != 0x80 {
                    out.push('\u{fffd}');
                    i += 2;
                } else if i + 3 >= n || (bytes[i + 3] & 0xc0) != 0x80 {
                    out.push('\u{fffd}');
                    i += 3;
                } else {
                    let cp = (u32::from(b1 & 0x07) << 18)
                        | (u32::from(b2 & 0x3f) << 12)
                        | (u32::from(bytes[i + 2] & 0x3f) << 6)
                        | u32::from(bytes[i + 3] & 0x3f);
                    out.push(char::from_u32(cp).unwrap_or('\u{fffd}'));
                    i += 4;
                }
            }
        } else {
            out.push('\u{fffd}');
            i += 1;
        }
    }
    out
}

/// Matches Java `String.isBlank()` (`Character.isWhitespace(int codePoint)`).
fn is_java_blank(text: &PklString) -> bool {
    if let Some(s) = text.as_str() {
        for &b in s.as_bytes() {
            if b > 0x20 && b < 0x7f {
                return false;
            }
            if b >= 0x80 {
                break;
            }
        }
        if s.is_ascii() {
            return s.bytes().all(|b| {
                matches!(
                    b,
                    0x09..=0x0d | 0x1c..=0x20
                )
            });
        }
    }
    let mut iter = text.encode_utf16().peekable();
    while let Some(u) = iter.next() {
        let cp = if (0xd800..=0xdbff).contains(&u)
            && iter
                .peek()
                .is_some_and(|&low| (0xdc00..=0xdfff).contains(&low))
        {
            let low = iter.next().expect("peeked low surrogate");
            0x10000 + ((u32::from(u) - 0xd800) << 10) + (u32::from(low) - 0xdc00)
        } else {
            u32::from(u)
        };
        let is_whitespace = crate::java_regex::character_record(cp)
            .is_some_and(|record| record.predicate_bits & (1 << 23) != 0);
        if !is_whitespace {
            return false;
        }
    }
    true
}

fn validate_java_uri(uri: &PklString) -> Result<(), RawError> {
    if uri.as_str() == Some("pkl:base") {
        return Ok(());
    }
    if let Err(err) = crate::host::resources::glob::validate_opaque_list_uri(uri) {
        let message = if let Some(cause) = err.uri_create_cause.as_ref() {
            match cause.index_utf16 {
                Some(index) => format!(
                    "{} at index {index}: {}",
                    cause.reason,
                    cause.input.to_java_utf8()
                ),
                None => format!("{}: {}", cause.reason, cause.input.to_java_utf8()),
            }
        } else {
            format!("Invalid URI: {}", uri.to_java_utf8())
        };
        return Err(RawError::direct(DecodeErrorKind::InvalidUri, message));
    }
    Ok(())
}
