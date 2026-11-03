//! Evaluator-owned tracing arena. No object edge uses reference counting.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::value::{ErrorKind, ObjectId, PklString, RuntimeError, Value};

static NEXT_HEAP: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Scope {
    pub receiver: ObjectId,
    pub owner: ObjectId,
}

#[derive(Clone, Debug, Default)]
pub struct Environment {
    pub locals: Vec<Value>,
    /// Source descriptor for materialized physical slots retained by closures.
    #[doc(hidden)]
    pub local_owners: BTreeMap<usize, (ExecutableOwnerId, (usize, usize))>,
    /// Parameters, let bindings and generator bindings in the current lexical
    /// frame. Enclosing function captures are not copied into a new descriptor.
    #[doc(hidden)]
    pub copy_locals: BTreeSet<usize>,
    /// Generated members restore their saved generator activation on forcing.
    #[doc(hidden)]
    pub restore_for_bindings: bool,
    pub this_value: Option<Value>,
    pub outer_value: Option<Value>,
    pub custom_this: Option<CustomThisSlot>,
    /// Persistent cloned executable identity, independent of the receiver slot.
    /// None keeps the canonical code template implicit across every branch.
    pub executable_owner: Option<ExecutableOwnerId>,
    /// Traced lifetime of dynamically compiled code captured by this environment.
    #[doc(hidden)]
    pub code_owner: Option<ExecutableOwnerId>,
    /// Lexical substitutions captured by reusable generic constraint bodies.
    /// Runtime values are immutable shared type DAGs; environment clones copy handles.
    pub type_arguments: BTreeMap<(usize, PklString), crate::vm::TypeCheck>,
    /// Source sections follow actual arguments without participating in type identity.
    pub type_argument_sources: BTreeMap<(usize, PklString), crate::vm::TypeArgumentSource>,
    /// Innermost first; immutable captured lexical object scopes.
    pub scopes: Vec<Scope>,
}

#[derive(Clone, Debug)]
pub struct Member {
    /// Authored modifier arithmetic and the pinned internal declaration bits.
    pub flags: u32,
    pub entry: bool,
    pub name: PklString,
    pub code: usize,
    pub local: bool,
    pub method: bool,
    pub ty: Option<crate::vm::TypeCheck>,
    pub type_span: Option<crate::source::Span>,
    pub constant: bool,
    /// A class property may be overridden only by another fixed declaration.
    pub fixed: bool,
}

impl Member {
    const CLASS: u32 = 0x100;
    const TYPE_ALIAS: u32 = 0x200;
    const IMPORT: u32 = 0x80;
    const GLOB: u32 = 0x1000;
    pub(crate) fn declaration_flags(member: &crate::syntax::Member) -> u32 {
        crate::syntax::modifier_flags(&member.modifiers)
            | match member.expr.kind {
                crate::syntax::ExprKind::Import {
                    clause_alias: Some(_),
                    glob,
                    ..
                } => Self::IMPORT | if glob { Self::GLOB } else { 0 },
                crate::syntax::ExprKind::Class { .. } => Self::CLASS,
                crate::syntax::ExprKind::TypeAlias { .. } => Self::TYPE_ALIAS,
                _ => 0,
            }
    }
    pub(crate) fn hidden(&self) -> bool {
        self.flags & crate::syntax::ModifierKind::Hidden.flag() != 0
    }
    pub(crate) fn external(&self) -> bool {
        self.flags & crate::syntax::ModifierKind::External.flag() != 0
    }
    pub(crate) fn abstract_member(&self) -> bool {
        self.flags & crate::syntax::ModifierKind::Abstract.flag() != 0
    }
    pub(crate) fn import(&self) -> bool {
        self.flags & Self::IMPORT != 0
    }
    pub(crate) fn glob(&self) -> bool {
        self.flags & Self::GLOB != 0
    }
    pub(crate) fn definition(&self) -> bool {
        self.flags & (Self::CLASS | Self::TYPE_ALIAS) != 0
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum LazyState {
    Unevaluated,
    Evaluating,
    Completed(Value),
}

/// Local definitions use owner identity; ordinary virtual properties use `None`.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct CacheKey {
    pub entry: bool,
    pub owner: Option<ObjectId>,
    pub name: PklString,
    pub method: bool,
}

#[derive(Clone, Debug)]
pub struct Function {
    pub block: usize,
    pub arity: usize,
    pub method: bool,
    /// Custom-scope receiver captured when the literal is evaluated.
    pub captured_this: Option<Value>,
}

/// Class labels follow PClassInfo.getDisplayName, independently of identity or URI.
pub(crate) fn display_class_name(
    module: impl Into<PklString>,
    simple_name: impl Into<PklString>,
) -> PklString {
    let module = module.into();
    let simple_name = simple_name.into();
    if module == "pkl.base" {
        simple_name
    } else if simple_name == "ModuleClass" {
        module
    } else {
        module.concat(&"#".into()).concat(&simple_name)
    }
}

#[derive(Clone, Debug)]
pub struct Class {
    pub module: Option<PklString>,
    /// Original declaration ModuleKey URI, independent of the displayed module name.
    pub origin_uri: Option<PklString>,
    pub id: usize,
    pub name: PklString,
    pub open: bool,
    pub abstract_: bool,
}

/// A class value is distinct from its instance prototype. Declaration metadata
/// and incremental member publication live on the owned prototype exactly once.
/// Both edges are traced; neither depends on a Rust reference-counted cycle.
#[derive(Clone, Debug)]
pub struct ClassRecord {
    pub prototype: ObjectId,
    pub superclass: Option<ObjectId>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CollectionKind {
    #[default]
    Dynamic,
    Listing,
    Mapping,
    List,
    Map,
    Set,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CollectionMemberKind {
    Property,
    Entry,
    Element,
}

#[derive(Clone, Debug)]
pub struct CollectionMember {
    pub key: Value,
    pub name: PklString,
    pub kind: CollectionMemberKind,
}

#[derive(Clone, Debug)]
pub struct CollectionIterator {
    pub source: ObjectId,
    pub members: Vec<CollectionMember>,
    pub position: usize,
    pub force_members: Vec<CollectionMember>,
    pub force_position: usize,
    pub awaiting_value: bool,
}

#[derive(Clone, Debug)]
pub struct PredicateIterator {
    pub source: ObjectId,
    pub members: Vec<(ObjectId, CollectionMember)>,
    pub position: usize,
    pub awaiting_value: bool,
    pub previous_this: Option<Value>,
}

#[derive(Clone, Debug)]
pub struct CollectionCast {
    pub source: Option<Arc<crate::value::RuntimeSource>>,
    pub(crate) node: Option<TypeNodeId>,
    pub ty: crate::vm::TypeCheck,
    pub span: crate::source::Span,
    pub member: crate::value::PklString,
    pub environment: Environment,
}

#[derive(Clone, Debug)]
pub struct CacheSlot {
    pub key: CacheKey,
    /// The key retained by the receiver cache, independently of its declaration.
    /// Bootstrap strict slots recover their declaration key when this is None.
    pub entry_key: Option<Value>,
    pub value: Value,
}

#[derive(Clone, Debug, Default)]
pub struct Object {
    /// Lazy mirror factory payload. Edges are traced independently of the
    /// class prototype, including the declaration's executable owner.
    pub mirror: Option<Box<crate::vm::reflection::Payload>>,
    /// Reader payload has no heap edges. Factory property nodes read their
    /// declaration owner's payload, including when inherited by an amendment.
    pub resource: Option<Arc<crate::resources::Resource>>,
    pub(crate) glob: Option<Arc<crate::vm::glob::Payload>>,
    /// Immutable match result has no heap edges; factory reads declaration owner.
    pub regex_match: Option<Arc<crate::pkl_regex::RegexMatchData>>,
    /// Recursive force publishes this mark before traversing children.
    pub forced: bool,
    /// VmDynamic caches its first regular-member count, including a reentrant
    /// force prefix. Later cache publication does not invalidate that count.
    pub dynamic_regular_member_count: Option<usize>,
    /// Evaluator-owned canonical identity, populated only after key hashing succeeds.
    pub structural_key: Option<u64>,
    /// Numeric Java hash, independent of canonical equality identity.
    /// Inner None marks a hash depending on JVM identity allocation.
    pub java_hash: Option<Option<i32>>,
    pub pair: Option<Box<[Value; 2]>>,
    /// Opaque Java Map.entry values exposed by VmCollection.flatten.
    /// Distinct from Pkl Pair; key/value remain traced and structurally comparable.
    pub foreign_map_entry: Option<Box<[Value; 2]>>,
    pub amendment: Option<crate::vm::amendment::Amendment>,
    /// Argument-check section and function-construction section have distinct
    /// Java source nodes, even when both belong to the same Mixin type use.
    pub mixin: Option<(
        crate::vm::TypeCheck,
        Option<crate::value::RuntimeFrame>,
        Option<crate::value::RuntimeFrame>,
    )>,
    pub function: Option<Function>,
    pub reference: Option<crate::vm::reference::Reference>,
    pub native_function: Option<crate::vm::reference::NativeFunction>,
    pub type_alias: Option<PklString>,
    pub type_alias_declaration: Option<usize>,
    /// Compiled declaration identity, separate from resolved type identity.
    /// The environment below retains the owning executable arena.
    pub declaration_code: Option<usize>,
    /// Successful mirror cache belongs to the declaration, never a VM-wide root.
    pub declaration_mirror: Option<ObjectId>,
    pub module_mirror: Option<ObjectId>,
    pub mirror_annotations: BTreeMap<crate::source::Span, Vec<Value>>,
    pub type_alias_initialized: bool,
    /// Declaration-owned annotation values; fields remain ordinary lazy members.
    pub annotations: Vec<Value>,
    /// Published property/method annotation metadata, keyed by declaration code.
    pub member_annotations: BTreeMap<usize, Vec<Value>>,
    /// First exact Deprecated annotation's forced message, published with its method.
    #[doc(hidden)]
    pub member_deprecations: BTreeMap<usize, crate::value::PklString>,
    /// Temporary annotation-list metadata, before declaration publication.
    #[doc(hidden)]
    pub deprecation: Option<crate::value::PklString>,
    /// Actual declaration of a materialized method call target.
    #[doc(hidden)]
    pub method_declaration: Option<(ObjectId, usize)>,
    pub builtin_type: Option<crate::vm::TypeCheck>,
    pub ref_module: bool,
    pub is_module: bool,
    pub module_unit: Option<crate::modules::UnitId>,
    pub module_initializing: bool,
    pub published_module_members: std::collections::BTreeSet<usize>,
    pub class_definition: Option<Class>,
    /// The actual runtime class of a typed instance or class prototype.
    pub instance_class: Option<ObjectId>,
    /// Published class whose initialization has not completed (including failure).
    /// Such classes accept new property names, but expose only published metadata.
    pub class_initializing: bool,
    /// Distinguishes a published class with an unresolved superclass from Any,
    /// whose initialized superclass is absent.
    pub class_superclass_initialized: bool,
    pub parent: Option<ObjectId>,
    pub members: Vec<Member>,
    pub environment: Environment,
    pub cache: BTreeMap<CacheKey, LazyState>,
    /// Successful cache publication order, matching VmObject.cachedValues.
    /// Evaluating markers do not occupy an insertion position.
    pub completed_cache_order: Vec<CacheKey>,
    pub cache_slots: Vec<CacheSlot>,
    pub cache_index: crate::economic_map::EconomicMapIndex,
    pub collection_kind: CollectionKind,
    pub collection_members: Vec<CollectionMember>,
    pub map_index: Option<Box<crate::strict_map::MapIndex>>,
    /// Captured Map insertion-order keys, independent of stored trie slots.
    /// Removal can change a different slot after recursive key forcing.
    pub map_key_order: Option<Vec<CollectionMember>>,
    /// Strict Set hash-table keys; element order lives in collection_members.
    pub set_entries: Vec<Value>,
    pub declaration_index: Option<crate::economic_map::EconomicMapIndex>,
    pub declaration_hashes: Vec<i32>,
    pub length: usize,
    pub mapping_length: Option<usize>,
    /// VmMapping.getAllKeys caches its own persistent Set wrapper.
    pub mapping_keys: Option<ObjectId>,
    pub specialized_literal: bool,
    pub member_environments: BTreeMap<CacheKey, Environment>,
    pub member_values: BTreeMap<CacheKey, Value>,
    pub iterator: Option<CollectionIterator>,
    pub int_seq_iterator: Option<Box<crate::int_seq::Cursor>>,
    pub bytes_iterator: Option<Box<(crate::value::Bytes, usize, bool)>>,
    pub conversion_delegate: Option<Box<crate::vm::collection_conversions::Delegate>>,
    pub predicate_iterator: Option<PredicateIterator>,
    pub collection_cast: Option<CollectionCast>,
    /// Inherited default value for inferred element/entry objects. `Some(None)`
    /// means the annotated value type has no default; `None` uses Dynamic's default.
    pub collection_default: Option<Option<Value>>,
    /// Authored declaration of a synthesized undefined collection default.
    pub collection_default_origin: Option<crate::value::RuntimeFrame>,
}

impl Object {
    pub(crate) fn map_index(&self) -> &crate::strict_map::MapIndex {
        self.map_index
            .as_deref()
            .unwrap_or(&crate::strict_map::EMPTY)
    }

    pub(crate) fn complete_cache(&mut self, key: CacheKey, value: Value) {
        // Completed entries persist for the object's lifetime. Failure cleanup
        // removes only pending entries, so retries acquire their position here.
        if !matches!(self.cache.get(&key), Some(LazyState::Completed(_))) {
            self.completed_cache_order.push(key.clone());
            self.cache_slots.push(CacheSlot {
                key: key.clone(),
                entry_key: None,
                value: value.clone(),
            });
        }
        if let Some(slot) = self.cache_slots.iter_mut().find(|slot| slot.key == key) {
            slot.value = value.clone();
        }
        self.cache.insert(key, LazyState::Completed(value));
    }
}

/// Identity of one logical call frame's auxiliary receiver slot. This is never
/// exposed as a Pkl value; its arena entry participates in ordinary tracing.
#[derive(Clone, Copy, Debug)]
pub struct CustomThisSlot(ObjectId);

impl CustomThisSlot {
    pub(crate) fn root(self) -> Value {
        Value::Object(self.0)
    }
}

/// Persistent evaluator-local executable state. Handles are generation checked
/// and traced, but are never exposed as language values.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ExecutableOwnerId(ObjectId);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct TypeNodeId(pub(crate) ExecutableOwnerId);

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum ExecutionSite {
    TypeUse { block: usize, instruction: usize },
    PropertyType(usize),
    TypeChild(usize),
    Constraint(usize),
    Function { block: usize, instruction: usize },
    RegexConstructor { block: usize, instruction: usize },
    Generator { block: usize, instruction: usize },
    Member(usize),
}

#[derive(Default)]
struct ExecutableState {
    code_roots: Vec<Value>,
    children: BTreeMap<ExecutionSite, ExecutableOwnerId>,
    // ObjectMember belongs to the declaration, whereas owner/receiver are read
    // from the current frame on every invocation. Never retain the first receiver.
    local_properties: BTreeMap<(usize, usize), Member>,
    // ReadAmbiguousLocalityPropertyNode replaces itself on its first execution.
    // The choice belongs to this executable clone, never to a receiver object.
    ambiguous_localities: BTreeMap<(usize, usize), bool>,
    local_kinds: BTreeMap<(usize, usize), crate::vm::identity::SlotKind>,
    // ImportNode caches its successful result, not the importing receiver or
    // activation. Distinct executable clones retain distinct node state.
    imported_modules: BTreeMap<(usize, usize), ObjectId>,
    glob_mappings: BTreeMap<(usize, usize, crate::value::PklString), ObjectId>,
    method_calls: BTreeMap<(usize, usize), crate::vm::deprecation::CallCache>,
    regex_constructor: crate::pkl_regex::ConstructorCache,
}

impl ExecutableOwnerId {
    pub(crate) fn root(self) -> Value {
        Value::Object(self.0)
    }
}

enum Entry {
    Object(Box<Object>),
    Class(ClassRecord),
    NullDefault(Value),
    CustomThis(Option<Value>),
    Executable(Box<ExecutableState>),
}

struct Slot {
    generation: u64,
    entry: Option<Entry>,
}

impl Slot {
    fn object(&self) -> Option<&Object> {
        match &self.entry {
            Some(Entry::Object(object)) => Some(object),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct HeapStats {
    pub live: usize,
    /// Persistent executable/type nodes included in `live`, excluding objects and receiver slots.
    pub executable_live: usize,
    pub slots: usize,
    pub collections: usize,
    pub reclaimed: usize,
}

pub struct Heap {
    identity: u64,
    slots: Vec<Slot>,
    free: Vec<usize>,
    stats: HeapStats,
    pub(crate) structural_keys: BTreeMap<crate::vm::equality::StructuralKey, u64>,
    pub(crate) next_structural_key: u64,
    pub(crate) key_equalities: std::collections::BTreeSet<(ObjectId, ObjectId)>,
    // Heap-local VmBytes.vmList backlinks never put evaluator handles in code constants.
    pub(crate) bytes_lists:
        BTreeMap<crate::value_identity::Token, (std::sync::Weak<()>, ObjectId, bool)>,
}

impl Default for Heap {
    fn default() -> Self {
        Self::new()
    }
}

impl Heap {
    pub fn new() -> Self {
        Self {
            identity: NEXT_HEAP.fetch_add(1, Ordering::Relaxed),
            slots: Vec::new(),
            free: Vec::new(),
            stats: HeapStats::default(),
            structural_keys: BTreeMap::new(),
            next_structural_key: 1,
            key_equalities: std::collections::BTreeSet::new(),
            bytes_lists: BTreeMap::new(),
        }
    }

    pub fn allocate(&mut self, mut object: Object) -> ObjectId {
        object.forced = false;
        object.dynamic_regular_member_count = None;
        self.allocate_entry(Entry::Object(Box::new(object)))
    }

    pub(crate) fn allocate_class(&mut self, prototype: ObjectId) -> ObjectId {
        let id = self.allocate_entry(Entry::Class(ClassRecord {
            prototype,
            superclass: None,
        }));
        self.get_mut(prototype)
            .expect("new class prototype")
            .instance_class = Some(id);
        id
    }

    pub fn get_class(&self, id: ObjectId) -> Result<&ClassRecord, RuntimeError> {
        match self.entry(id)? {
            Entry::Class(class) => Ok(class),
            _ => Err(RuntimeError::new(
                ErrorKind::StaleHandle,
                "value is not a class",
            )),
        }
    }

    pub(crate) fn link_superclass(
        &mut self,
        id: ObjectId,
        superclass: Option<ObjectId>,
    ) -> Result<(), RuntimeError> {
        let prototype = self.get_class(id)?.prototype;
        let parent = superclass
            .map(|class| self.get_class(class).map(|c| c.prototype))
            .transpose()?;
        if let Some(Entry::Class(class)) = &mut self.slots[id.index].entry {
            class.superclass = superclass;
        }
        let prototype = self.get_mut(prototype)?;
        prototype.parent = parent;
        prototype.class_superclass_initialized = true;
        Ok(())
    }

    pub(crate) fn class_definition(&self, id: ObjectId) -> Result<&Class, RuntimeError> {
        self.get(self.get_class(id)?.prototype)?
            .class_definition
            .as_ref()
            .ok_or_else(|| {
                RuntimeError::new(ErrorKind::InvalidBytecode, "missing class definition")
            })
    }

    pub(crate) fn class_simple_name(&self, id: ObjectId) -> Result<PklString, RuntimeError> {
        let prototype = self.get(self.get_class(id)?.prototype)?;
        if prototype.is_module {
            return Ok("ModuleClass".into());
        }
        Ok(self.class_definition(id)?.name.clone())
    }

    pub(crate) fn class_qualified_name(&self, id: ObjectId) -> Result<PklString, RuntimeError> {
        let prototype = self.get(self.get_class(id)?.prototype)?;
        let definition = self.class_definition(id)?;
        let module = if prototype.is_module {
            definition.name.clone()
        } else {
            definition.module.clone().unwrap_or_default()
        };
        Ok(module
            .concat(&"#".into())
            .concat(&self.class_simple_name(id)?))
    }

    pub(crate) fn class_display_name(&self, id: ObjectId) -> Result<PklString, RuntimeError> {
        let prototype = self.get(self.get_class(id)?.prototype)?;
        let definition = self.class_definition(id)?;
        let module = if prototype.is_module {
            definition.name.clone()
        } else {
            definition.module.clone().unwrap_or_default()
        };
        Ok(display_class_name(module, self.class_simple_name(id)?))
    }

    pub(crate) fn declaration_object(&self, value: &Value) -> Result<ObjectId, RuntimeError> {
        match value {
            Value::Class(id) => Ok(self.get_class(*id)?.prototype),
            _ => value.heap_id().ok_or_else(|| {
                RuntimeError::new(ErrorKind::InvalidBytecode, "declaration has no owner")
            }),
        }
    }

    pub(crate) fn object_class(&self, mut id: ObjectId) -> Result<Option<ObjectId>, RuntimeError> {
        let mut visited = BTreeSet::new();
        loop {
            if !visited.insert(id) {
                return Err(RuntimeError::new(ErrorKind::Cycle, "cyclic parent chain"));
            }
            let object = self.get(id)?;
            if let Some(class) = object.instance_class {
                self.get_class(class)?;
                return Ok(Some(class));
            }
            let Some(parent) = object.parent else {
                return Ok(None);
            };
            id = parent;
        }
    }

    pub(crate) fn is_subclass(&self, class: ObjectId, expected: ObjectId) -> bool {
        let mut current = Some(class);
        let mut seen = BTreeSet::new();
        while let Some(class) = current {
            if class == expected {
                return true;
            }
            if !seen.insert(class) {
                return false;
            }
            let Ok(record) = self.get_class(class) else {
                return false;
            };
            current = record.superclass;
        }
        false
    }

    pub(crate) fn allocate_null(&mut self, default: Option<Value>) -> Value {
        default.map_or(Value::Null, |value| {
            Value::NullWithDefault(self.allocate_entry(Entry::NullDefault(value)))
        })
    }

    pub(crate) fn null_default(&self, id: ObjectId) -> Result<Value, RuntimeError> {
        match self.entry(id)? {
            Entry::NullDefault(value) => Ok(value.clone()),
            _ => Err(RuntimeError::new(
                ErrorKind::StaleHandle,
                "invalid null default slot",
            )),
        }
    }

    pub(crate) fn validate_value(&self, value: &Value) -> Result<(), RuntimeError> {
        if let Value::NullWithDefault(id) = value {
            self.null_default(*id)?;
        } else if let Value::Class(id) = value {
            self.get_class(*id)?;
        } else if let Some(id) = value.heap_id() {
            self.get(id)?;
        }
        Ok(())
    }

    pub(crate) fn allocate_custom_this(&mut self, value: Option<Value>) -> CustomThisSlot {
        CustomThisSlot(self.allocate_entry(Entry::CustomThis(value)))
    }

    pub(crate) fn custom_this(&self, slot: CustomThisSlot) -> Result<Option<Value>, RuntimeError> {
        match self.entry(slot.0)? {
            Entry::CustomThis(value) => Ok(value.clone()),
            _ => Err(RuntimeError::new(
                ErrorKind::StaleHandle,
                "invalid custom receiver slot",
            )),
        }
    }

    pub(crate) fn set_custom_this(
        &mut self,
        slot: CustomThisSlot,
        value: Option<Value>,
    ) -> Result<(), RuntimeError> {
        self.custom_this(slot)?;
        self.slots[slot.0.index].entry = Some(Entry::CustomThis(value));
        Ok(())
    }

    pub(crate) fn allocate_executable(&mut self) -> ExecutableOwnerId {
        ExecutableOwnerId(self.allocate_entry(Entry::Executable(Box::default())))
    }

    fn executable(&self, owner: ExecutableOwnerId) -> Result<&ExecutableState, RuntimeError> {
        match self.entry(owner.0)? {
            Entry::Executable(state) => Ok(state),
            _ => Err(RuntimeError::new(
                ErrorKind::StaleHandle,
                "invalid executable owner",
            )),
        }
    }

    pub(crate) fn regex_constructor_cache(
        &mut self,
        owner: ExecutableOwnerId,
    ) -> Result<&mut crate::pkl_regex::ConstructorCache, RuntimeError> {
        Ok(&mut self.executable_mut(owner)?.regex_constructor)
    }

    pub(crate) fn executable_is_live(&self, owner: ExecutableOwnerId) -> bool {
        self.executable(owner).is_ok()
    }

    pub(crate) fn set_code_roots(&mut self, owner: ExecutableOwnerId, roots: Vec<Value>) {
        self.executable_mut(owner)
            .expect("live code owner")
            .code_roots = roots;
    }

    #[cfg(test)]
    pub(crate) fn code_roots_for_test(&self, owner: ExecutableOwnerId) -> &[Value] {
        &self
            .executable(owner)
            .expect("live executable owner")
            .code_roots
    }

    fn executable_mut(
        &mut self,
        owner: ExecutableOwnerId,
    ) -> Result<&mut ExecutableState, RuntimeError> {
        self.executable(owner)?;
        match self.slots[owner.0.index].entry.as_mut() {
            Some(Entry::Executable(state)) => Ok(state),
            _ => unreachable!("validated executable owner"),
        }
    }

    pub(crate) fn executable_child(
        &mut self,
        owner: ExecutableOwnerId,
        site: ExecutionSite,
    ) -> Result<ExecutableOwnerId, RuntimeError> {
        if let Some(child) = self.executable(owner)?.children.get(&site) {
            return Ok(*child);
        }
        let child = self.allocate_executable();
        self.executable_mut(owner)?.children.insert(site, child);
        Ok(child)
    }

    pub(crate) fn existing_method_call_cache(
        &self,
        owner: ExecutableOwnerId,
        site: (usize, usize),
    ) -> Result<Option<&crate::vm::deprecation::CallCache>, RuntimeError> {
        Ok(self.executable(owner)?.method_calls.get(&site))
    }

    pub(crate) fn method_call_cache(
        &mut self,
        owner: ExecutableOwnerId,
        site: (usize, usize),
    ) -> Result<&mut crate::vm::deprecation::CallCache, RuntimeError> {
        Ok(self
            .executable_mut(owner)?
            .method_calls
            .entry(site)
            .or_default())
    }

    pub(crate) fn local_kind(
        &self,
        owner: ExecutableOwnerId,
        slot: (usize, usize),
    ) -> Result<Option<crate::vm::identity::SlotKind>, RuntimeError> {
        Ok(self.executable(owner)?.local_kinds.get(&slot).copied())
    }

    pub(crate) fn glob_mapping(
        &self,
        owner: ExecutableOwnerId,
        site: (usize, usize),
        pattern: &crate::value::PklString,
    ) -> Result<Option<ObjectId>, RuntimeError> {
        Ok(self
            .executable(owner)?
            .glob_mappings
            .get(&(site.0, site.1, pattern.clone()))
            .copied())
    }

    pub(crate) fn publish_glob_mapping(
        &mut self,
        owner: ExecutableOwnerId,
        site: (usize, usize),
        pattern: crate::value::PklString,
        mapping: ObjectId,
    ) -> Result<(), RuntimeError> {
        self.get(mapping)?;
        self.executable_mut(owner)?
            .glob_mappings
            .insert((site.0, site.1, pattern), mapping);
        Ok(())
    }

    pub(crate) fn imported_module(
        &self,
        owner: ExecutableOwnerId,
        site: (usize, usize),
    ) -> Result<Option<ObjectId>, RuntimeError> {
        Ok(self.executable(owner)?.imported_modules.get(&site).copied())
    }

    pub(crate) fn publish_imported_module(
        &mut self,
        owner: ExecutableOwnerId,
        site: (usize, usize),
        receiver: ObjectId,
    ) -> Result<(), RuntimeError> {
        self.get(receiver)?;
        self.executable_mut(owner)?
            .imported_modules
            .insert(site, receiver);
        Ok(())
    }

    pub(crate) fn set_local_kind(
        &mut self,
        owner: ExecutableOwnerId,
        slot: (usize, usize),
        kind: crate::vm::identity::SlotKind,
    ) -> Result<(), RuntimeError> {
        self.executable_mut(owner)?.local_kinds.insert(slot, kind);
        Ok(())
    }

    pub(crate) fn ambiguous_locality(
        &self,
        owner: ExecutableOwnerId,
        site: (usize, usize),
    ) -> Result<Option<bool>, RuntimeError> {
        Ok(self
            .executable(owner)?
            .ambiguous_localities
            .get(&site)
            .copied())
    }

    pub(crate) fn specialize_locality(
        &mut self,
        owner: ExecutableOwnerId,
        site: (usize, usize),
        local: bool,
    ) -> Result<(), RuntimeError> {
        self.executable_mut(owner)?
            .ambiguous_localities
            .insert(site, local);
        Ok(())
    }

    pub(crate) fn resolved_local(
        &self,
        owner: ExecutableOwnerId,
        site: (usize, usize),
    ) -> Result<Option<Member>, RuntimeError> {
        Ok(self.executable(owner)?.local_properties.get(&site).cloned())
    }

    pub(crate) fn publish_local(
        &mut self,
        owner: ExecutableOwnerId,
        site: (usize, usize),
        member: Member,
    ) -> Result<(), RuntimeError> {
        self.executable_mut(owner)?
            .local_properties
            .insert(site, member);
        Ok(())
    }

    fn allocate_entry(&mut self, entry: Entry) -> ObjectId {
        self.stats.executable_live += usize::from(matches!(entry, Entry::Executable(_)));
        let index = if let Some(index) = self.free.pop() {
            self.slots[index].entry = Some(entry);
            index
        } else {
            self.slots.push(Slot {
                generation: 0,
                entry: Some(entry),
            });
            self.slots.len() - 1
        };
        self.stats.live += 1;
        self.stats.slots = self.slots.len();
        ObjectId {
            heap: self.identity,
            index,
            generation: self.slots[index].generation,
        }
    }

    pub(crate) fn find_type_alias(&self, name: &PklString) -> Option<ObjectId> {
        self.slots.iter().enumerate().find_map(|(index, slot)| {
            slot.object()
                .filter(|o| o.type_alias.as_ref() == Some(name))
                .map(|_| ObjectId {
                    heap: self.identity,
                    index,
                    generation: slot.generation,
                })
        })
    }

    pub(crate) fn find_module(&self, unit: crate::modules::UnitId) -> Option<ObjectId> {
        self.slots.iter().enumerate().find_map(|(index, slot)| {
            slot.object()
                .filter(|object| object.module_unit == Some(unit))
                .map(|_| ObjectId {
                    heap: self.identity,
                    index,
                    generation: slot.generation,
                })
        })
    }

    pub(crate) fn find_class(&self, class_id: usize) -> Option<ObjectId> {
        self.slots.iter().find_map(|slot| {
            slot.object()
                .filter(|object| {
                    object
                        .class_definition
                        .as_ref()
                        .is_some_and(|class| class.id == class_id)
                })
                .and_then(|object| object.instance_class)
        })
    }

    fn entry(&self, id: ObjectId) -> Result<&Entry, RuntimeError> {
        if id.heap == self.identity
            && let Some(slot) = self.slots.get(id.index)
            && slot.generation == id.generation
            && let Some(entry) = &slot.entry
        {
            return Ok(entry);
        }
        Err(RuntimeError::new(
            ErrorKind::StaleHandle,
            "object does not belong to this live heap",
        ))
    }

    pub fn get(&self, id: ObjectId) -> Result<&Object, RuntimeError> {
        match self.entry(id)? {
            Entry::Object(object) => Ok(object),
            Entry::Class(_)
            | Entry::CustomThis(_)
            | Entry::Executable(_)
            | Entry::NullDefault(_) => Err(RuntimeError::new(
                ErrorKind::StaleHandle,
                "internal slot is not a Pkl object",
            )),
        }
    }

    pub fn get_mut(&mut self, id: ObjectId) -> Result<&mut Object, RuntimeError> {
        self.get(id)?;
        match self.slots[id.index].entry.as_mut() {
            Some(Entry::Object(object)) => Ok(object),
            _ => unreachable!("validated object slot"),
        }
    }

    pub fn stats(&self) -> HeapStats {
        self.stats
    }

    fn traced_id(&self, value: &Value) -> Option<ObjectId> {
        if let Value::Bytes(bytes) = value {
            self.bytes_lists
                .get(&bytes.reference())
                .map(|(_, id, _)| *id)
        } else {
            value.heap_id()
        }
    }

    /// Trace all object edges, including lazy captures and completed values, iteratively.
    /// The caller must include every externally retained handle in `roots`.
    pub fn collect(&mut self, roots: &[Value]) -> HeapStats {
        let mut marked = vec![false; self.slots.len()];
        let mut pending: Vec<ObjectId> = roots
            .iter()
            .filter_map(|value| self.traced_id(value))
            .collect();
        pending.extend(
            self.bytes_lists
                .values()
                .filter_map(|(lifetime, id, constant)| {
                    (*constant && lifetime.strong_count() != 0).then_some(*id)
                }),
        );
        while let Some(id) = pending.pop() {
            let Ok(entry) = self.entry(id) else { continue };
            if marked[id.index] {
                continue;
            }
            marked[id.index] = true;
            let object = match entry {
                Entry::Class(class) => {
                    pending.push(class.prototype);
                    pending.extend(class.superclass);
                    continue;
                }
                Entry::NullDefault(value) => {
                    pending.extend(self.traced_id(value));
                    continue;
                }
                Entry::Executable(state) => {
                    pending.extend(
                        state
                            .code_roots
                            .iter()
                            .filter_map(|value| self.traced_id(value)),
                    );
                    pending.extend(state.children.values().map(|owner| owner.0));
                    pending.extend(state.imported_modules.values().copied());
                    pending.extend(state.glob_mappings.values().copied());
                    for cache in state.method_calls.values() {
                        cache.trace(&mut pending);
                    }
                    continue;
                }
                Entry::CustomThis(value) => {
                    pending.extend(value.iter().filter_map(|value| self.traced_id(value)));
                    continue;
                }
                Entry::Object(object) => object,
            };
            if let Some(mirror) = &object.mirror {
                pending.extend(
                    mirror
                        .roots()
                        .iter()
                        .filter_map(|value| self.traced_id(value)),
                );
            }
            pending.extend(object.parent);
            pending.extend(object.instance_class);
            pending.extend(object.method_declaration.map(|(owner, _)| owner));
            if let Some(amendment) = &object.amendment {
                pending.extend(self.traced_id(&amendment.parent));
            }
            if let Some(pair) = &object.pair {
                pending.extend(pair.iter().filter_map(|value| self.traced_id(value)));
            }
            if let Some(entry) = &object.foreign_map_entry {
                pending.extend(entry.iter().filter_map(|value| self.traced_id(value)));
            }
            pending.extend(
                object
                    .annotations
                    .iter()
                    .filter_map(|value| self.traced_id(value)),
            );
            pending.extend(
                object
                    .member_annotations
                    .values()
                    .flatten()
                    .filter_map(|value| self.traced_id(value)),
            );
            pending.extend(
                object
                    .collection_cast
                    .as_ref()
                    .and_then(|cast| cast.node)
                    .map(|node| (node.0).0),
            );
            if let Some(function) = &object.function {
                pending.extend(
                    function
                        .captured_this
                        .iter()
                        .filter_map(|value| self.traced_id(value)),
                );
            }
            if let Some(reference) = &object.reference {
                pending.extend(self.traced_id(&reference.domain));
                pending.extend(self.traced_id(&reference.data));
                pending.extend(
                    reference
                        .path
                        .iter()
                        .filter_map(|value| self.traced_id(value)),
                );
                pending.extend(reference.classes.iter().copied());
            }
            if let Some(native) = &object.native_function {
                pending.extend(self.traced_id(&native.receiver));
            }
            if let Some(order) = &object.map_key_order {
                for member in order {
                    pending.extend(self.traced_id(&member.key));
                }
            }
            pending.extend(object.mapping_keys);
            for member in &object.collection_members {
                pending.extend(self.traced_id(&member.key));
            }
            if let Some(iterator) = &object.iterator {
                pending.push(iterator.source);
                for member in iterator.members.iter().chain(&iterator.force_members) {
                    pending.extend(self.traced_id(&member.key));
                }
            }
            if let Some(delegate) = &object.conversion_delegate {
                pending.push(delegate.source);
            }
            if let Some(iterator) = &object.bytes_iterator {
                pending.extend(self.traced_id(&crate::value::Value::Bytes(iterator.0.clone())));
            }
            if let Some(iterator) = &object.predicate_iterator {
                pending.push(iterator.source);
                pending.extend(
                    iterator
                        .previous_this
                        .iter()
                        .filter_map(|value| self.traced_id(value)),
                );
                for (owner, member) in &iterator.members {
                    pending.push(*owner);
                    pending.extend(self.traced_id(&member.key));
                }
            }
            for environment in object
                .member_environments
                .values()
                .chain(object.collection_cast.iter().map(|cast| &cast.environment))
            {
                pending.extend(environment.custom_this.map(|slot| slot.0));
                pending.extend(environment.executable_owner.map(|owner| owner.0));
                pending.extend(environment.code_owner.map(|owner| owner.0));
                pending.extend(environment.local_owners.values().map(|(owner, _)| owner.0));
                for scope in &environment.scopes {
                    pending.push(scope.receiver);
                    pending.push(scope.owner);
                }
                pending.extend(
                    environment
                        .locals
                        .iter()
                        .chain(environment.this_value.iter())
                        .chain(environment.outer_value.iter())
                        .filter_map(|value| self.traced_id(value)),
                );
            }
            pending.extend(object.declaration_mirror);
            pending.extend(object.module_mirror);
            pending.extend(
                object
                    .mirror_annotations
                    .values()
                    .flatten()
                    .filter_map(|value| self.traced_id(value)),
            );
            pending.extend(object.environment.custom_this.map(|slot| slot.0));
            pending.extend(object.environment.executable_owner.map(|owner| owner.0));
            pending.extend(object.environment.code_owner.map(|owner| owner.0));
            pending.extend(
                object
                    .environment
                    .local_owners
                    .values()
                    .map(|(owner, _)| owner.0),
            );
            for scope in &object.environment.scopes {
                pending.push(scope.receiver);
                pending.push(scope.owner);
            }
            for value in object
                .environment
                .locals
                .iter()
                .chain(object.environment.this_value.iter())
                .chain(object.environment.outer_value.iter())
                .chain(object.collection_default.iter().flatten())
                .chain(object.member_values.values())
                .chain(object.set_entries.iter())
                .chain(object.cache_slots.iter().map(|slot| &slot.value))
                .chain(
                    object
                        .cache_slots
                        .iter()
                        .filter_map(|slot| slot.entry_key.as_ref()),
                )
                .chain(object.cache.values().filter_map(|state| match state {
                    LazyState::Completed(value) => Some(value),
                    _ => None,
                }))
            {
                if let Some(id) = self.traced_id(value) {
                    pending.push(id);
                }
            }
            for key in object
                .cache
                .keys()
                .chain(object.cache_slots.iter().map(|slot| &slot.key))
            {
                pending.extend(key.owner);
            }
        }
        for (index, slot) in self.slots.iter_mut().enumerate() {
            if !marked[index]
                && let Some(entry) = slot.entry.take()
            {
                self.stats.executable_live -= usize::from(matches!(entry, Entry::Executable(_)));
                self.stats.live -= 1;
                self.stats.reclaimed += 1;
                // Exhausted generations are retired rather than wrapped.
                if let Some(generation) = slot.generation.checked_add(1) {
                    slot.generation = generation;
                    self.free.push(index);
                }
            }
        }
        let live: std::collections::BTreeSet<_> = self
            .slots
            .iter()
            .enumerate()
            .filter_map(|(index, slot)| {
                slot.object().map(|_| ObjectId {
                    heap: self.identity,
                    index,
                    generation: slot.generation,
                })
            })
            .collect();
        self.bytes_lists
            .retain(|_, (lifetime, id, _)| lifetime.strong_count() != 0 && live.contains(id));
        self.key_equalities
            .retain(|(a, b)| live.contains(a) && live.contains(b));
        // Canonical keys are metadata, not roots. Retain only keys referenced by
        // live objects and their structural components.
        let by_id: BTreeMap<_, _> = self
            .structural_keys
            .iter()
            .map(|(key, id)| (*id, key))
            .collect();
        let mut pending: Vec<_> = self
            .slots
            .iter()
            .filter_map(|slot| slot.object().and_then(|o| o.structural_key))
            .collect();
        let mut retained = std::collections::BTreeSet::new();
        while let Some(id) = pending.pop() {
            if retained.insert(id)
                && let Some(key) = by_id.get(&id)
            {
                pending.extend(key.children());
            }
        }
        self.structural_keys.retain(|_, id| retained.contains(id));
        self.stats.collections += 1;
        self.stats
    }

    pub fn clear(&mut self) {
        self.collect(&[]);
    }

    /// Permanently discard this arena's backing storage; all old IDs become invalid.
    pub fn release(&mut self) {
        self.stats.reclaimed += self.stats.live;
        self.stats.live = 0;
        self.stats.executable_live = 0;
        self.stats.slots = 0;
        self.stats.collections += 1;
        self.slots = Vec::new();
        self.free = Vec::new();
        self.structural_keys = BTreeMap::new();
        self.key_equalities.clear();
        self.bytes_lists.clear();
        self.identity = NEXT_HEAP.fetch_add(1, Ordering::Relaxed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nullable_defaults_are_null_values_with_checked_private_heap_edges() {
        let mut heap = Heap::new();
        let value = heap.allocate_null(Some(Value::Int(7.into())));
        assert_eq!(value, Value::Null);
        let id = value.heap_id().expect("default payload is traced");
        assert_eq!(heap.null_default(id), Ok(Value::Int(7.into())));
        assert_eq!(heap.get(id).unwrap_err().kind, ErrorKind::StaleHandle);
        assert_eq!(
            Heap::new().null_default(id).unwrap_err().kind,
            ErrorKind::StaleHandle
        );
        assert_eq!(heap.collect(std::slice::from_ref(&value)).live, 1);
        assert_eq!(heap.collect(&[]).live, 0);
        let replacement = heap.allocate_null(Some(Value::Int(8.into())));
        assert_ne!(replacement.heap_id(), Some(id));
        assert_eq!(
            heap.null_default(id).unwrap_err().kind,
            ErrorKind::StaleHandle
        );
        assert_eq!(heap.allocate_null(None), Value::Null);
    }

    #[test]
    fn deep_nullable_defaults_trace_and_release_iteratively_on_small_stack() {
        std::thread::Builder::new()
            .stack_size(256 * 1024)
            .spawn(|| {
                let mut heap = Heap::new();
                let mut value = Value::Object(heap.allocate(Object::default()));
                for _ in 0..20_000 {
                    value = heap.allocate_null(Some(value));
                }
                assert_eq!(heap.collect(std::slice::from_ref(&value)).live, 20_001);
                assert_eq!(heap.collect(&[]).live, 0);
                value = Value::Object(heap.allocate(Object::default()));
                for _ in 0..20_000 {
                    value = heap.allocate_null(Some(value));
                }
                // Arena destruction releases edges as handles, not Rust recursion.
                drop(heap);
            })
            .unwrap()
            .join()
            .unwrap();
    }

    #[test]
    fn captured_executable_graphs_trace_children_without_retaining_receivers() {
        let mut heap = Heap::new();
        let owner = heap.allocate_executable();
        let child = heap
            .executable_child(owner, ExecutionSite::Member(7))
            .unwrap();
        let capture = heap.allocate(Object {
            environment: Environment {
                executable_owner: Some(owner),
                ..Environment::default()
            },
            ..Object::default()
        });
        let retained = heap.collect(&[Value::Object(capture)]);
        assert_eq!(retained.live, 3);
        assert_eq!(retained.executable_live, 2);
        assert_eq!(
            heap.executable_child(owner, ExecutionSite::Member(7))
                .unwrap(),
            child
        );
        assert!(heap.get(owner.0).is_err());
        // A static owner may persist, but must not keep a prior lexical receiver.
        let retained = heap.collect(&[owner.root()]);
        assert_eq!(retained.live, 2);
        assert_eq!(retained.executable_live, 2);
        assert!(heap.get(capture).is_err());
        assert_eq!(heap.collect(&[]).live, 0);
        assert_eq!(heap.stats().executable_live, 0);
        let replacement = heap.allocate_executable();
        assert!(
            heap.executable_child(owner, ExecutionSite::Member(7))
                .is_err()
        );
        assert!(
            Heap::new()
                .executable_child(replacement, ExecutionSite::Member(7))
                .is_err()
        );
        heap.release();
        assert!(
            heap.executable_child(replacement, ExecutionSite::Member(7))
                .is_err()
        );
    }

    #[test]
    fn lazy_cast_retains_its_type_node_without_a_live_execution_frame() {
        let mut heap = Heap::new();
        let owner = heap.allocate_executable();
        let node = TypeNodeId(
            heap.executable_child(owner, ExecutionSite::TypeChild(0))
                .unwrap(),
        );
        let wrapper = heap.allocate(Object {
            collection_cast: Some(CollectionCast {
                source: None,
                node: Some(node),
                ty: crate::vm::TypeCheck::Int,
                span: crate::source::Span::new(0, 1),
                member: "#values".into(),
                environment: Environment::default(),
            }),
            ..Object::default()
        });
        let retained = heap.collect(&[Value::Object(wrapper)]);
        assert_eq!(retained.live, 2);
        assert_eq!(retained.executable_live, 1);
        assert!(heap.executable(owner).is_err());
        assert!(heap.executable(node.0).is_ok());
        assert_eq!(heap.collect(&[]).live, 0);
        assert!(heap.executable(node.0).is_err());
    }

    #[test]
    fn custom_receiver_cycles_are_traced_and_reclaimed_in_the_object_arena() {
        let mut heap = Heap::new();
        let slot = heap.allocate_custom_this(None);
        let object = heap.allocate(Object {
            environment: Environment {
                custom_this: Some(slot),
                ..Environment::default()
            },
            ..Object::default()
        });
        heap.set_custom_this(slot, Some(Value::Object(object)))
            .unwrap();
        assert_eq!(heap.collect(&[slot.root()]).live, 2);
        assert_eq!(heap.custom_this(slot).unwrap(), Some(Value::Object(object)));
        assert!(heap.get(object).is_ok());
        assert!(
            heap.get(slot.0).is_err(),
            "internal slots are not Pkl objects"
        );
        assert_eq!(heap.collect(&[Value::Object(object)]).live, 2);
        assert_eq!(heap.collect(&[]).live, 0);
        assert!(heap.custom_this(slot).is_err());
        let replacement = heap.allocate_custom_this(Some(Value::Null));
        assert!(
            heap.custom_this(slot).is_err(),
            "reused slots reject old generations"
        );
        assert_eq!(heap.custom_this(replacement).unwrap(), Some(Value::Null));
        assert!(Heap::new().custom_this(replacement).is_err());
        heap.release();
        assert!(heap.custom_this(replacement).is_err());
        assert_eq!(heap.stats().slots, 0);
    }

    #[test]
    fn function_snapshot_survives_an_independent_receiver_slot_mutation() {
        let mut heap = Heap::new();
        let snapshot = heap.allocate(Object::default());
        let slot = heap.allocate_custom_this(Some(Value::Object(snapshot)));
        let function = heap.allocate(Object {
            function: Some(Function {
                block: 0,
                arity: 0,
                method: false,
                captured_this: Some(Value::Object(snapshot)),
            }),
            environment: Environment {
                custom_this: Some(slot),
                ..Environment::default()
            },
            ..Object::default()
        });
        heap.set_custom_this(slot, Some(Value::Function(function)))
            .unwrap();
        assert_eq!(heap.collect(&[Value::Function(function)]).live, 3);
        assert!(heap.get(snapshot).is_ok());
        assert_eq!(
            heap.custom_this(slot).unwrap(),
            Some(Value::Function(function))
        );
        assert_eq!(heap.collect(&[]).live, 0);
    }
}
