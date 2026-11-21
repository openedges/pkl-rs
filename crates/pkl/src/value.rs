//! Ergonomic Rust representation of evaluated Pkl values (`PklValue`, `PObject`, `PModule`,
//! `FromPklValue`, and `PklBinaryDecoder`).

use crate::error::{PklError, Result};
use pkl_runtime::pkl_binary_decode::{
    DecodedObject, DecodedValue, ObjectMemberKind, decode as decode_pkl_binary,
};
use std::collections::BTreeMap;

/// Duration value decoded from Pkl (`pkl:base#Duration`).
#[derive(Clone, Debug, PartialEq)]
pub struct PklDuration {
    pub value: f64,
    pub unit: String,
}

/// DataSize value decoded from Pkl (`pkl:base#DataSize`).
#[derive(Clone, Debug, PartialEq)]
pub struct PklDataSize {
    pub value: f64,
    pub unit: String,
}

/// A decoded Pkl typed/dynamic object (`org.pkl.core.PObject`).
#[derive(Clone, Debug, PartialEq)]
pub struct PObject {
    pub class_name: String,
    pub module_uri: String,
    pub properties: BTreeMap<String, PklValue>,
    pub entries: Vec<(PklValue, PklValue)>,
    pub elements: Vec<PklValue>,
}

impl PObject {
    pub fn get(&self, property: &str) -> Option<&PklValue> {
        self.properties.get(property)
    }

    pub fn get_typed<T: FromPklValue>(&self, property: &str) -> Result<T> {
        let val = self.properties.get(property).ok_or_else(|| {
            PklError::decode(format!(
                "Property `{property}` not found on `{}`.",
                self.class_name
            ))
        })?;
        T::from_pkl_value(val)
    }
}

/// A decoded Pkl module (`org.pkl.core.PModule`).
#[derive(Clone, Debug, PartialEq)]
pub struct PModule {
    pub uri: String,
    pub name: String,
    pub properties: BTreeMap<String, PklValue>,
    pub entries: Vec<(PklValue, PklValue)>,
    pub elements: Vec<PklValue>,
}

impl PModule {
    pub fn get(&self, property: &str) -> Option<&PklValue> {
        self.properties.get(property)
    }

    pub fn get_typed<T: FromPklValue>(&self, property: &str) -> Result<T> {
        let val = self.properties.get(property).ok_or_else(|| {
            PklError::decode(format!(
                "Property `{property}` not found on module `{}`.",
                self.name
            ))
        })?;
        T::from_pkl_value(val)
    }
}

/// High-level Rust value decoded from `pkl-binary` MessagePack output.
#[derive(Clone, Debug, PartialEq)]
pub enum PklValue {
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    String(String),
    Bytes(Vec<u8>),
    Duration(PklDuration),
    DataSize(PklDataSize),
    Regex(String),
    Pair(Box<PklValue>, Box<PklValue>),
    List(Vec<PklValue>),
    Set(Vec<PklValue>),
    Listing(Vec<PklValue>),
    Map(Vec<(PklValue, PklValue)>),
    Mapping(Vec<(PklValue, PklValue)>),
    Object(PObject),
    IntSeq {
        start: i64,
        end: i64,
        step: i64,
    },
    Class {
        name: String,
        module_uri: String,
    },
    TypeAlias {
        name: String,
        module_uri: String,
    },
    Function,
    Reference {
        domain: Box<PklValue>,
        data: Box<PklValue>,
        path: Vec<PklValue>,
    },
}

impl PklValue {
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Self::Bool(v) => Some(*v),
            _ => None,
        }
    }

    pub fn as_i64(&self) -> Option<i64> {
        match self {
            Self::Int(v) => Some(*v),
            _ => None,
        }
    }

    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Self::Float(v) => Some(*v),
            Self::Int(v) => Some(*v as f64),
            _ => None,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Self::String(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_bytes(&self) -> Option<&[u8]> {
        match self {
            Self::Bytes(b) => Some(b),
            _ => None,
        }
    }

    pub fn as_object(&self) -> Option<&PObject> {
        match self {
            Self::Object(obj) => Some(obj),
            _ => None,
        }
    }

    pub fn into_module(self) -> Result<PModule> {
        match self {
            Self::Object(obj) => Ok(PModule {
                uri: obj.module_uri,
                name: obj.class_name,
                properties: obj.properties,
                entries: obj.entries,
                elements: obj.elements,
            }),
            other => Err(PklError::decode(format!(
                "Expected module object, got {other:?}."
            ))),
        }
    }

    pub fn from_decoded(val: DecodedValue) -> Self {
        match val {
            DecodedValue::Null => Self::Null,
            DecodedValue::Bool(b) => Self::Bool(b),
            DecodedValue::Int(i) => Self::Int(i),
            DecodedValue::Float(f) => Self::Float(f),
            DecodedValue::String(s) => Self::String(s.to_java_utf8()),
            DecodedValue::Bytes(b) => Self::Bytes(b),
            DecodedValue::Duration(d) => Self::Duration(PklDuration {
                value: d.value(),
                unit: d.unit().symbol().to_string(),
            }),
            DecodedValue::DataSize(d) => Self::DataSize(PklDataSize {
                value: d.value(),
                unit: d.unit().symbol().to_string(),
            }),
            DecodedValue::Regex(r) => Self::Regex(r.to_java_utf8()),
            DecodedValue::Pair(a, b) => Self::Pair(
                Box::new(Self::from_decoded(*a)),
                Box::new(Self::from_decoded(*b)),
            ),
            DecodedValue::List(xs) => Self::List(xs.into_iter().map(Self::from_decoded).collect()),
            DecodedValue::Set(xs) => Self::Set(xs.into_iter().map(Self::from_decoded).collect()),
            DecodedValue::Listing(xs) => {
                Self::Listing(xs.into_iter().map(Self::from_decoded).collect())
            }
            DecodedValue::Map(xs) => Self::Map(
                xs.into_iter()
                    .map(|(k, v)| (Self::from_decoded(k), Self::from_decoded(v)))
                    .collect(),
            ),
            DecodedValue::Mapping(xs) => Self::Mapping(
                xs.into_iter()
                    .map(|(k, v)| (Self::from_decoded(k), Self::from_decoded(v)))
                    .collect(),
            ),
            DecodedValue::Object(obj) => Self::Object(decoded_object_to_pobject(obj)),
            DecodedValue::IntSeq(seq) => Self::IntSeq {
                start: seq.start,
                end: seq.end,
                step: seq.step,
            },
            DecodedValue::Class {
                qualified_name,
                module_uri,
            } => Self::Class {
                name: qualified_name.to_java_utf8(),
                module_uri: module_uri.to_java_utf8(),
            },
            DecodedValue::TypeAlias {
                qualified_name,
                module_uri,
            } => Self::TypeAlias {
                name: qualified_name.to_java_utf8(),
                module_uri: module_uri.to_java_utf8(),
            },
            DecodedValue::Function => Self::Function,
            DecodedValue::Reference(r) => Self::Reference {
                domain: Box::new(Self::from_decoded(*r.domain)),
                data: Box::new(Self::from_decoded(*r.data)),
                path: r.path.into_iter().map(Self::from_decoded).collect(),
            },
        }
    }
}

fn decoded_object_to_pobject(obj: DecodedObject) -> PObject {
    let mut properties = BTreeMap::new();
    let mut entries = Vec::new();
    let mut elements = Vec::new();
    for member in obj.members {
        match member.kind {
            ObjectMemberKind::Property => {
                properties.insert(
                    member.key.to_java_key_string().to_java_utf8(),
                    PklValue::from_decoded(member.value),
                );
            }
            ObjectMemberKind::Entry => {
                entries.push((
                    PklValue::from_decoded(member.key),
                    PklValue::from_decoded(member.value),
                ));
            }
            ObjectMemberKind::Element => {
                elements.push(PklValue::from_decoded(member.value));
            }
        }
    }
    PObject {
        class_name: obj.class_name.to_java_utf8(),
        module_uri: obj.module_uri.to_java_utf8(),
        properties,
        entries,
        elements,
    }
}

/// Decoder for Apple Pkl `pkl-binary` MessagePack payloads (`org.pkl.core.PklBinaryDecoder`).
pub struct PklBinaryDecoder;

impl PklBinaryDecoder {
    pub fn decode(bytes: &[u8]) -> Result<PklValue> {
        let decoded = decode_pkl_binary(bytes)
            .map_err(|err| PklError::decode(format!("Failed to decode pkl-binary: {err}")))?;
        Ok(PklValue::from_decoded(decoded))
    }
}

/// Trait for typed extraction from [`PklValue`], used by [`crate::Evaluator::evaluate_typed`].
pub trait FromPklValue: Sized {
    fn from_pkl_value(value: &PklValue) -> Result<Self>;
}

impl FromPklValue for PklValue {
    fn from_pkl_value(value: &PklValue) -> Result<Self> {
        Ok(value.clone())
    }
}

impl FromPklValue for bool {
    fn from_pkl_value(value: &PklValue) -> Result<Self> {
        value
            .as_bool()
            .ok_or_else(|| PklError::decode(format!("Expected Boolean, got {value:?}.")))
    }
}

impl FromPklValue for i64 {
    fn from_pkl_value(value: &PklValue) -> Result<Self> {
        value
            .as_i64()
            .ok_or_else(|| PklError::decode(format!("Expected Int, got {value:?}.")))
    }
}

impl FromPklValue for i32 {
    fn from_pkl_value(value: &PklValue) -> Result<Self> {
        let v = i64::from_pkl_value(value)?;
        i32::try_from(v).map_err(|_| PklError::decode(format!("Int {v} out of range for i32.")))
    }
}

impl FromPklValue for u64 {
    fn from_pkl_value(value: &PklValue) -> Result<Self> {
        let v = i64::from_pkl_value(value)?;
        u64::try_from(v).map_err(|_| PklError::decode(format!("Int {v} out of range for u64.")))
    }
}

impl FromPklValue for usize {
    fn from_pkl_value(value: &PklValue) -> Result<Self> {
        let v = i64::from_pkl_value(value)?;
        usize::try_from(v).map_err(|_| PklError::decode(format!("Int {v} out of range for usize.")))
    }
}

impl FromPklValue for f64 {
    fn from_pkl_value(value: &PklValue) -> Result<Self> {
        value
            .as_f64()
            .ok_or_else(|| PklError::decode(format!("Expected Float or Int, got {value:?}.")))
    }
}

impl FromPklValue for String {
    fn from_pkl_value(value: &PklValue) -> Result<Self> {
        value
            .as_str()
            .map(ToOwned::to_owned)
            .ok_or_else(|| PklError::decode(format!("Expected String, got {value:?}.")))
    }
}

impl FromPklValue for Vec<u8> {
    fn from_pkl_value(value: &PklValue) -> Result<Self> {
        value
            .as_bytes()
            .map(|b| b.to_vec())
            .ok_or_else(|| PklError::decode(format!("Expected Bytes, got {value:?}.")))
    }
}

impl<T: FromPklValue> FromPklValue for Option<T> {
    fn from_pkl_value(value: &PklValue) -> Result<Self> {
        match value {
            PklValue::Null => Ok(None),
            other => Ok(Some(T::from_pkl_value(other)?)),
        }
    }
}

impl<T: FromPklValue> FromPklValue for Vec<T> {
    fn from_pkl_value(value: &PklValue) -> Result<Self> {
        match value {
            PklValue::List(xs) | PklValue::Listing(xs) | PklValue::Set(xs) => {
                xs.iter().map(T::from_pkl_value).collect()
            }
            other => Err(PklError::decode(format!(
                "Expected List/Listing/Set, got {other:?}."
            ))),
        }
    }
}

impl<T: FromPklValue> FromPklValue for BTreeMap<String, T> {
    fn from_pkl_value(value: &PklValue) -> Result<Self> {
        match value {
            PklValue::Map(entries) | PklValue::Mapping(entries) => {
                let mut out = BTreeMap::new();
                for (k, v) in entries {
                    let ks = String::from_pkl_value(k)?;
                    let vs = T::from_pkl_value(v)?;
                    out.insert(ks, vs);
                }
                Ok(out)
            }
            PklValue::Object(obj) => {
                let mut out = BTreeMap::new();
                for (k, v) in &obj.properties {
                    out.insert(k.clone(), T::from_pkl_value(v)?);
                }
                Ok(out)
            }
            other => Err(PklError::decode(format!(
                "Expected Map/Mapping/Object, got {other:?}."
            ))),
        }
    }
}

impl FromPklValue for PObject {
    fn from_pkl_value(value: &PklValue) -> Result<Self> {
        match value {
            PklValue::Object(obj) => Ok(obj.clone()),
            other => Err(PklError::decode(format!("Expected Object, got {other:?}."))),
        }
    }
}

impl FromPklValue for PModule {
    fn from_pkl_value(value: &PklValue) -> Result<Self> {
        value.clone().into_module()
    }
}
