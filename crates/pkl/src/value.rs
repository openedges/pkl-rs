//! Ergonomic Rust representation of evaluated Pkl values (`PklValue`, `PObject`, `PModule`,
//! `FromPklValue`, and `PklBinaryDecoder`).

use crate::error::{PklError, Result};
use pkl_runtime::pkl_binary_decode::{
    DecodedValue, ObjectMemberKind, decode as decode_pkl_binary,
    decode_lossless as decode_pkl_binary_lossless,
};
use std::collections::BTreeMap;
use std::fmt;

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
pub struct PObject {
    pub class_name: String,
    pub module_uri: String,
    pub properties: BTreeMap<String, PklValue>,
    pub entries: Vec<(PklValue, PklValue)>,
    pub elements: Vec<PklValue>,
}

impl Drop for PObject {
    fn drop(&mut self) {
        let mut stack = Vec::new();
        drain_pobject_children(self, &mut stack);
        drain_pkl_value_stack(stack);
    }
}

impl Clone for PObject {
    fn clone(&self) -> Self {
        clone_pobject(self)
    }
}

impl PartialEq for PObject {
    fn eq(&self, other: &Self) -> bool {
        eq_pobject(self, other)
    }
}

impl fmt::Debug for PObject {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        debug_pobject(self, f, 0)
    }
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
pub struct PModule {
    pub uri: String,
    pub name: String,
    pub properties: BTreeMap<String, PklValue>,
    pub entries: Vec<(PklValue, PklValue)>,
    pub elements: Vec<PklValue>,
}

impl Drop for PModule {
    fn drop(&mut self) {
        let mut stack = Vec::new();
        stack.extend(std::mem::take(&mut self.properties).into_values());
        for (k, v) in std::mem::take(&mut self.entries) {
            stack.push(k);
            stack.push(v);
        }
        stack.extend(std::mem::take(&mut self.elements));
        drain_pkl_value_stack(stack);
    }
}

impl Clone for PModule {
    fn clone(&self) -> Self {
        Self {
            uri: self.uri.clone(),
            name: self.name.clone(),
            properties: self
                .properties
                .iter()
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect(),
            entries: self
                .entries
                .iter()
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect(),
            elements: self.elements.to_vec(),
        }
    }
}

impl PartialEq for PModule {
    fn eq(&self, other: &Self) -> bool {
        self.uri == other.uri
            && self.name == other.name
            && self.properties == other.properties
            && self.entries == other.entries
            && self.elements == other.elements
    }
}

impl fmt::Debug for PModule {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PModule")
            .field("uri", &self.uri)
            .field("name", &self.name)
            .field("properties", &DebugMap(&self.properties, 1))
            .field("entries", &DebugPairs(&self.entries, 1))
            .field("elements", &DebugSlice(&self.elements, 1))
            .finish()
    }
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

/// Owned byte buffer wrapper decoded from Pkl `Bytes` (`pkl:base#Bytes`).
///
/// Using a distinct wrapper avoids coherence ambiguity with `Vec<u8>` (which decodes Pkl
/// `List<UInt8>` / `Listing<UInt8>` via the blanket `FromPklValue` impl for `Vec<T>`).
#[derive(Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PklBytes(pub Vec<u8>);

impl PklBytes {
    /// Returns the underlying byte vector.
    pub fn into_inner(self) -> Vec<u8> {
        self.0
    }
}

impl std::ops::Deref for PklBytes {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl AsRef<[u8]> for PklBytes {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

impl From<Vec<u8>> for PklBytes {
    fn from(bytes: Vec<u8>) -> Self {
        Self(bytes)
    }
}

impl From<PklBytes> for Vec<u8> {
    fn from(bytes: PklBytes) -> Self {
        bytes.0
    }
}

/// High-level Rust value decoded from `pkl-binary` MessagePack output.
#[non_exhaustive]
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

const MAX_PKL_VALUE_DEBUG_DEPTH: usize = 64;

fn drain_pobject_children(obj: &mut PObject, stack: &mut Vec<PklValue>) {
    stack.extend(std::mem::take(&mut obj.properties).into_values());
    for (k, v) in std::mem::take(&mut obj.entries) {
        stack.push(k);
        stack.push(v);
    }
    stack.extend(std::mem::take(&mut obj.elements));
}

fn drain_pkl_value_children(val: &mut PklValue, stack: &mut Vec<PklValue>) {
    match val {
        PklValue::Pair(a, b) => {
            stack.push(std::mem::replace(a.as_mut(), PklValue::Null));
            stack.push(std::mem::replace(b.as_mut(), PklValue::Null));
        }
        PklValue::List(items) | PklValue::Set(items) | PklValue::Listing(items) => {
            stack.extend(std::mem::take(items));
        }
        PklValue::Map(pairs) | PklValue::Mapping(pairs) => {
            for (k, v) in std::mem::take(pairs) {
                stack.push(k);
                stack.push(v);
            }
        }
        PklValue::Object(obj) => drain_pobject_children(obj, stack),
        PklValue::Reference { domain, data, path } => {
            stack.push(std::mem::replace(domain.as_mut(), PklValue::Null));
            stack.push(std::mem::replace(data.as_mut(), PklValue::Null));
            stack.extend(std::mem::take(path));
        }
        _ => {}
    }
}

fn drain_pkl_value_stack(mut stack: Vec<PklValue>) {
    while let Some(mut item) = stack.pop() {
        drain_pkl_value_children(&mut item, &mut stack);
    }
}

impl Drop for PklValue {
    fn drop(&mut self) {
        let mut stack = Vec::new();
        drain_pkl_value_children(self, &mut stack);
        drain_pkl_value_stack(stack);
    }
}

fn clone_pobject(obj: &PObject) -> PObject {
    let mut work: Vec<CloneTask<'_>> = Vec::new();
    let mut out: Vec<PklValue> = Vec::new();
    push_clone_pobject(obj, &mut work);
    run_clone_work(work, &mut out);
    let mut popped = out.pop().expect("cloned PObject");
    match &mut popped {
        PklValue::Object(cloned) => std::mem::replace(
            cloned,
            PObject {
                class_name: String::new(),
                module_uri: String::new(),
                properties: BTreeMap::new(),
                entries: Vec::new(),
                elements: Vec::new(),
            },
        ),
        _ => unreachable!(),
    }
}

enum CloneTask<'a> {
    Visit(&'a PklValue),
    Pair,
    List(usize),
    Set(usize),
    Listing(usize),
    Map(usize),
    Mapping(usize),
    Object {
        class_name: String,
        module_uri: String,
        prop_keys: Vec<String>,
        entries_len: usize,
        elements_len: usize,
    },
    Reference {
        path_len: usize,
    },
}

fn push_clone_pobject<'a>(obj: &'a PObject, work: &mut Vec<CloneTask<'a>>) {
    let prop_keys: Vec<String> = obj.properties.keys().cloned().collect();
    work.push(CloneTask::Object {
        class_name: obj.class_name.clone(),
        module_uri: obj.module_uri.clone(),
        prop_keys,
        entries_len: obj.entries.len(),
        elements_len: obj.elements.len(),
    });
    for elem in obj.elements.iter().rev() {
        work.push(CloneTask::Visit(elem));
    }
    for (k, v) in obj.entries.iter().rev() {
        work.push(CloneTask::Visit(v));
        work.push(CloneTask::Visit(k));
    }
    for v in obj.properties.values().rev() {
        work.push(CloneTask::Visit(v));
    }
}

fn run_clone_work<'a>(mut work: Vec<CloneTask<'a>>, out: &mut Vec<PklValue>) {
    while let Some(task) = work.pop() {
        match task {
            CloneTask::Visit(val) => match val {
                PklValue::Null => out.push(PklValue::Null),
                PklValue::Bool(b) => out.push(PklValue::Bool(*b)),
                PklValue::Int(i) => out.push(PklValue::Int(*i)),
                PklValue::Float(f) => out.push(PklValue::Float(*f)),
                PklValue::String(s) => out.push(PklValue::String(s.clone())),
                PklValue::Bytes(b) => out.push(PklValue::Bytes(b.clone())),
                PklValue::Duration(d) => out.push(PklValue::Duration(d.clone())),
                PklValue::DataSize(d) => out.push(PklValue::DataSize(d.clone())),
                PklValue::Regex(r) => out.push(PklValue::Regex(r.clone())),
                PklValue::IntSeq { start, end, step } => out.push(PklValue::IntSeq {
                    start: *start,
                    end: *end,
                    step: *step,
                }),
                PklValue::Class { name, module_uri } => out.push(PklValue::Class {
                    name: name.clone(),
                    module_uri: module_uri.clone(),
                }),
                PklValue::TypeAlias { name, module_uri } => out.push(PklValue::TypeAlias {
                    name: name.clone(),
                    module_uri: module_uri.clone(),
                }),
                PklValue::Function => out.push(PklValue::Function),
                PklValue::Pair(a, b) => {
                    work.push(CloneTask::Pair);
                    work.push(CloneTask::Visit(b));
                    work.push(CloneTask::Visit(a));
                }
                PklValue::List(items) => {
                    work.push(CloneTask::List(items.len()));
                    work.extend(items.iter().rev().map(CloneTask::Visit));
                }
                PklValue::Set(items) => {
                    work.push(CloneTask::Set(items.len()));
                    work.extend(items.iter().rev().map(CloneTask::Visit));
                }
                PklValue::Listing(items) => {
                    work.push(CloneTask::Listing(items.len()));
                    work.extend(items.iter().rev().map(CloneTask::Visit));
                }
                PklValue::Map(pairs) => {
                    work.push(CloneTask::Map(pairs.len()));
                    for (k, v) in pairs.iter().rev() {
                        work.push(CloneTask::Visit(v));
                        work.push(CloneTask::Visit(k));
                    }
                }
                PklValue::Mapping(pairs) => {
                    work.push(CloneTask::Mapping(pairs.len()));
                    for (k, v) in pairs.iter().rev() {
                        work.push(CloneTask::Visit(v));
                        work.push(CloneTask::Visit(k));
                    }
                }
                PklValue::Object(obj) => push_clone_pobject(obj, &mut work),
                PklValue::Reference { domain, data, path } => {
                    work.push(CloneTask::Reference {
                        path_len: path.len(),
                    });
                    work.extend(path.iter().rev().map(CloneTask::Visit));
                    work.push(CloneTask::Visit(data));
                    work.push(CloneTask::Visit(domain));
                }
            },
            CloneTask::Pair => {
                let b = out.pop().expect("pair second");
                let a = out.pop().expect("pair first");
                out.push(PklValue::Pair(Box::new(a), Box::new(b)));
            }
            CloneTask::List(n) => {
                let start = out.len() - n;
                let items = out.drain(start..).collect();
                out.push(PklValue::List(items));
            }
            CloneTask::Set(n) => {
                let start = out.len() - n;
                let items = out.drain(start..).collect();
                out.push(PklValue::Set(items));
            }
            CloneTask::Listing(n) => {
                let start = out.len() - n;
                let items = out.drain(start..).collect();
                out.push(PklValue::Listing(items));
            }
            CloneTask::Map(n) => {
                let start = out.len() - n * 2;
                let pairs = {
                    let mut pairs = Vec::with_capacity(n);
                    let mut iter = out.drain(start..);
                    while let (Some(k), Some(v)) = (iter.next(), iter.next()) {
                        pairs.push((k, v));
                    }
                    pairs
                };
                out.push(PklValue::Map(pairs));
            }
            CloneTask::Mapping(n) => {
                let start = out.len() - n * 2;
                let pairs = {
                    let mut pairs = Vec::with_capacity(n);
                    let mut iter = out.drain(start..);
                    while let (Some(k), Some(v)) = (iter.next(), iter.next()) {
                        pairs.push((k, v));
                    }
                    pairs
                };
                out.push(PklValue::Mapping(pairs));
            }
            CloneTask::Object {
                class_name,
                module_uri,
                prop_keys,
                entries_len,
                elements_len,
            } => {
                let child_count = prop_keys.len() + entries_len * 2 + elements_len;
                let start = out.len() - child_count;
                let (properties, entries, elements) = {
                    let mut iter = out.drain(start..);
                    let mut properties = BTreeMap::new();
                    for k in prop_keys {
                        properties.insert(k, iter.next().expect("property value"));
                    }
                    let mut entries = Vec::with_capacity(entries_len);
                    for _ in 0..entries_len {
                        let k = iter.next().expect("entry key");
                        let v = iter.next().expect("entry value");
                        entries.push((k, v));
                    }
                    let elements: Vec<PklValue> = iter.collect();
                    (properties, entries, elements)
                };
                out.push(PklValue::Object(PObject {
                    class_name,
                    module_uri,
                    properties,
                    entries,
                    elements,
                }));
            }
            CloneTask::Reference { path_len } => {
                let start = out.len() - (2 + path_len);
                let (domain, data, path) = {
                    let mut iter = out.drain(start..);
                    let domain = iter.next().expect("ref domain");
                    let data = iter.next().expect("ref data");
                    let path = iter.collect();
                    (domain, data, path)
                };
                out.push(PklValue::Reference {
                    domain: Box::new(domain),
                    data: Box::new(data),
                    path,
                });
            }
        }
    }
}

impl Clone for PklValue {
    fn clone(&self) -> Self {
        let mut out = Vec::new();
        run_clone_work(vec![CloneTask::Visit(self)], &mut out);
        out.pop().expect("cloned PklValue")
    }
}

fn eq_pobject(a: &PObject, b: &PObject) -> bool {
    if a.class_name != b.class_name
        || a.module_uri != b.module_uri
        || a.properties.len() != b.properties.len()
        || a.entries.len() != b.entries.len()
        || a.elements.len() != b.elements.len()
    {
        return false;
    }
    let mut stack = Vec::new();
    for ((ka, va), (kb, vb)) in a.properties.iter().zip(b.properties.iter()) {
        if ka != kb {
            return false;
        }
        stack.push((va, vb));
    }
    for ((ka, va), (kb, vb)) in a.entries.iter().zip(b.entries.iter()) {
        stack.push((ka, kb));
        stack.push((va, vb));
    }
    for (ea, eb) in a.elements.iter().zip(b.elements.iter()) {
        stack.push((ea, eb));
    }
    eq_pkl_value_stack(stack)
}

fn eq_pkl_value_stack(mut stack: Vec<(&PklValue, &PklValue)>) -> bool {
    while let Some((lhs, rhs)) = stack.pop() {
        match (lhs, rhs) {
            (PklValue::Null, PklValue::Null) | (PklValue::Function, PklValue::Function) => {}
            (PklValue::Bool(a), PklValue::Bool(b)) if a == b => {}
            (PklValue::Int(a), PklValue::Int(b)) if a == b => {}
            (PklValue::Float(a), PklValue::Float(b)) if a == b => {}
            (PklValue::String(a), PklValue::String(b)) if a == b => {}
            (PklValue::Bytes(a), PklValue::Bytes(b)) if a == b => {}
            (PklValue::Duration(a), PklValue::Duration(b)) if a == b => {}
            (PklValue::DataSize(a), PklValue::DataSize(b)) if a == b => {}
            (PklValue::Regex(a), PklValue::Regex(b)) if a == b => {}
            (
                PklValue::IntSeq {
                    start: s1,
                    end: e1,
                    step: st1,
                },
                PklValue::IntSeq {
                    start: s2,
                    end: e2,
                    step: st2,
                },
            ) if s1 == s2 && e1 == e2 && st1 == st2 => {}
            (
                PklValue::Class {
                    name: n1,
                    module_uri: u1,
                },
                PklValue::Class {
                    name: n2,
                    module_uri: u2,
                },
            )
            | (
                PklValue::TypeAlias {
                    name: n1,
                    module_uri: u1,
                },
                PklValue::TypeAlias {
                    name: n2,
                    module_uri: u2,
                },
            ) if n1 == n2 && u1 == u2 => {}
            (PklValue::Pair(a1, b1), PklValue::Pair(a2, b2)) => {
                stack.push((b1, b2));
                stack.push((a1, a2));
            }
            (PklValue::List(a), PklValue::List(b))
            | (PklValue::Set(a), PklValue::Set(b))
            | (PklValue::Listing(a), PklValue::Listing(b))
                if a.len() == b.len() =>
            {
                for (ea, eb) in a.iter().zip(b.iter()) {
                    stack.push((ea, eb));
                }
            }
            (PklValue::Map(a), PklValue::Map(b)) | (PklValue::Mapping(a), PklValue::Mapping(b))
                if a.len() == b.len() =>
            {
                for ((ka, va), (kb, vb)) in a.iter().zip(b.iter()) {
                    stack.push((ka, kb));
                    stack.push((va, vb));
                }
            }
            (PklValue::Object(a), PklValue::Object(b))
                if a.class_name == b.class_name
                    && a.module_uri == b.module_uri
                    && a.properties.len() == b.properties.len()
                    && a.entries.len() == b.entries.len()
                    && a.elements.len() == b.elements.len() =>
            {
                for ((ka, va), (kb, vb)) in a.properties.iter().zip(b.properties.iter()) {
                    if ka != kb {
                        return false;
                    }
                    stack.push((va, vb));
                }
                for ((ka, va), (kb, vb)) in a.entries.iter().zip(b.entries.iter()) {
                    stack.push((ka, kb));
                    stack.push((va, vb));
                }
                for (ea, eb) in a.elements.iter().zip(b.elements.iter()) {
                    stack.push((ea, eb));
                }
            }
            (
                PklValue::Reference {
                    domain: d1,
                    data: dt1,
                    path: p1,
                },
                PklValue::Reference {
                    domain: d2,
                    data: dt2,
                    path: p2,
                },
            ) if p1.len() == p2.len() => {
                stack.push((d1, d2));
                stack.push((dt1, dt2));
                for (pa, pb) in p1.iter().zip(p2.iter()) {
                    stack.push((pa, pb));
                }
            }
            _ => return false,
        }
    }
    true
}

impl PartialEq for PklValue {
    fn eq(&self, other: &Self) -> bool {
        eq_pkl_value_stack(vec![(self, other)])
    }
}

struct DebugVal<'a>(&'a PklValue, usize);
impl fmt::Debug for DebugVal<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        debug_pkl_value(self.0, f, self.1)
    }
}

struct DebugSlice<'a>(&'a [PklValue], usize);
impl fmt::Debug for DebugSlice<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.1 > MAX_PKL_VALUE_DEBUG_DEPTH {
            return f.write_str("[...]");
        }
        let mut list = f.debug_list();
        for item in self.0 {
            list.entry(&DebugVal(item, self.1));
        }
        list.finish()
    }
}

struct DebugPairs<'a>(&'a [(PklValue, PklValue)], usize);
impl fmt::Debug for DebugPairs<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.1 > MAX_PKL_VALUE_DEBUG_DEPTH {
            return f.write_str("[...]");
        }
        let mut list = f.debug_list();
        for (k, v) in self.0 {
            list.entry(&(DebugVal(k, self.1), DebugVal(v, self.1)));
        }
        list.finish()
    }
}

struct DebugMap<'a>(&'a BTreeMap<String, PklValue>, usize);
impl fmt::Debug for DebugMap<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.1 > MAX_PKL_VALUE_DEBUG_DEPTH {
            return f.write_str("{...}");
        }
        let mut map = f.debug_map();
        for (k, v) in self.0 {
            map.entry(k, &DebugVal(v, self.1));
        }
        map.finish()
    }
}

fn debug_pobject(obj: &PObject, f: &mut fmt::Formatter<'_>, depth: usize) -> fmt::Result {
    if depth > MAX_PKL_VALUE_DEBUG_DEPTH {
        return f.write_str("PObject { ... }");
    }
    f.debug_struct("PObject")
        .field("class_name", &obj.class_name)
        .field("module_uri", &obj.module_uri)
        .field("properties", &DebugMap(&obj.properties, depth + 1))
        .field("entries", &DebugPairs(&obj.entries, depth + 1))
        .field("elements", &DebugSlice(&obj.elements, depth + 1))
        .finish()
}

fn debug_pkl_value(val: &PklValue, f: &mut fmt::Formatter<'_>, depth: usize) -> fmt::Result {
    if depth > MAX_PKL_VALUE_DEBUG_DEPTH {
        return f.write_str("...");
    }
    match val {
        PklValue::Null => f.write_str("Null"),
        PklValue::Bool(v) => f.debug_tuple("Bool").field(v).finish(),
        PklValue::Int(v) => f.debug_tuple("Int").field(v).finish(),
        PklValue::Float(v) => f.debug_tuple("Float").field(v).finish(),
        PklValue::String(v) => f.debug_tuple("String").field(v).finish(),
        PklValue::Bytes(v) => f.debug_tuple("Bytes").field(v).finish(),
        PklValue::Duration(v) => f.debug_tuple("Duration").field(v).finish(),
        PklValue::DataSize(v) => f.debug_tuple("DataSize").field(v).finish(),
        PklValue::Regex(v) => f.debug_tuple("Regex").field(v).finish(),
        PklValue::Pair(a, b) => f
            .debug_tuple("Pair")
            .field(&DebugVal(a, depth + 1))
            .field(&DebugVal(b, depth + 1))
            .finish(),
        PklValue::List(items) => f
            .debug_tuple("List")
            .field(&DebugSlice(items, depth + 1))
            .finish(),
        PklValue::Set(items) => f
            .debug_tuple("Set")
            .field(&DebugSlice(items, depth + 1))
            .finish(),
        PklValue::Listing(items) => f
            .debug_tuple("Listing")
            .field(&DebugSlice(items, depth + 1))
            .finish(),
        PklValue::Map(pairs) => f
            .debug_tuple("Map")
            .field(&DebugPairs(pairs, depth + 1))
            .finish(),
        PklValue::Mapping(pairs) => f
            .debug_tuple("Mapping")
            .field(&DebugPairs(pairs, depth + 1))
            .finish(),
        PklValue::Object(obj) => {
            struct DebugObj<'a>(&'a PObject, usize);
            impl fmt::Debug for DebugObj<'_> {
                fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                    debug_pobject(self.0, f, self.1)
                }
            }
            f.debug_tuple("Object")
                .field(&DebugObj(obj, depth + 1))
                .finish()
        }
        PklValue::IntSeq { start, end, step } => f
            .debug_struct("IntSeq")
            .field("start", start)
            .field("end", end)
            .field("step", step)
            .finish(),
        PklValue::Class { name, module_uri } => f
            .debug_struct("Class")
            .field("name", name)
            .field("module_uri", module_uri)
            .finish(),
        PklValue::TypeAlias { name, module_uri } => f
            .debug_struct("TypeAlias")
            .field("name", name)
            .field("module_uri", module_uri)
            .finish(),
        PklValue::Function => f.write_str("Function"),
        PklValue::Reference { domain, data, path } => f
            .debug_struct("Reference")
            .field("domain", &DebugVal(domain, depth + 1))
            .field("data", &DebugVal(data, depth + 1))
            .field("path", &DebugSlice(path, depth + 1))
            .finish(),
    }
}

impl fmt::Debug for PklValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        debug_pkl_value(self, f, 0)
    }
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

    pub fn into_module(mut self) -> Result<PModule> {
        match &mut self {
            Self::Object(obj) => Ok(PModule {
                uri: std::mem::take(&mut obj.module_uri),
                name: std::mem::take(&mut obj.class_name),
                properties: std::mem::take(&mut obj.properties),
                entries: std::mem::take(&mut obj.entries),
                elements: std::mem::take(&mut obj.elements),
            }),
            other => Err(PklError::decode(format!(
                "Expected module object, got {other:?}."
            ))),
        }
    }

    pub(crate) fn from_decoded(val: DecodedValue) -> Self {
        enum ObjMemberTag {
            Property(String),
            Entry,
            Element,
        }
        enum Frame {
            Visit(DecodedValue),
            Pair,
            List(usize),
            Set(usize),
            Listing(usize),
            Map(usize),
            Mapping(usize),
            Object {
                class_name: String,
                module_uri: String,
                members: Vec<ObjMemberTag>,
            },
            Reference {
                path_len: usize,
            },
        }
        let mut work = vec![Frame::Visit(val)];
        let mut out: Vec<PklValue> = Vec::new();

        while let Some(frame) = work.pop() {
            match frame {
                Frame::Visit(mut cur) => match &mut cur {
                    DecodedValue::Null => out.push(Self::Null),
                    DecodedValue::Bool(b) => out.push(Self::Bool(*b)),
                    DecodedValue::Int(i) => out.push(Self::Int(*i)),
                    DecodedValue::Float(f) => out.push(Self::Float(*f)),
                    DecodedValue::String(s) => out.push(Self::String(s.to_java_utf8())),
                    DecodedValue::Bytes(b) => out.push(Self::Bytes(std::mem::take(b))),
                    DecodedValue::Duration(d) => out.push(Self::Duration(PklDuration {
                        value: d.value(),
                        unit: d.unit().symbol().to_string(),
                    })),
                    DecodedValue::DataSize(d) => out.push(Self::DataSize(PklDataSize {
                        value: d.value(),
                        unit: d.unit().symbol().to_string(),
                    })),
                    DecodedValue::Regex(r) => out.push(Self::Regex(r.to_java_utf8())),
                    DecodedValue::IntSeq(seq) => out.push(Self::IntSeq {
                        start: seq.start,
                        end: seq.end,
                        step: seq.step,
                    }),
                    DecodedValue::Class {
                        qualified_name,
                        module_uri,
                    } => out.push(Self::Class {
                        name: qualified_name.to_java_utf8(),
                        module_uri: module_uri.to_java_utf8(),
                    }),
                    DecodedValue::TypeAlias {
                        qualified_name,
                        module_uri,
                    } => out.push(Self::TypeAlias {
                        name: qualified_name.to_java_utf8(),
                        module_uri: module_uri.to_java_utf8(),
                    }),
                    DecodedValue::Function => out.push(Self::Function),
                    DecodedValue::Pair(a, b) => {
                        let a_val = std::mem::replace(a.as_mut(), DecodedValue::Null);
                        let b_val = std::mem::replace(b.as_mut(), DecodedValue::Null);
                        work.push(Frame::Pair);
                        work.push(Frame::Visit(b_val));
                        work.push(Frame::Visit(a_val));
                    }
                    DecodedValue::List(xs) => {
                        let items = std::mem::take(xs);
                        work.push(Frame::List(items.len()));
                        for item in items.into_iter().rev() {
                            work.push(Frame::Visit(item));
                        }
                    }
                    DecodedValue::Set(xs) => {
                        let items = std::mem::take(xs);
                        work.push(Frame::Set(items.len()));
                        for item in items.into_iter().rev() {
                            work.push(Frame::Visit(item));
                        }
                    }
                    DecodedValue::Listing(xs) => {
                        let items = std::mem::take(xs);
                        work.push(Frame::Listing(items.len()));
                        for item in items.into_iter().rev() {
                            work.push(Frame::Visit(item));
                        }
                    }
                    DecodedValue::Map(xs) => {
                        let pairs = std::mem::take(xs);
                        work.push(Frame::Map(pairs.len()));
                        for (k, v) in pairs.into_iter().rev() {
                            work.push(Frame::Visit(v));
                            work.push(Frame::Visit(k));
                        }
                    }
                    DecodedValue::Mapping(xs) => {
                        let pairs = std::mem::take(xs);
                        work.push(Frame::Mapping(pairs.len()));
                        for (k, v) in pairs.into_iter().rev() {
                            work.push(Frame::Visit(v));
                            work.push(Frame::Visit(k));
                        }
                    }
                    DecodedValue::Object(obj) => {
                        let class_name = obj.class_name.to_java_utf8();
                        let module_uri = obj.module_uri.to_java_utf8();
                        let raw_members = std::mem::take(&mut obj.members);
                        let mut tags = Vec::with_capacity(raw_members.len());
                        let mut children = Vec::with_capacity(raw_members.len());
                        for member in raw_members {
                            match member.kind {
                                ObjectMemberKind::Property => {
                                    tags.push(ObjMemberTag::Property(
                                        member.key.to_java_key_string().to_java_utf8(),
                                    ));
                                    children.push(member.value);
                                }
                                ObjectMemberKind::Entry => {
                                    tags.push(ObjMemberTag::Entry);
                                    children.push(member.key);
                                    children.push(member.value);
                                }
                                ObjectMemberKind::Element => {
                                    tags.push(ObjMemberTag::Element);
                                    children.push(member.value);
                                }
                            }
                        }
                        work.push(Frame::Object {
                            class_name,
                            module_uri,
                            members: tags,
                        });
                        for child in children.into_iter().rev() {
                            work.push(Frame::Visit(child));
                        }
                    }
                    DecodedValue::Reference(r) => {
                        let domain = std::mem::replace(r.domain.as_mut(), DecodedValue::Null);
                        let data = std::mem::replace(r.data.as_mut(), DecodedValue::Null);
                        let path = std::mem::take(&mut r.path);
                        work.push(Frame::Reference {
                            path_len: path.len(),
                        });
                        for seg in path.into_iter().rev() {
                            work.push(Frame::Visit(seg));
                        }
                        work.push(Frame::Visit(data));
                        work.push(Frame::Visit(domain));
                    }
                },
                Frame::Pair => {
                    let b = out.pop().expect("pair second");
                    let a = out.pop().expect("pair first");
                    out.push(Self::Pair(Box::new(a), Box::new(b)));
                }
                Frame::List(n) => {
                    let start = out.len() - n;
                    let items = out.drain(start..).collect();
                    out.push(Self::List(items));
                }
                Frame::Set(n) => {
                    let start = out.len() - n;
                    let items = out.drain(start..).collect();
                    out.push(Self::Set(items));
                }
                Frame::Listing(n) => {
                    let start = out.len() - n;
                    let items = out.drain(start..).collect();
                    out.push(Self::Listing(items));
                }
                Frame::Map(n) => {
                    let start = out.len() - n * 2;
                    let pairs = {
                        let mut iter = out.drain(start..);
                        let mut pairs = Vec::with_capacity(n);
                        while let (Some(k), Some(v)) = (iter.next(), iter.next()) {
                            pairs.push((k, v));
                        }
                        pairs
                    };
                    out.push(Self::Map(pairs));
                }
                Frame::Mapping(n) => {
                    let start = out.len() - n * 2;
                    let pairs = {
                        let mut iter = out.drain(start..);
                        let mut pairs = Vec::with_capacity(n);
                        while let (Some(k), Some(v)) = (iter.next(), iter.next()) {
                            pairs.push((k, v));
                        }
                        pairs
                    };
                    out.push(Self::Mapping(pairs));
                }
                Frame::Object {
                    class_name,
                    module_uri,
                    members,
                } => {
                    let child_count: usize = members
                        .iter()
                        .map(|t| match t {
                            ObjMemberTag::Entry => 2,
                            ObjMemberTag::Property(_) | ObjMemberTag::Element => 1,
                        })
                        .sum();
                    let start = out.len() - child_count;
                    let (properties, entries, elements) = {
                        let mut iter = out.drain(start..);
                        let mut properties = BTreeMap::new();
                        let mut entries = Vec::new();
                        let mut elements = Vec::new();
                        for tag in members {
                            match tag {
                                ObjMemberTag::Property(name) => {
                                    properties.insert(name, iter.next().expect("property value"));
                                }
                                ObjMemberTag::Entry => {
                                    let k = iter.next().expect("entry key");
                                    let v = iter.next().expect("entry value");
                                    entries.push((k, v));
                                }
                                ObjMemberTag::Element => {
                                    elements.push(iter.next().expect("element value"));
                                }
                            }
                        }
                        (properties, entries, elements)
                    };
                    out.push(Self::Object(PObject {
                        class_name,
                        module_uri,
                        properties,
                        entries,
                        elements,
                    }));
                }
                Frame::Reference { path_len } => {
                    let start = out.len() - (2 + path_len);
                    let (domain, data, path) = {
                        let mut iter = out.drain(start..);
                        let domain = iter.next().expect("ref domain");
                        let data = iter.next().expect("ref data");
                        let path = iter.collect();
                        (domain, data, path)
                    };
                    out.push(Self::Reference {
                        domain: Box::new(domain),
                        data: Box::new(data),
                        path,
                    });
                }
            }
        }
        out.pop().expect("decoded root")
    }
}

/// Decoder for Apple Pkl `pkl-binary` MessagePack payloads (`org.pkl.core.PklBinaryDecoder`).
#[derive(Clone, Copy, Debug, Default)]
pub struct PklBinaryDecoder;

impl PklBinaryDecoder {
    pub fn decode(bytes: &[u8]) -> Result<PklValue> {
        let decoded = decode_pkl_binary(bytes)
            .map_err(|err| PklError::decode(format!("Failed to decode pkl-binary: {err}")))?;
        Ok(PklValue::from_decoded(decoded))
    }

    /// Decodes a `pkl-binary` payload accepting all 19 `PklBinaryCode` wire tags,
    /// including `IntSeq`, `Class`, `TypeAlias`, `Function`, and `0x20` `Reference`.
    pub fn decode_lossless(bytes: &[u8]) -> Result<PklValue> {
        let decoded = decode_pkl_binary_lossless(bytes)
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

impl FromPklValue for i8 {
    fn from_pkl_value(value: &PklValue) -> Result<Self> {
        let v = i64::from_pkl_value(value)?;
        i8::try_from(v).map_err(|_| PklError::decode(format!("Int {v} out of range for i8.")))
    }
}

impl FromPklValue for u8 {
    fn from_pkl_value(value: &PklValue) -> Result<Self> {
        let v = i64::from_pkl_value(value)?;
        u8::try_from(v).map_err(|_| PklError::decode(format!("Int {v} out of range for u8.")))
    }
}

impl FromPklValue for i16 {
    fn from_pkl_value(value: &PklValue) -> Result<Self> {
        let v = i64::from_pkl_value(value)?;
        i16::try_from(v).map_err(|_| PklError::decode(format!("Int {v} out of range for i16.")))
    }
}

impl FromPklValue for u16 {
    fn from_pkl_value(value: &PklValue) -> Result<Self> {
        let v = i64::from_pkl_value(value)?;
        u16::try_from(v).map_err(|_| PklError::decode(format!("Int {v} out of range for u16.")))
    }
}

impl FromPklValue for i32 {
    fn from_pkl_value(value: &PklValue) -> Result<Self> {
        let v = i64::from_pkl_value(value)?;
        i32::try_from(v).map_err(|_| PklError::decode(format!("Int {v} out of range for i32.")))
    }
}

impl FromPklValue for u32 {
    fn from_pkl_value(value: &PklValue) -> Result<Self> {
        let v = i64::from_pkl_value(value)?;
        u32::try_from(v).map_err(|_| PklError::decode(format!("Int {v} out of range for u32.")))
    }
}

impl FromPklValue for u64 {
    fn from_pkl_value(value: &PklValue) -> Result<Self> {
        let v = i64::from_pkl_value(value)?;
        u64::try_from(v).map_err(|_| PklError::decode(format!("Int {v} out of range for u64.")))
    }
}

impl FromPklValue for isize {
    fn from_pkl_value(value: &PklValue) -> Result<Self> {
        let v = i64::from_pkl_value(value)?;
        isize::try_from(v).map_err(|_| PklError::decode(format!("Int {v} out of range for isize.")))
    }
}

impl FromPklValue for usize {
    fn from_pkl_value(value: &PklValue) -> Result<Self> {
        let v = i64::from_pkl_value(value)?;
        usize::try_from(v).map_err(|_| PklError::decode(format!("Int {v} out of range for usize.")))
    }
}

impl FromPklValue for f32 {
    fn from_pkl_value(value: &PklValue) -> Result<Self> {
        let v = f64::from_pkl_value(value)?;
        let narrowed = v as f32;
        if v.is_finite() && !narrowed.is_finite() {
            return Err(PklError::decode(format!("Float {v} out of range for f32.")));
        }
        Ok(narrowed)
    }
}

impl FromPklValue for f64 {
    fn from_pkl_value(value: &PklValue) -> Result<Self> {
        value
            .as_f64()
            .ok_or_else(|| PklError::decode(format!("Expected Float or Int, got {value:?}.")))
    }
}

impl FromPklValue for PklDuration {
    fn from_pkl_value(value: &PklValue) -> Result<Self> {
        match value {
            PklValue::Duration(d) => Ok(d.clone()),
            other => Err(PklError::decode(format!(
                "Expected Duration, got {other:?}."
            ))),
        }
    }
}

impl FromPklValue for PklDataSize {
    fn from_pkl_value(value: &PklValue) -> Result<Self> {
        match value {
            PklValue::DataSize(d) => Ok(d.clone()),
            other => Err(PklError::decode(format!(
                "Expected DataSize, got {other:?}."
            ))),
        }
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

impl FromPklValue for PklBytes {
    fn from_pkl_value(value: &PklValue) -> Result<Self> {
        value
            .as_bytes()
            .map(|b| Self(b.to_vec()))
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_pkl_value_primitives_and_pkl_bytes() {
        assert_eq!(i8::from_pkl_value(&PklValue::Int(-128)).unwrap(), -128);
        assert!(i8::from_pkl_value(&PklValue::Int(128)).is_err());

        assert_eq!(u8::from_pkl_value(&PklValue::Int(255)).unwrap(), 255);
        assert!(u8::from_pkl_value(&PklValue::Int(-1)).is_err());
        assert!(u8::from_pkl_value(&PklValue::Int(256)).is_err());

        assert_eq!(
            i16::from_pkl_value(&PklValue::Int(-32_768)).unwrap(),
            -32_768
        );
        assert!(i16::from_pkl_value(&PklValue::Int(32_768)).is_err());

        assert_eq!(u16::from_pkl_value(&PklValue::Int(65_535)).unwrap(), 65_535);
        assert!(u16::from_pkl_value(&PklValue::Int(-1)).is_err());

        assert_eq!(
            u32::from_pkl_value(&PklValue::Int(u32::MAX as i64)).unwrap(),
            u32::MAX
        );
        assert!(u32::from_pkl_value(&PklValue::Int(u32::MAX as i64 + 1)).is_err());

        assert_eq!(isize::from_pkl_value(&PklValue::Int(-42)).unwrap(), -42);
        assert_eq!(f32::from_pkl_value(&PklValue::Float(1.5)).unwrap(), 1.5_f32);
        assert_eq!(f32::from_pkl_value(&PklValue::Int(4)).unwrap(), 4.0_f32);
        assert!(f32::from_pkl_value(&PklValue::Float(f64::MAX)).is_err());
        assert_eq!(
            f32::from_pkl_value(&PklValue::Float(f64::INFINITY)).unwrap(),
            f32::INFINITY
        );

        let dur = PklDuration {
            value: 2.5,
            unit: "s".to_string(),
        };
        assert_eq!(
            PklDuration::from_pkl_value(&PklValue::Duration(dur.clone())).unwrap(),
            dur
        );
        let sz = PklDataSize {
            value: 16.0,
            unit: "mb".to_string(),
        };
        assert_eq!(
            PklDataSize::from_pkl_value(&PklValue::DataSize(sz.clone())).unwrap(),
            sz
        );

        // `Vec<u8>` decodes `List<UInt8>` without conflicting with `Bytes`.
        let list = PklValue::List(vec![
            PklValue::Int(0),
            PklValue::Int(127),
            PklValue::Int(255),
        ]);
        assert_eq!(Vec::<u8>::from_pkl_value(&list).unwrap(), vec![0, 127, 255]);
        assert!(Vec::<u8>::from_pkl_value(&PklValue::Bytes(vec![1, 2, 3])).is_err());

        // `PklBytes` explicitly decodes Pkl `Bytes`.
        let raw = PklBytes::from_pkl_value(&PklValue::Bytes(vec![0xde, 0xad, 0xbe, 0xef])).unwrap();
        assert_eq!(&*raw, &[0xde, 0xad, 0xbe, 0xef]);
        assert_eq!(raw.into_inner(), vec![0xde, 0xad, 0xbe, 0xef]);
        assert!(PklBytes::from_pkl_value(&list).is_err());
    }

    #[test]
    fn round4_deep_pkl_value_drop_clone_eq_and_debug_do_not_overflow_stack() {
        let mut val = PklValue::Int(42);
        for _ in 0..4_000 {
            val = PklValue::List(vec![val]);
        }
        let cloned = val.clone();
        assert_eq!(val, cloned);
        let dbg = format!("{val:?}");
        assert!(
            dbg.contains("..."),
            "deep Debug must truncate cleanly: {dbg}"
        );
        drop(cloned);
        drop(val);

        // P2-10: PklBinaryDecoder::decode_lossless decodes IntSeq tag 0x0a.
        let decoded = PklBinaryDecoder::decode_lossless(&[0x94, 0x0a, 1, 5, 2]).unwrap();
        assert_eq!(
            decoded,
            PklValue::IntSeq {
                start: 1,
                end: 5,
                step: 2,
            }
        );
    }
}
