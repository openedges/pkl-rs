//! Canonical runtime classes and their separate instance prototypes.
use super::*;
use crate::heap::CollectionKind;
mod ordinary;
pub(super) use ordinary::builtin_members as ordinary_members;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum BuiltinClass {
    Any,
    Null,
    Boolean,
    Int,
    Float,
    Number,
    String,
    Bytes,
    Duration,
    Object,
    Typed,
    DataSize,
    Regex,
    IntSeq,
    VarArgs,
    Class,
    TypeAlias,
    Function(Option<usize>),
    Pair,
    Collection,
    Dynamic,
    Listing,
    Mapping,
    List,
    Set,
    Map,
    Reference,
    Domain,
    Access,
    Ordinary(usize),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum RuntimeClass {
    Builtin(BuiltinClass),
    Declared(ObjectId),
}

pub(super) fn typed_instance(heap: &Heap, id: ObjectId) -> Result<bool, RuntimeError> {
    let object = heap.get(id)?;
    if object.pair.is_some() || object.reference.is_some() {
        return Ok(false);
    }
    let Some(class) = heap.object_class(id)? else {
        return Ok(false);
    };
    let prototype = heap.get(heap.get_class(class)?.prototype)?;
    Ok(!matches!(
        prototype
            .builtin_type
            .as_ref()
            .and_then(BuiltinClass::from_type),
        Some(
            BuiltinClass::Dynamic
                | BuiltinClass::Listing
                | BuiltinClass::Mapping
                | BuiltinClass::List
                | BuiltinClass::Set
                | BuiltinClass::Map
        )
    ))
}

pub(super) fn runtime_class(value: &Value, heap: &Heap) -> Result<RuntimeClass, RuntimeError> {
    use BuiltinClass as B;
    let class = match value {
        Value::Null | Value::NullWithDefault(_) => B::Null,
        Value::Bool(_) => B::Boolean,
        Value::Int(_) => B::Int,
        Value::Float(_) => B::Float,
        Value::Duration(_) => B::Duration,
        Value::DataSize(_) => B::DataSize,
        Value::String(_) => B::String,
        Value::Bytes(_) => B::Bytes,
        Value::IntSeq(_) => B::IntSeq,
        Value::Regex(_) => B::Regex,
        Value::Class(_) => B::Class,
        Value::TypeAlias(_) => B::TypeAlias,
        Value::Function(id) => {
            B::Function(Some(crate::diagnostic_values::function_arity(*id, heap)?))
        }
        Value::Object(id) => {
            let object = heap.get(*id)?;
            if object.foreign_map_entry.is_some() {
                return Err(RuntimeError::foreign(
                    crate::java_exception::provenance::ForeignOperation::KeyValueHolderClassCast,
                    "class java.util.KeyValueHolder cannot be cast to class org.pkl.core.runtime.VmValue (java.util.KeyValueHolder is in module java.base of loader 'bootstrap'; org.pkl.core.runtime.VmValue is in unnamed module of loader 'app')",
                ));
            }
            if object.pair.is_some() {
                B::Pair
            } else if object.reference.is_some() {
                B::Reference
            } else if let Some(class) = heap.object_class(*id)? {
                return Ok(RuntimeClass::Declared(class));
            } else {
                B::from_type(&TypeCheck::Collection(object.collection_kind))
                    .expect("collection class")
            }
        }
    };
    Ok(RuntimeClass::Builtin(class))
}

#[inline]
pub(super) fn accepts_nominal(id: usize, value: &Value, heap: &Heap) -> bool {
    if id == TYPED {
        // Java's TypedTypeNode tests VmTyped storage, not the class graph.
        // A permitted pkl.* declaration can extend Any and still be VmTyped.
        return matches!(value, Value::Object(object) if typed_instance(heap, *object).unwrap_or(false));
    }
    if let Value::Object(obj_id) = value
        && let Some(object) = heap.get_fast(*obj_id)
        && object.foreign_map_entry.is_none()
        && object.pair.is_none()
        && object.reference.is_none()
    {
        match heap.object_class(*obj_id) {
            Ok(Some(class)) => return heap.class_has_ancestor_id(class, id),
            Ok(None) => {}
            Err(_) => return false,
        }
        let mut class = BuiltinClass::from_type(&TypeCheck::Collection(object.collection_kind))
            .expect("collection class");
        loop {
            if class.id() == id {
                return true;
            }
            let Some(parent) = class.parent() else {
                return false;
            };
            class = parent;
        }
    }
    match runtime_class(value, heap) {
        Ok(RuntimeClass::Declared(class)) => heap.class_has_ancestor_id(class, id),
        Ok(RuntimeClass::Builtin(mut class)) => loop {
            if class.id() == id {
                return true;
            }
            let Some(parent) = class.parent() else {
                return false;
            };
            class = parent;
        },
        Err(_) => false,
    }
}

pub(super) fn specialized_type(module: Option<&PklString>, name: &PklString) -> Option<TypeCheck> {
    // UnresolvedTypeNode.Declared selects these intrinsic type nodes by name.
    // This must not merge the identity of a user-declared pkl.base class with
    // the canonical builtin Class value used by the selected type node.
    if !module.is_some_and(|module| module == "pkl.base") {
        return None;
    }
    let name = name.as_str()?;
    match name {
        "String" | "Boolean" | "Int" | "Float" | "Number" | "Any" | "Typed" | "Dynamic" => {
            reference::base_type(name)
        }
        _ => None,
    }
}

pub(crate) const OBJECT: usize = usize::MAX - 40;
pub(crate) const TYPED: usize = usize::MAX - 41;
const DATA_SIZE: usize = usize::MAX - 42;
pub(super) const REGEX: usize = usize::MAX - 43;
const INT_SEQ: usize = usize::MAX - 44;
const VAR_ARGS: usize = usize::MAX - 45;

pub(crate) fn named_type(name: &str) -> Option<TypeCheck> {
    let id = match name {
        "Object" => OBJECT,
        "Typed" => TYPED,
        "DataSize" => DATA_SIZE,
        "Regex" => REGEX,
        "IntSeq" => INT_SEQ,
        "VarArgs" => VAR_ARGS,
        _ => return None,
    };
    Some(TypeCheck::Class {
        module: None,
        id,
        name: name.into(),
    })
}

pub(crate) fn base_class_name(id: usize) -> Option<&'static str> {
    Some(match usize::MAX.checked_sub(id)? {
        10 => "Any",
        12 => "Null",
        13 => "Boolean",
        14 => "Int",
        15 => "Float",
        16 => "Number",
        17 => "String",
        18 => "Bytes",
        19 => "Dynamic",
        20 => "Listing",
        21 => "Mapping",
        22 => "List",
        23 => "Class",
        24 => "TypeAlias",
        25 => "Map",
        26 => "Set",
        27 => "Collection",
        28 => "Pair",
        29 => "Duration",
        30 => "Function",
        31 => "Function0",
        32 => "Function1",
        33 => "Function2",
        34 => "Function3",
        35 => "Function4",
        36 => "Function5",
        40 => "Object",
        41 => "Typed",
        42 => "DataSize",
        43 => "Regex",
        44 => "IntSeq",
        45 => "VarArgs",
        _ => return None,
    })
}

impl BuiltinClass {
    /// Pinned base/ref declaration arities, selected by canonical class identity.
    pub(super) fn type_parameter_count(self) -> usize {
        match self {
            Self::List
            | Self::Set
            | Self::Listing
            | Self::Collection
            | Self::Class
            | Self::VarArgs
            | Self::Function(None) => 1,
            Self::Map | Self::Mapping | Self::Pair | Self::Reference => 2,
            Self::Function(Some(arity)) => arity + 1,
            _ => 0,
        }
    }

    pub(super) fn parameterized_type(
        self,
        mut arguments: Vec<(TypeCheck, Span)>,
        span: Span,
    ) -> Result<TypeCheck, RuntimeError> {
        use CollectionKind as C;
        Ok(match self {
            Self::List | Self::Set | Self::Listing | Self::Map | Self::Mapping => {
                let kind = match self {
                    Self::List => C::List,
                    Self::Set => C::Set,
                    Self::Listing => C::Listing,
                    Self::Map => C::Map,
                    Self::Mapping => C::Mapping,
                    _ => unreachable!(),
                };
                TypeCheck::ParameterizedCollection { kind, arguments }
            }
            Self::Collection => TypeCheck::CollectionBase { arguments },
            Self::Pair => TypeCheck::Pair { arguments },
            Self::Class => TypeCheck::MetaClass("Class".into()),
            Self::Function(arity) => {
                let result = Box::new(arguments.pop().expect("function result argument").0);
                TypeCheck::Function {
                    parameters: arity.map(|_| arguments.into_iter().map(|(ty, _)| ty).collect()),
                    result,
                    syntax: false,
                }
            }
            Self::Reference => {
                let referent = Box::new(arguments.pop().expect("reference referent").0);
                let domain = Box::new(arguments.pop().expect("reference domain").0);
                TypeCheck::Reference { domain, referent }
            }
            Self::VarArgs => TypeCheck::VarArgs {
                element: Box::new(arguments.pop().expect("VarArgs argument").0),
                span,
            },
            _ => return Err(invalid("parameterized non-generic builtin class")),
        })
    }

    pub(super) fn from_type(ty: &TypeCheck) -> Option<Self> {
        use CollectionKind as C;
        Some(match ty.unaliased() {
            TypeCheck::Any => Self::Any,
            TypeCheck::Null => Self::Null,
            TypeCheck::Boolean => Self::Boolean,
            TypeCheck::Int => Self::Int,
            TypeCheck::Float => Self::Float,
            TypeCheck::Number => Self::Number,
            TypeCheck::String => Self::String,
            TypeCheck::Bytes => Self::Bytes,
            TypeCheck::Duration => Self::Duration,
            TypeCheck::MetaClass(name) if name == "Class" => Self::Class,
            TypeCheck::MetaClass(name) if name == "TypeAlias" => Self::TypeAlias,
            TypeCheck::Function { parameters, .. } => {
                Self::Function(parameters.as_ref().map(Vec::len))
            }
            TypeCheck::Pair { .. } => Self::Pair,
            TypeCheck::CollectionBase { .. } => Self::Collection,
            TypeCheck::Collection(kind) | TypeCheck::ParameterizedCollection { kind, .. } => {
                match kind {
                    C::Dynamic => Self::Dynamic,
                    C::Listing => Self::Listing,
                    C::Mapping => Self::Mapping,
                    C::List => Self::List,
                    C::Set => Self::Set,
                    C::Map => Self::Map,
                }
            }
            TypeCheck::RefDomain => Self::Domain,
            TypeCheck::Reference { .. } => Self::Reference,
            TypeCheck::VarArgs { .. } => Self::VarArgs,
            TypeCheck::Class {
                id: reference::ACCESS_ID,
                ..
            } => Self::Access,
            TypeCheck::Class { id: OBJECT, .. } => Self::Object,
            TypeCheck::Class { id: TYPED, .. } => Self::Typed,
            TypeCheck::Class { id: DATA_SIZE, .. } => Self::DataSize,
            TypeCheck::Class { id: REGEX, .. } => Self::Regex,
            TypeCheck::Class { id: INT_SEQ, .. } => Self::IntSeq,
            TypeCheck::Class { id: VAR_ARGS, .. } => Self::VarArgs,
            TypeCheck::Class { id, .. } if builtins::name(*id).is_some() => Self::Ordinary(*id),
            _ => return None,
        })
    }

    fn id(self) -> usize {
        usize::MAX
            - match self {
                Self::Domain => 0,
                Self::Access => 1,
                Self::Reference => 2,
                Self::Any => 10,
                Self::Null => 12,
                Self::Boolean => 13,
                Self::Int => 14,
                Self::Float => 15,
                Self::Number => 16,
                Self::String => 17,
                Self::Bytes => 18,
                Self::Dynamic => 19,
                Self::Listing => 20,
                Self::Mapping => 21,
                Self::List => 22,
                Self::Class => 23,
                Self::TypeAlias => 24,
                Self::Map => 25,
                Self::Set => 26,
                Self::Collection => 27,
                Self::Pair => 28,
                Self::Duration => 29,
                Self::Function(None) => 30,
                Self::Function(Some(n)) => 31 + n,
                Self::Object => 40,
                Self::Typed => 41,
                Self::DataSize => 42,
                Self::Regex => 43,
                Self::IntSeq => 44,
                Self::VarArgs => 45,
                Self::Ordinary(id) => return id,
            }
    }

    fn name(self) -> String {
        match self {
            Self::Function(None) => "Function".into(),
            Self::Function(Some(n)) => format!("Function{n}"),
            Self::Ordinary(id) => builtins::name(id).expect("ordinary builtin").into(),
            _ => format!("{self:?}"),
        }
    }

    fn parent(self) -> Option<Self> {
        Some(match self {
            Self::Any => return None,
            Self::Int | Self::Float => Self::Number,
            Self::Typed | Self::Dynamic | Self::Listing | Self::Mapping => Self::Object,
            Self::List | Self::Set => Self::Collection,
            Self::Function(Some(_)) => Self::Function(None),
            Self::Domain | Self::Access | Self::Reference | Self::Regex | Self::VarArgs => {
                Self::Typed
            }
            Self::Ordinary(id) => builtins::parent(id).map_or(Self::Typed, Self::Ordinary),
            _ => Self::Any,
        })
    }

    fn abstract_(self) -> bool {
        matches!(
            self,
            Self::Any
                | Self::Number
                | Self::Object
                | Self::Typed
                | Self::Function(None)
                | Self::Collection
                | Self::Domain
        ) || matches!(self, Self::Ordinary(id) if builtins::abstract_class(id))
    }

    pub(super) fn external(self) -> bool {
        !matches!(
            self,
            Self::Typed
                | Self::Dynamic
                | Self::Listing
                | Self::Mapping
                | Self::Domain
                | Self::Access
        ) && !matches!(self, Self::Ordinary(id) if id != output::MODULE)
    }

    fn ty(self) -> TypeCheck {
        if self == Self::Reference {
            return TypeCheck::Reference {
                domain: Box::new(TypeCheck::Any),
                referent: Box::new(TypeCheck::Any),
            };
        }
        if self == Self::Domain {
            return TypeCheck::RefDomain;
        }
        reference::base_type(&self.name()).unwrap_or_else(|| TypeCheck::Class {
            module: None,
            id: self.id(),
            name: self.name().into(),
        })
    }
}

impl Vm {
    pub(super) fn canonical_type_class(
        &self,
        class: ObjectId,
    ) -> Result<Option<BuiltinClass>, RuntimeError> {
        let prototype = self.heap.get(self.heap.get_class(class)?.prototype)?;
        Ok(prototype
            .builtin_type
            .as_ref()
            .and_then(BuiltinClass::from_type)
            .filter(|key| self.builtin_classes.get(key) == Some(&class)))
    }

    pub(super) fn class_member(
        &mut self,
        receiver: &Value,
        name: &PklString,
        method: bool,
    ) -> Result<bool, RuntimeError> {
        let Value::Class(class) = receiver else {
            return Ok(false);
        };
        self.heap.get_class(*class)?;
        if !method && name == "simpleName" {
            self.stack
                .push(Value::string(self.heap.class_simple_name(*class)?));
        } else if method && matches!(name.as_str(), Some("getClass" | "toString")) {
            let function = self.native_reference_method(
                receiver.clone(),
                if name == "toString" {
                    "scalarToString"
                } else {
                    name.as_str().expect("recognized Class method")
                },
            );
            self.stack.push(function);
        } else {
            let metaclass = self.canonical_builtin_class(BuiltinClass::Class);
            return Err(self.missing_member(
                self.heap.get_class(metaclass)?.prototype,
                name,
                method,
            )?);
        }
        Ok(true)
    }

    pub(super) fn canonical_builtin_class(&mut self, key: BuiltinClass) -> ObjectId {
        if let Some(&id) = self.builtin_classes.get(&key) {
            return id;
        }
        let mut chain = vec![key];
        let mut current = key;
        while let Some(parent) = current.parent() {
            if self.builtin_classes.contains_key(&parent) {
                break;
            }
            chain.push(parent);
            current = parent;
        }
        while let Some(key) = chain.pop() {
            if self.builtin_classes.contains_key(&key) {
                continue;
            }
            let id = key.id();
            let members = if key == BuiltinClass::Access {
                crate::heap::MemberList::from_arc(Arc::clone(&self.program.access_members))
            } else {
                self.program
                    .builtin_members
                    .get(&id)
                    .cloned()
                    .unwrap_or_default()
                    .into()
            };
            let prototype = self.heap.allocate(Object {
                members,
                collection_kind: match key {
                    BuiltinClass::Listing => CollectionKind::Listing,
                    BuiltinClass::Mapping => CollectionKind::Mapping,
                    _ => CollectionKind::Dynamic,
                },
                builtin_type: Some(key.ty()),
                class_definition: Some(Class {
                    module: Some(
                        if matches!(
                            key,
                            BuiltinClass::Reference | BuiltinClass::Domain | BuiltinClass::Access
                        ) {
                            "pkl.ref"
                        } else {
                            "pkl.base"
                        }
                        .into(),
                    ),
                    origin_uri: Some(
                        if matches!(
                            key,
                            BuiltinClass::Reference | BuiltinClass::Domain | BuiltinClass::Access
                        ) {
                            "pkl:ref"
                        } else {
                            "pkl:base"
                        }
                        .into(),
                    ),
                    id,
                    name: key.name().into(),
                    abstract_: key.abstract_(),
                    open: matches!(key, BuiltinClass::Ordinary(id) if builtins::open_class(id)),
                }),
                ..Object::default()
            });
            let class = self.heap.allocate_class(prototype);
            self.builtin_classes.insert(key, class);
            self.builtin_fast_classes.record(key, class);
            let superclass = key
                .parent()
                .and_then(|parent| self.builtin_classes.get(&parent).copied());
            self.heap
                .link_superclass(class, superclass)
                .expect("new builtin class graph");
        }
        *self
            .builtin_classes
            .get(&key)
            .expect("canonical builtin class")
    }

    pub(super) fn builtin_class(&mut self, ty: TypeCheck) -> Value {
        let key = BuiltinClass::from_type(&ty).expect("resolved builtin class type");
        Value::Class(self.canonical_builtin_class(key))
    }

    pub(super) fn class_external(&self, class: ObjectId) -> Result<bool, RuntimeError> {
        let prototype = self.heap.get(self.heap.get_class(class)?.prototype)?;
        Ok(prototype
            .builtin_type
            .as_ref()
            .and_then(BuiltinClass::from_type)
            .is_some_and(BuiltinClass::external)
            || prototype
                .declaration_code
                .and_then(|code| self.program.declaration_source(code))
                .is_some_and(|declaration| {
                    crate::syntax::modifier_flags(&declaration.modifiers)
                        & crate::syntax::ModifierKind::External.flag()
                        != 0
                }))
    }
}

#[cfg(test)]
mod tests;
