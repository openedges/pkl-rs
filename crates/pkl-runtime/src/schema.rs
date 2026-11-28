//! Pure-Rust Wasm-compatible `ModuleSchema` export model matching Apple Pkl 0.32.0
//! `org.pkl.core.{ModuleSchema, PClass, PClassInfo, PType, TypeAlias, TypeParameter, Modifier, Member}`.
//!
//! Cyclic class/type references (e.g. `Any <-> String`, recursive classes) are represented
//! safely without `unsafe` or reference-counted cycles via an immutable [`SchemaStore`]
//! indexed by [`ClassId`] and [`TypeAliasId`].

use crate::evaluation::session::SourceSession;
use crate::pkl_binary_decode::DecodedObject;
use crate::value::{RuntimeError, Value};
use std::collections::BTreeMap;
use std::fmt;
use std::sync::Arc;

/// Index of a [`PClass`] inside a [`SchemaStore`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ClassId(pub usize);

/// Index of a [`TypeAlias`] inside a [`SchemaStore`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TypeAliasId(pub usize);

/// Declaration modifier exported by `org.pkl.core.Modifier` / `VmModifier.export`.
///
/// Order matches Java's `EnumSet<Modifier>` iteration order:
/// `abstract`, `open`, `hidden`, `external` (`external` is exported on classes/typealiases only).
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Modifier {
    Abstract,
    Open,
    Hidden,
    External,
}

impl Modifier {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Abstract => "abstract",
            Self::Open => "open",
            Self::Hidden => "hidden",
            Self::External => "external",
        }
    }
}

impl fmt::Display for Modifier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Type parameter variance (`org.pkl.core.TypeParameter.Variance`).
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Variance {
    Invariant,
    Covariant,
    Contravariant,
}

/// Generic type parameter (`org.pkl.core.TypeParameter`).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct TypeParameter {
    pub name: String,
    pub variance: Variance,
}

/// 1-based line range of a declaration (`org.pkl.core.Member.SourceLocation`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SourceLocation {
    pub start_line: u32,
    pub end_line: u32,
}

/// Class metadata (`org.pkl.core.PClassInfo`).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct PClassInfo {
    pub module_name: String,
    pub simple_name: String,
    pub module_uri: String,
    pub is_module_class: bool,
}

impl PClassInfo {
    pub const MODULE_CLASS_NAME: &'static str = "ModuleClass";

    pub fn qualified_name(&self) -> String {
        format!("{}#{}", self.module_name, self.simple_name)
    }

    pub fn display_name(&self) -> String {
        if self.module_name == "pkl.base" {
            self.simple_name.clone()
        } else if self.is_module_class {
            self.module_name.clone()
        } else {
            self.qualified_name()
        }
    }

    pub fn is_standard_library_member(&self) -> bool {
        self.module_name.starts_with("pkl.")
    }

    pub fn is_typed(&self) -> bool {
        self.module_name == "pkl.base" && self.simple_name == "Typed"
    }

    pub fn is_deprecated(&self) -> bool {
        self.module_name == "pkl.base" && self.simple_name == "Deprecated"
    }

    pub fn is_unlisted(&self) -> bool {
        self.module_name == "pkl.base" && self.simple_name == "Unlisted"
    }

    pub fn is_also_known_as(&self) -> bool {
        self.module_name == "pkl.base" && self.simple_name == "AlsoKnownAs"
    }

    pub fn is_module_info(&self) -> bool {
        self.module_name == "pkl.base" && self.simple_name == "ModuleInfo"
    }

    pub fn is_doc_example(&self) -> bool {
        self.module_name == "pkl.base" && self.simple_name == "DocExample"
    }
}

/// Type alias identity metadata.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct TypeAliasInfo {
    pub module_name: String,
    pub simple_name: String,
    pub qualified_name: String,
}

impl TypeAliasInfo {
    pub fn display_name(&self) -> String {
        if self.module_name == "pkl.base" {
            self.simple_name.clone()
        } else {
            self.qualified_name.clone()
        }
    }

    pub fn is_standard_library_member(&self) -> bool {
        self.module_name.starts_with("pkl.")
    }
}

/// Exported Pkl type annotation (`org.pkl.core.PType`).
#[non_exhaustive]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PType {
    Unknown,
    Nothing,
    Module,
    StringLiteral(String),
    Class {
        class_id: ClassId,
        class_info: PClassInfo,
        type_arguments: Vec<PType>,
    },
    Nullable(Box<PType>),
    Union(Vec<PType>),
    Function {
        parameter_types: Vec<PType>,
        return_type: Box<PType>,
    },
    Constrained {
        base_type: Box<PType>,
        constraints: Vec<String>,
    },
    Alias {
        alias_id: TypeAliasId,
        alias_info: TypeAliasInfo,
        type_arguments: Vec<PType>,
        aliased_type: Box<PType>,
    },
    TypeVariable(TypeParameter),
}

impl PType {
    pub fn type_arguments(&self) -> &[PType] {
        match self {
            Self::Class { type_arguments, .. } | Self::Alias { type_arguments, .. } => {
                type_arguments
            }
            _ => &[],
        }
    }
}

/// Exported class/module property (`org.pkl.core.PClass.Property`).
#[derive(Clone, Debug, PartialEq)]
pub struct PClassProperty {
    pub owner: ClassId,
    pub owner_info: PClassInfo,
    pub simple_name: String,
    pub doc_comment: Option<String>,
    pub source_location: SourceLocation,
    pub modifiers: Vec<Modifier>,
    pub annotations: Vec<DecodedObject>,
    pub ty: PType,
}

impl PClassProperty {
    pub fn module_name(&self) -> &str {
        &self.owner_info.module_name
    }

    pub fn is_abstract(&self) -> bool {
        self.modifiers.contains(&Modifier::Abstract)
    }

    pub fn is_open(&self) -> bool {
        self.modifiers.contains(&Modifier::Open)
    }

    pub fn is_hidden(&self) -> bool {
        self.modifiers.contains(&Modifier::Hidden)
    }

    pub fn is_external(&self) -> bool {
        self.modifiers.contains(&Modifier::External)
    }

    pub fn is_standard_library_member(&self) -> bool {
        self.owner_info.is_standard_library_member()
    }
}

/// Exported class/module method (`org.pkl.core.PClass.Method`).
#[derive(Clone, Debug, PartialEq)]
pub struct PClassMethod {
    pub owner: ClassId,
    pub owner_info: PClassInfo,
    pub simple_name: String,
    pub doc_comment: Option<String>,
    pub source_location: SourceLocation,
    pub modifiers: Vec<Modifier>,
    pub annotations: Vec<DecodedObject>,
    pub type_parameters: Vec<TypeParameter>,
    /// Parameter names and types in exact declaration order.
    pub parameters: Vec<(String, PType)>,
    pub return_type: PType,
}

impl PClassMethod {
    pub fn module_name(&self) -> &str {
        &self.owner_info.module_name
    }

    pub fn is_abstract(&self) -> bool {
        self.modifiers.contains(&Modifier::Abstract)
    }

    pub fn is_open(&self) -> bool {
        self.modifiers.contains(&Modifier::Open)
    }

    pub fn is_hidden(&self) -> bool {
        self.modifiers.contains(&Modifier::Hidden)
    }

    pub fn is_external(&self) -> bool {
        self.modifiers.contains(&Modifier::External)
    }

    pub fn is_standard_library_member(&self) -> bool {
        self.owner_info.is_standard_library_member()
    }
}

/// Exported class (`org.pkl.core.PClass`).
#[derive(Clone, Debug, PartialEq)]
pub struct PClass {
    pub id: ClassId,
    pub info: PClassInfo,
    pub doc_comment: Option<String>,
    pub source_location: SourceLocation,
    pub modifiers: Vec<Modifier>,
    pub annotations: Vec<DecodedObject>,
    pub type_parameters: Vec<TypeParameter>,
    pub superclass: Option<ClassId>,
    pub supertype: Option<PType>,
    /// Properties declared directly on this class in declaration order.
    pub properties: Vec<(String, PClassProperty)>,
    /// Methods declared directly on this class in declaration order.
    pub methods: Vec<(String, PClassMethod)>,
    /// Enclosing module's `ModuleClass` (`None` if this class itself is a module class).
    pub module_class: Option<ClassId>,
}

impl PClass {
    pub fn module_name(&self) -> &str {
        &self.info.module_name
    }

    pub fn simple_name(&self) -> &str {
        &self.info.simple_name
    }

    pub fn qualified_name(&self) -> String {
        self.info.qualified_name()
    }

    pub fn display_name(&self) -> String {
        self.info.display_name()
    }

    pub fn is_module_class(&self) -> bool {
        self.info.is_module_class
    }

    pub fn is_abstract(&self) -> bool {
        self.modifiers.contains(&Modifier::Abstract)
    }

    pub fn is_open(&self) -> bool {
        self.modifiers.contains(&Modifier::Open)
    }

    pub fn is_hidden(&self) -> bool {
        self.modifiers.contains(&Modifier::Hidden)
    }

    pub fn is_external(&self) -> bool {
        self.modifiers.contains(&Modifier::External)
    }

    pub fn is_standard_library_member(&self) -> bool {
        self.info.is_standard_library_member()
    }

    pub fn property(&self, name: &str) -> Option<&PClassProperty> {
        self.properties
            .iter()
            .find(|(k, _)| k == name)
            .map(|(_, v)| v)
    }

    pub fn method(&self, name: &str) -> Option<&PClassMethod> {
        self.methods.iter().find(|(k, _)| k == name).map(|(_, v)| v)
    }
}

/// Exported type alias (`org.pkl.core.TypeAlias`).
#[derive(Clone, Debug, PartialEq)]
pub struct TypeAlias {
    pub id: TypeAliasId,
    pub info: TypeAliasInfo,
    pub doc_comment: Option<String>,
    pub source_location: SourceLocation,
    pub modifiers: Vec<Modifier>,
    pub annotations: Vec<DecodedObject>,
    pub type_parameters: Vec<TypeParameter>,
    pub aliased_type: PType,
    pub module_class: ClassId,
}

impl TypeAlias {
    pub fn module_name(&self) -> &str {
        &self.info.module_name
    }

    pub fn simple_name(&self) -> &str {
        &self.info.simple_name
    }

    pub fn qualified_name(&self) -> &str {
        &self.info.qualified_name
    }

    pub fn display_name(&self) -> String {
        self.info.display_name()
    }

    pub fn is_standard_library_member(&self) -> bool {
        self.info.is_standard_library_member()
    }
}

/// Immutable arena storing all classes and type aliases referenced by a [`ModuleSchema`].
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SchemaStore {
    pub classes: Vec<PClass>,
    pub type_aliases: Vec<TypeAlias>,
}

impl SchemaStore {
    pub fn class(&self, id: ClassId) -> &PClass {
        &self.classes[id.0]
    }

    pub fn type_alias(&self, id: TypeAliasId) -> &TypeAlias {
        &self.type_aliases[id.0]
    }

    /// Returns all properties declared on `id` and its superclasses in Java `LinkedHashMap`
    /// order (superclass properties first; overrides replace in-place).
    pub fn all_properties(&self, id: ClassId) -> Vec<(String, &PClassProperty)> {
        let mut chain = Vec::new();
        let mut cur = Some(id);
        while let Some(cid) = cur {
            chain.push(cid);
            cur = self.class(cid).superclass;
        }
        chain.reverse();

        let mut out: Vec<(String, &PClassProperty)> = Vec::new();
        let mut positions: BTreeMap<&str, usize> = BTreeMap::new();
        for cid in chain {
            for (name, prop) in &self.class(cid).properties {
                if let Some(&idx) = positions.get(name.as_str()) {
                    out[idx].1 = prop;
                } else {
                    positions.insert(name.as_str(), out.len());
                    out.push((name.clone(), prop));
                }
            }
        }
        out
    }

    /// Returns all methods declared on `id` and its superclasses in Java `LinkedHashMap`
    /// order (superclass methods first; overrides replace in-place).
    pub fn all_methods(&self, id: ClassId) -> Vec<(String, &PClassMethod)> {
        let mut chain = Vec::new();
        let mut cur = Some(id);
        while let Some(cid) = cur {
            chain.push(cid);
            cur = self.class(cid).superclass;
        }
        chain.reverse();

        let mut out: Vec<(String, &PClassMethod)> = Vec::new();
        let mut positions: BTreeMap<&str, usize> = BTreeMap::new();
        for cid in chain {
            for (name, method) in &self.class(cid).methods {
                if let Some(&idx) = positions.get(name.as_str()) {
                    out[idx].1 = method;
                } else {
                    positions.insert(name.as_str(), out.len());
                    out.push((name.clone(), method));
                }
            }
        }
        out
    }

    /// Mirrors `PClass.Property.getInheritedDocComment()`.
    pub fn inherited_property_doc_comment(
        &self,
        owner: ClassId,
        property_name: &str,
    ) -> Option<&str> {
        let mut cur = Some(owner);
        while let Some(cid) = cur {
            let cls = self.class(cid);
            if let Some(prop) = cls.property(property_name)
                && let Some(doc) = prop.doc_comment.as_deref()
            {
                return Some(doc);
            }
            cur = cls.superclass;
        }
        None
    }

    /// Mirrors `PClass.Method.getInheritedDocComment()`.
    pub fn inherited_method_doc_comment(&self, owner: ClassId, method_name: &str) -> Option<&str> {
        let mut cur = Some(owner);
        while let Some(cid) = cur {
            let cls = self.class(cid);
            if let Some(method) = cls.method(method_name)
                && let Some(doc) = method.doc_comment.as_deref()
            {
                return Some(doc);
            }
            cur = cls.superclass;
        }
        None
    }

    /// Returns direct superclass followed by ancestor superclasses up to `pkl.base#Any`.
    pub fn superclass_chain(&self, id: ClassId) -> Vec<ClassId> {
        let mut chain = Vec::new();
        let mut cur = self.class(id).superclass;
        while let Some(cid) = cur {
            chain.push(cid);
            cur = self.class(cid).superclass;
        }
        chain
    }

    /// Mirrors `PClass.isSubclassOf(other)`.
    pub fn is_subclass_of(&self, sub: ClassId, sup: ClassId) -> bool {
        let mut cur = Some(sub);
        while let Some(cid) = cur {
            if cid == sup {
                return true;
            }
            cur = self.class(cid).superclass;
        }
        false
    }
}

/// Exported module schema (`org.pkl.core.ModuleSchema`).
#[derive(Clone, Debug, PartialEq)]
pub struct ModuleSchema {
    pub module_uri: String,
    pub module_name: String,
    pub is_amend: bool,
    pub supermodule: Option<Box<ModuleSchema>>,
    pub module_class_id: ClassId,
    pub doc_comment: Option<String>,
    pub annotations: Vec<DecodedObject>,
    /// Classes declared in this module in declaration order.
    pub classes: Vec<(String, ClassId)>,
    /// Type aliases declared in this module in declaration order.
    pub type_aliases: Vec<(String, TypeAliasId)>,
    /// Imports declared in this module in declaration order `(alias, absolute_uri)`.
    pub imports: Vec<(String, String)>,
    pub store: Arc<SchemaStore>,
}

impl ModuleSchema {
    pub fn is_extend(&self) -> bool {
        self.supermodule.is_some() && !self.is_amend
    }

    pub fn short_module_name(&self) -> &str {
        self.module_name
            .rsplit_once('.')
            .map_or(self.module_name.as_str(), |(_, tail)| tail)
    }

    pub fn module_class(&self) -> &PClass {
        self.store.class(self.module_class_id)
    }

    pub fn class(&self, id: ClassId) -> &PClass {
        self.store.class(id)
    }

    pub fn type_alias(&self, id: TypeAliasId) -> &TypeAlias {
        self.store.type_alias(id)
    }

    pub fn class_by_name(&self, name: &str) -> Option<&PClass> {
        self.classes
            .iter()
            .find(|(k, _)| k == name)
            .map(|(_, id)| self.store.class(*id))
    }

    pub fn type_alias_by_name(&self, name: &str) -> Option<&TypeAlias> {
        self.type_aliases
            .iter()
            .find(|(k, _)| k == name)
            .map(|(_, id)| self.store.type_alias(*id))
    }

    /// All classes declared in this module and its supermodules (supermodule classes first).
    pub fn all_classes(&self) -> Vec<(String, ClassId)> {
        let mut chain = Vec::new();
        let mut cur = Some(self);
        while let Some(schema) = cur {
            chain.push(schema);
            cur = schema.supermodule.as_deref();
        }
        chain.reverse();

        let mut out: Vec<(String, ClassId)> = Vec::new();
        let mut positions: BTreeMap<&str, usize> = BTreeMap::new();
        for schema in chain {
            for (name, cid) in &schema.classes {
                if let Some(&idx) = positions.get(name.as_str()) {
                    out[idx].1 = *cid;
                } else {
                    positions.insert(name.as_str(), out.len());
                    out.push((name.clone(), *cid));
                }
            }
        }
        out
    }

    /// All type aliases declared in this module and its supermodules (supermodule aliases first).
    pub fn all_type_aliases(&self) -> Vec<(String, TypeAliasId)> {
        let mut chain = Vec::new();
        let mut cur = Some(self);
        while let Some(schema) = cur {
            chain.push(schema);
            cur = schema.supermodule.as_deref();
        }
        chain.reverse();

        let mut out: Vec<(String, TypeAliasId)> = Vec::new();
        let mut positions: BTreeMap<&str, usize> = BTreeMap::new();
        for schema in chain {
            for (name, tid) in &schema.type_aliases {
                if let Some(&idx) = positions.get(name.as_str()) {
                    out[idx].1 = *tid;
                } else {
                    positions.insert(name.as_str(), out.len());
                    out.push((name.clone(), *tid));
                }
            }
        }
        out
    }
}

/// Step result from [`SchemaExtractor::step`].
pub enum SchemaStep {
    /// A VM preparation block has been started on `session`; the caller must drive
    /// `session` to completion and pass the returned [`Value`] to the next `step` call.
    RunPending,
    /// Schema extraction completed.
    Done(ModuleSchema),
}

/// Host-agnostic, Wasm-pure state machine that prepares lazy classes/typealiases/imports/annotations
/// on a loaded module and builds a complete [`ModuleSchema`].
pub struct SchemaExtractor {
    pub(crate) inner: crate::vm::schema::VmSchemaExtractor,
}

impl SchemaExtractor {
    pub fn new(module: Value) -> Result<Self, RuntimeError> {
        Ok(Self {
            inner: crate::vm::schema::VmSchemaExtractor::new(module)?,
        })
    }

    pub fn step(
        &mut self,
        session: &mut SourceSession,
        last_result: Option<Value>,
        now_ms: u64,
        timeout_ms: Option<u64>,
    ) -> Result<SchemaStep, RuntimeError> {
        self.inner.step(session, last_result, now_ms, timeout_ms)
    }
}

/// Convenience driver for extracting a [`ModuleSchema`] from an already-loaded module [`Value`]
/// on a [`SourceSession`] using a caller-supplied host drive function.
pub fn extract_module_schema_with<F, E>(
    session: &mut SourceSession,
    module: Value,
    now_ms: u64,
    timeout_ms: Option<u64>,
    mut drive: F,
) -> Result<ModuleSchema, E>
where
    F: FnMut(&mut SourceSession) -> Result<Value, E>,
    E: From<RuntimeError>,
{
    let mut extractor = SchemaExtractor::new(module)?;
    let mut last_result = None;
    loop {
        match extractor.step(session, last_result, now_ms, timeout_ms)? {
            SchemaStep::RunPending => {
                last_result = Some(drive(session)?);
            }
            SchemaStep::Done(schema) => return Ok(schema),
        }
    }
}
