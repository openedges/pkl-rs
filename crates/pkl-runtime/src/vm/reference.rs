//! Deferred references use declaration types without executing referent bodies.
use super::*;
use crate::heap::{CollectionKind, CollectionMember, CollectionMemberKind};

pub(crate) const DOMAIN_ID: usize = usize::MAX;
pub(crate) const MODULE_ID: usize = usize::MAX - 3;
pub(crate) const ACCESS_ID: usize = usize::MAX - 1;

#[derive(Clone, Debug)]
pub struct Reference {
    pub domain: Value,
    pub data: Value,
    pub path: Vec<Value>,
    pub referent: TypeCheck,
    pub classes: Vec<ObjectId>,
    pub forced: bool,
}
#[derive(Clone, Debug)]
pub struct NativeFunction {
    pub receiver: Value,
    pub name: Arc<str>,
}

pub(crate) fn base_type(name: &str) -> Option<TypeCheck> {
    Some(match name {
        "Class" | "TypeAlias" => TypeCheck::MetaClass(Arc::from(name)),
        "Function" | "Function0" | "Function1" | "Function2" | "Function3" | "Function4"
        | "Function5" => TypeCheck::Function {
            parameters: name
                .strip_prefix("Function")
                .and_then(|n| n.parse::<usize>().ok())
                .map(|n| vec![TypeCheck::Unknown; n]),
            result: Box::new(TypeCheck::Unknown),
            syntax: false,
        },
        "Any" => TypeCheck::Any,
        "Null" => TypeCheck::Null,
        "Boolean" => TypeCheck::Boolean,
        "Int" => TypeCheck::Int,
        "Float" => TypeCheck::Float,
        "Number" => TypeCheck::Number,
        "String" => TypeCheck::String,
        "Bytes" => TypeCheck::Bytes,
        "Dynamic" => TypeCheck::Collection(CollectionKind::Dynamic),
        "Listing" => TypeCheck::Collection(CollectionKind::Listing),
        "Mapping" => TypeCheck::Collection(CollectionKind::Mapping),
        "List" => TypeCheck::Collection(CollectionKind::List),
        _ => return None,
    })
}

pub(crate) fn type_name(ty: &TypeCheck, module: &str) -> String {
    match ty {
        TypeCheck::Unknown => "unknown".into(),
        TypeCheck::NamedAlias {
            name, arguments, ..
        } => {
            if arguments.is_empty() {
                format!("{module}#{name}")
            } else {
                format!(
                    "{module}#{name}<{}>",
                    arguments
                        .iter()
                        .map(|ty| type_name(ty, module))
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            }
        }
        TypeCheck::Function {
            parameters,
            result,
            syntax,
        } => {
            let args = parameters
                .iter()
                .flatten()
                .map(|ty| type_name(ty, module))
                .collect::<Vec<_>>();
            if *syntax {
                format!("({}) -> {}", args.join(", "), type_name(result, module))
            } else {
                let mut args = args;
                args.push(type_name(result, module));
                format!("{}<{}>", super::type_name(ty, module), args.join(", "))
            }
        }
        TypeCheck::RefDomain => "pkl.ref#Domain".into(),
        TypeCheck::Reference { domain, referent } => format!(
            "pkl.ref#Reference<{}, {}>",
            type_name(domain, module),
            type_name(referent, module)
        ),
        TypeCheck::Class { id: MODULE_ID, .. } => module.to_string(),
        TypeCheck::Class { id: ACCESS_ID, .. } => "pkl.ref#Access".into(),
        TypeCheck::Class { name, .. } => format!("{module}#{name}"),
        TypeCheck::Union { types, .. } => types
            .iter()
            .map(|t| type_name(t, module))
            .collect::<Vec<_>>()
            .join(" | "),
        TypeCheck::Nullable(t) => format!("{}?", type_name(t, module)),
        TypeCheck::Alias(t) | TypeCheck::GenericAlias(t) => type_name(t, module),
        TypeCheck::Constrained { base, .. } => type_name(base, module),
        TypeCheck::ParameterizedCollection { kind, arguments } => format!(
            "{kind:?}<{}>",
            arguments
                .iter()
                .map(|(t, _)| type_name(t, module))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        TypeCheck::Collection(kind) => format!("{kind:?}"),
        TypeCheck::StringLiteral(s) => format!("\"{s}\""),
        _ => super::type_name(ty, module),
    }
}

/// Java exports nullable types as flattened unions and erases constraints.
pub(crate) fn normalize(ty: &TypeCheck, module: &str) -> TypeCheck {
    match ty {
        TypeCheck::NonNull => TypeCheck::Any,
        TypeCheck::NamedAlias {
            expanded: inner, ..
        }
        | TypeCheck::Alias(inner)
        | TypeCheck::GenericAlias(inner)
        | TypeCheck::Constrained { base: inner, .. } => normalize(inner, module),
        TypeCheck::Nullable(inner) => {
            union(vec![normalize(inner, module), TypeCheck::Null], module)
        }
        TypeCheck::Union { types, .. } => union(
            types.iter().map(|ty| normalize(ty, module)).collect(),
            module,
        ),
        TypeCheck::Collection(
            kind @ (CollectionKind::List | CollectionKind::Listing | CollectionKind::Mapping),
        ) => TypeCheck::ParameterizedCollection {
            kind: *kind,
            arguments: vec![
                (TypeCheck::Unknown, Span::new(0, 0));
                if *kind == CollectionKind::Mapping {
                    2
                } else {
                    1
                }
            ],
        },
        TypeCheck::ParameterizedCollection { kind, arguments } => {
            TypeCheck::ParameterizedCollection {
                kind: *kind,
                arguments: arguments
                    .iter()
                    .map(|(t, _)| (normalize(t, module), Span::new(0, 0)))
                    .collect(),
            }
        }
        TypeCheck::Function {
            parameters,
            result,
            syntax: false,
        } => TypeCheck::Function {
            parameters: parameters
                .as_ref()
                .map(|p| p.iter().map(|t| normalize(t, module)).collect()),
            result: Box::new(normalize(result, module)),
            syntax: false,
        },
        TypeCheck::Reference { domain, referent } => TypeCheck::Reference {
            domain: Box::new(normalize(domain, module)),
            referent: Box::new(normalize(referent, module)),
        },
        ty => ty.clone(),
    }
}
fn union(types: Vec<TypeCheck>, module: &str) -> TypeCheck {
    let mut result = Vec::new();
    for ty in types {
        if let TypeCheck::Union { types, .. } = ty {
            result.extend(types);
        } else {
            result.push(ty);
        }
    }
    if result.iter().any(|t| matches!(t, TypeCheck::Unknown)) {
        return TypeCheck::Unknown;
    }
    if result.iter().any(|t| matches!(t, TypeCheck::Any)) {
        return TypeCheck::Any;
    }
    result.sort_by(|a, b| {
        type_name(a, module)
            .encode_utf16()
            .cmp(type_name(b, module).encode_utf16())
    });
    result.dedup_by(|a, b| metadata_key(a) == metadata_key(b));
    if result.len() == 1 {
        return result.remove(0);
    }
    TypeCheck::Union {
        types: result,
        spans: Vec::new(),
        labels: Vec::new(),
        default: None,
    }
}

/// Equality/hash identity of exported PType metadata, separate from checking
/// equivalence. Arrow types retain nested aliases and exact constraint source.
pub(crate) fn metadata_key(ty: &TypeCheck) -> String {
    match ty {
        TypeCheck::NamedAlias {
            declaration,
            arguments,
            ..
        } => format!(
            "alias:{declaration}:{:?}",
            arguments.iter().map(metadata_key).collect::<Vec<_>>()
        ),
        TypeCheck::Alias(inner) | TypeCheck::GenericAlias(inner) => metadata_key(inner),
        TypeCheck::Function {
            parameters,
            result,
            syntax,
        } => format!(
            "function:{syntax}:{:?}:{}",
            parameters
                .as_ref()
                .map(|p| p.iter().map(metadata_key).collect::<Vec<_>>()),
            metadata_key(result)
        ),
        TypeCheck::Constrained {
            base, constraints, ..
        } => format!(
            "constraint:{}:{:?}",
            metadata_key(base),
            constraints
                .iter()
                .map(|c| c.source.as_ref())
                .collect::<Vec<_>>()
        ),
        TypeCheck::Nullable(inner) => format!("nullable:{}", metadata_key(inner)),
        TypeCheck::Union { types, .. } => format!(
            "union:{:?}",
            types.iter().map(metadata_key).collect::<Vec<_>>()
        ),
        TypeCheck::ParameterizedCollection { kind, arguments } => format!(
            "collection:{kind:?}:{:?}",
            arguments
                .iter()
                .map(|(ty, _)| metadata_key(ty))
                .collect::<Vec<_>>()
        ),
        TypeCheck::Reference { domain, referent } => format!(
            "reference:{}:{}",
            metadata_key(domain),
            metadata_key(referent)
        ),
        other => format!("{other:?}"),
    }
}

pub(crate) fn is_subtype(a: &TypeCheck, b: &TypeCheck, heap: &Heap) -> bool {
    if matches!(a, TypeCheck::Unknown) || matches!(b, TypeCheck::Unknown | TypeCheck::Any) {
        return true;
    }
    if matches!(b, TypeCheck::Nothing) {
        return false;
    }
    subtype(a, b, heap)
}
fn contains_function_syntax(ty: &TypeCheck) -> bool {
    match ty {
        TypeCheck::Function { syntax: true, .. } => true,
        TypeCheck::Function {
            parameters, result, ..
        } => {
            parameters.iter().flatten().any(contains_function_syntax)
                || contains_function_syntax(result)
        }
        TypeCheck::ParameterizedCollection { arguments, .. } => {
            arguments.iter().any(|(ty, _)| contains_function_syntax(ty))
        }
        TypeCheck::Reference { domain, referent } => {
            contains_function_syntax(domain) || contains_function_syntax(referent)
        }
        TypeCheck::Union { types, .. } => types.iter().any(contains_function_syntax),
        _ => false,
    }
}

fn subtype(a: &TypeCheck, b: &TypeCheck, heap: &Heap) -> bool {
    // VmReference does not structurally compare exported function syntax types.
    if matches!(a, TypeCheck::Function { syntax: true, .. })
        || matches!(b, TypeCheck::Function { syntax: true, .. })
    {
        return false;
    }
    if a.equivalent(b) && !contains_function_syntax(a) && !contains_function_syntax(b) {
        return true;
    }
    match (a, b) {
        (TypeCheck::IntegerAlias { name: a, .. }, TypeCheck::IntegerAlias { name: b, .. }) => {
            match a.as_ref() {
                "Int8" => matches!(b.as_ref(), "Int16" | "Int32"),
                "Int16" => b.as_ref() == "Int32",
                "UInt8" => matches!(b.as_ref(), "Int16" | "Int32" | "UInt16" | "UInt32" | "UInt"),
                "UInt16" => matches!(b.as_ref(), "Int32" | "UInt32" | "UInt"),
                "UInt32" => b.as_ref() == "UInt",
                _ => false,
            }
        }
        (TypeCheck::IntegerAlias { .. }, TypeCheck::Int | TypeCheck::Float | TypeCheck::Number) => {
            true
        }
        (
            TypeCheck::Null
            | TypeCheck::Boolean
            | TypeCheck::Int
            | TypeCheck::Float
            | TypeCheck::Number
            | TypeCheck::String
            | TypeCheck::Bytes
            | TypeCheck::Class { .. }
            | TypeCheck::RefDomain
            | TypeCheck::Collection(_)
            | TypeCheck::ParameterizedCollection { .. }
            | TypeCheck::Reference { .. },
            TypeCheck::Any,
        ) => true,
        (TypeCheck::Class { id, .. }, TypeCheck::RefDomain) => subtype(
            &TypeCheck::Class {
                id: *id,
                name: Arc::from(""),
            },
            &TypeCheck::Class {
                id: DOMAIN_ID,
                name: Arc::from("Domain"),
            },
            heap,
        ),
        (
            TypeCheck::Reference {
                domain: ad,
                referent: at,
            },
            TypeCheck::Reference {
                domain: bd,
                referent: bt,
            },
        ) => subtype(ad, bd, heap) && subtype(at, bt, heap),
        (
            TypeCheck::Function {
                parameters: a,
                result: ar,
                syntax: false,
            },
            TypeCheck::Function {
                parameters: b,
                result: br,
                syntax: false,
            },
        ) => {
            subtype(ar, br, heap)
                && match (a, b) {
                    (None, None) => true,
                    (Some(a), None) => a.is_empty(),
                    (Some(a), Some(b)) => {
                        a.len() == b.len() && a.iter().zip(b).all(|(a, b)| subtype(b, a, heap))
                    }
                    _ => false,
                }
        }
        (TypeCheck::StringLiteral(_), TypeCheck::String) => true,
        (TypeCheck::Int | TypeCheck::Float, TypeCheck::Number) => true,
        (TypeCheck::Class { id, .. }, TypeCheck::Class { id: other, .. }) => {
            let mut current = heap.find_class(*id);
            let mut visited = BTreeSet::new();
            while let Some(id) = current {
                if !visited.insert(id) {
                    return false;
                }
                let Ok(object) = heap.get(id) else {
                    return false;
                };
                if object
                    .class_definition
                    .as_ref()
                    .is_some_and(|c| c.id == *other)
                {
                    return true;
                }
                current = object.parent;
            }
            false
        }
        (
            TypeCheck::ParameterizedCollection {
                kind: a,
                arguments: aa,
            },
            TypeCheck::ParameterizedCollection {
                kind: b,
                arguments: ba,
            },
        ) => {
            a == b
                && aa.len() == ba.len()
                && aa
                    .iter()
                    .zip(ba)
                    .all(|((a, _), (b, _))| subtype(a, b, heap))
        }
        (TypeCheck::Union { types: a, .. }, TypeCheck::Union { types: b, .. }) => {
            a.iter().all(|a| b.iter().any(|b| subtype(a, b, heap)))
        }
        (a, TypeCheck::Union { types, .. }) => types.iter().any(|b| subtype(a, b, heap)),
        _ => false,
    }
}

impl Vm {
    pub(super) fn builtin_class(&mut self, ty: TypeCheck) -> Value {
        if let Some(id) = self.heap.find_builtin_class(&ty) {
            return Value::Class(id);
        }
        let (id, name, abstract_) = match ty {
            TypeCheck::Class { id: ACCESS_ID, .. } => (ACCESS_ID, "Access".to_owned(), false),
            TypeCheck::RefDomain => (DOMAIN_ID, "Domain".to_owned(), true),
            _ => (
                usize::MAX
                    - match ty {
                        TypeCheck::MetaClass(ref name) if name.as_ref() == "Class" => 23,
                        TypeCheck::MetaClass(_) => 24,
                        TypeCheck::Function { ref parameters, .. } => {
                            30 + parameters.as_ref().map_or(0, |p| p.len() + 1)
                        }
                        TypeCheck::Any => 10,
                        TypeCheck::NonNull => 11,
                        TypeCheck::Null => 12,
                        TypeCheck::Boolean => 13,
                        TypeCheck::Int => 14,
                        TypeCheck::Float => 15,
                        TypeCheck::Number => 16,
                        TypeCheck::String => 17,
                        TypeCheck::Bytes => 18,
                        TypeCheck::Collection(CollectionKind::Dynamic) => 19,
                        TypeCheck::Collection(CollectionKind::Listing) => 20,
                        TypeCheck::Collection(CollectionKind::Mapping) => 21,
                        TypeCheck::Collection(CollectionKind::List) => 22,
                        _ => 2,
                    },
                super::type_name(&ty, &self.program.module_name),
                false,
            ),
        };
        let members = if id == ACCESS_ID {
            self.program.access_members.clone()
        } else {
            Vec::new()
        };
        let object = self.heap.allocate(Object {
            members,
            builtin_type: Some(ty),
            class_definition: Some(Class {
                id,
                name: Arc::from(name),
                open: false,
                abstract_,
            }),
            ..Object::default()
        });
        Value::Class(object)
    }

    pub(super) fn native_reference_method(&mut self, receiver: Value, name: &str) -> Value {
        let object = self.heap.allocate(Object {
            native_function: Some(NativeFunction {
                receiver,
                name: Arc::from(name),
            }),
            ..Object::default()
        });
        Value::Function(object)
    }

    pub(super) fn reference_member(
        &mut self,
        id: ObjectId,
        name: &str,
        method: bool,
    ) -> Result<bool, RuntimeError> {
        if self.heap.get(id)?.ref_module {
            let value = match (name, method) {
                ("Reference", true) => self.native_reference_method(Value::Object(id), "Reference"),
                ("Reference", false) => self.builtin_class(TypeCheck::Reference {
                    domain: Box::new(TypeCheck::Any),
                    referent: Box::new(TypeCheck::Any),
                }),
                ("Domain", false) => self.builtin_class(TypeCheck::RefDomain),
                ("Access", false) => self.builtin_class(TypeCheck::Class {
                    id: ACCESS_ID,
                    name: Arc::from("Access"),
                }),
                _ => {
                    return Err(RuntimeError::new(
                        ErrorKind::Unsupported,
                        format!("pkl:ref member `{name}` is not implemented"),
                    ));
                }
            };
            self.stack.push(value);
            return Ok(true);
        }
        if self.heap.get(id)?.reference.is_none() {
            return Ok(false);
        }
        let value = if method {
            match name {
                "getDomain" | "getData" | "getPath" | "toString" => {
                    self.native_reference_method(Value::Object(id), name)
                }
                _ => {
                    return Err(RuntimeError::new(
                        ErrorKind::MissingMember,
                        format!("Cannot find method `{name}` in class `pkl.ref#Reference`."),
                    ));
                }
            }
        } else {
            self.reference_access(id, Some(name), None)?
        };
        self.stack.push(value);
        Ok(true)
    }

    pub(super) fn invoke_reference_native(
        &mut self,
        native: NativeFunction,
        args: Vec<Value>,
    ) -> Result<(), RuntimeError> {
        let expected = if native.name.as_ref() == "Reference" {
            3
        } else {
            0
        };
        if args.len() != expected {
            return Err(RuntimeError::new(
                ErrorKind::Type,
                format!(
                    "Expected {expected} function arguments but got {}.",
                    args.len()
                ),
            ));
        }
        if native.name.as_ref() == "getClass" {
            let value = self.reference_get_class(&native.receiver)?;
            self.stack.push(value);
            return Ok(());
        }
        if native.name.as_ref() == "scalarToString" {
            let text = match &native.receiver {
                Value::TypeAlias(_) | Value::Class(_) | Value::Function(_) => {
                    crate::diagnostic_values::render(
                        &native.receiver,
                        &self.heap,
                        &self.program.module_name,
                        usize::MAX,
                    )?
                }
                value => scalar_string(value)?,
            };
            self.stack.push(Value::string(text));
            return Ok(());
        }
        let value = if native.name.as_ref() == "Reference" {
            if !TypeCheck::RefDomain.accepts(&args[0], &self.heap) {
                let value = crate::diagnostic_values::render(
                    &args[0],
                    &self.heap,
                    &self.program.module_name,
                    80,
                )?;
                return Err(constructor_error(
                    format!("Type constraint `this is Domain` violated.\nValue: {value}"),
                    "this is Domain",
                ));
            }
            let Value::Class(class_id) = args[1] else {
                let value = crate::diagnostic_values::render(
                    &args[1],
                    &self.heap,
                    &self.program.module_name,
                    80,
                )?;
                let actual = crate::diagnostic_values::type_name(
                    &args[1],
                    &self.heap,
                    &self.program.module_name,
                )?;
                return Err(constructor_error(
                    format!(
                        "Expected value of type `Class`, but got type `{actual}`.\nValue: {value}"
                    ),
                    "Class<T>",
                ));
            };
            let object = self.heap.get(class_id)?;
            let class = object
                .class_definition
                .as_ref()
                .ok_or_else(|| invalid("class metadata missing"))?;
            let ty = object
                .builtin_type
                .clone()
                .unwrap_or_else(|| TypeCheck::Class {
                    id: class.id,
                    name: class.name.clone(),
                });
            let reference = Reference {
                domain: args[0].clone(),
                data: args[2].clone(),
                path: Vec::new(),
                referent: normalize(&ty, &self.program.module_name),
                classes: vec![class_id],
                forced: false,
            };
            Value::Object(self.heap.allocate(Object {
                reference: Some(reference),
                ..Object::default()
            }))
        } else {
            let id = native.receiver.as_object()?;
            let reference = self
                .heap
                .get(id)?
                .reference
                .clone()
                .ok_or_else(|| invalid("reference native receiver"))?;
            match native.name.as_ref() {
                "getDomain" => reference.domain,
                "getData" => reference.data,
                "getPath" => self.reference_list(reference.path),
                "toString" => {
                    self.render_reference(id)?;
                    return Ok(());
                }
                _ => return Err(invalid("unknown reference native method")),
            }
        };
        self.stack.push(value);
        Ok(())
    }

    fn reference_get_class(&mut self, value: &Value) -> Result<Value, RuntimeError> {
        let ty = match value {
            Value::Null => TypeCheck::Null,
            Value::Bool(_) => TypeCheck::Boolean,
            Value::Int(_) => TypeCheck::Int,
            Value::Float(_) => TypeCheck::Float,
            Value::String(_) => TypeCheck::String,
            Value::Bytes(_) => TypeCheck::Bytes,
            Value::TypeAlias(_) => TypeCheck::MetaClass(Arc::from("TypeAlias")),
            Value::Class(_) => TypeCheck::MetaClass(Arc::from("Class")),
            Value::Function(id) => {
                let object = self.heap.get(*id)?;
                let arity = object
                    .function
                    .as_ref()
                    .map(|f| f.arity)
                    .or_else(|| {
                        object
                            .native_function
                            .as_ref()
                            .map(|f| if f.name.as_ref() == "Reference" { 3 } else { 0 })
                    })
                    .ok_or_else(|| invalid("function metadata missing"))?;
                TypeCheck::Function {
                    parameters: Some(vec![TypeCheck::Unknown; arity]),
                    result: Box::new(TypeCheck::Unknown),
                    syntax: false,
                }
            }
            Value::Object(id) => {
                let object = self.heap.get(*id)?;
                if object.reference.is_some() {
                    TypeCheck::Reference {
                        domain: Box::new(TypeCheck::Any),
                        referent: Box::new(TypeCheck::Any),
                    }
                } else if object.is_module {
                    if let Some(id) = self.heap.find_class(MODULE_ID) {
                        return Ok(Value::Class(id));
                    }
                    let class = self.heap.allocate(Object {
                        class_definition: Some(Class {
                            id: MODULE_ID,
                            name: self.program.module_name.clone(),
                            open: false,
                            abstract_: false,
                        }),
                        members: object.members.clone(),
                        environment: Environment {
                            scopes: vec![Scope {
                                receiver: *id,
                                owner: *id,
                            }],
                            ..Environment::default()
                        },
                        ..Object::default()
                    });
                    return Ok(Value::Class(class));
                } else if let Some(class) = collections::typed_class(&self.heap, *id)? {
                    return Ok(Value::Class(
                        self.heap
                            .find_class(class.id)
                            .ok_or_else(|| invalid("missing class"))?,
                    ));
                } else {
                    TypeCheck::Collection(object.collection_kind)
                }
            }
        };
        Ok(self.builtin_class(ty))
    }

    pub(super) fn render_reference(&mut self, id: ObjectId) -> Result<(), RuntimeError> {
        let function = self.heap.allocate(Object {
            function: Some(Function {
                block: self.program.reference_render_block,
                arity: 0,
                method: true,
                capture_custom_this: false,
            }),
            environment: Environment {
                locals: vec![Value::Object(id)],
                ..Environment::default()
            },
            ..Object::default()
        });
        self.invoke_function(function, Vec::new(), true)
    }

    fn reference_list(&mut self, values: Vec<Value>) -> Value {
        let mut object = Object {
            collection_kind: CollectionKind::List,
            length: values.len(),
            ..Object::default()
        };
        for (index, value) in values.into_iter().enumerate() {
            let name: Arc<str> = Arc::from(format!("\0{index}"));
            object.members.push(Member {
                entry: true,
                name: name.clone(),
                code: usize::MAX,
                local: false,
                method: false,
                ty: None,
                type_span: None,
                constant: false,
                fixed: false,
            });
            object.collection_members.push(CollectionMember {
                key: Value::Int(index as i64),
                name: name.clone(),
                kind: CollectionMemberKind::Element,
            });
            object.complete_cache(
                CacheKey {
                    entry: true,
                    owner: None,
                    name,
                    method: false,
                },
                value,
            );
        }
        Value::Object(self.heap.allocate(object))
    }

    fn reference_access_value(&mut self, property: Option<&str>, key: Option<Value>) -> Value {
        let Value::Class(parent) = self.builtin_class(TypeCheck::Class {
            id: ACCESS_ID,
            name: Arc::from("Access"),
        }) else {
            unreachable!()
        };
        let mut object = Object {
            parent: Some(parent),
            ..Object::default()
        };
        for (name, value) in [
            ("property", property.map_or(Value::Null, Value::string)),
            ("key", key.unwrap_or(Value::Null)),
            ("isProperty", Value::Bool(property.is_some())),
            ("isSubscript", Value::Bool(property.is_none())),
        ] {
            let name: Arc<str> = Arc::from(name);
            object.members.push(Member {
                entry: false,
                name: name.clone(),
                code: usize::MAX,
                local: false,
                method: false,
                ty: None,
                type_span: None,
                constant: false,
                fixed: false,
            });
            object.complete_cache(
                CacheKey {
                    entry: false,
                    owner: None,
                    name,
                    method: false,
                },
                value,
            );
        }
        if self
            .heap
            .get(parent)
            .expect("access class")
            .members
            .is_empty()
        {
            self.heap.get_mut(parent).expect("access class").members = object.members.clone();
        }
        Value::Object(self.heap.allocate(object))
    }

    pub(super) fn reference_access(
        &mut self,
        id: ObjectId,
        property: Option<&str>,
        key: Option<Value>,
    ) -> Result<Value, RuntimeError> {
        let mut reference = self
            .heap
            .get(id)?
            .reference
            .clone()
            .ok_or_else(|| invalid("missing reference payload"))?;
        let types = if let TypeCheck::Union { types, .. } = &reference.referent {
            types.clone()
        } else {
            vec![reference.referent.clone()]
        };
        let mut candidates = Vec::new();
        for ty in types {
            let candidate = self
                .reference_candidate(&ty, property, key.as_ref())
                .map_err(|_| {
                    self.reference_access_error(id, &reference, &ty, property, key.as_ref())
                        .unwrap_or_else(|error| error)
                })?;
            candidates.push(candidate);
        }
        reference.referent = union(candidates, &self.program.module_name);
        reference.forced = false;
        reference
            .path
            .push(self.reference_access_value(property, key));
        Ok(Value::Object(self.heap.allocate(Object {
            reference: Some(reference),
            ..Object::default()
        })))
    }

    fn reference_access_error(
        &self,
        id: ObjectId,
        reference: &Reference,
        failed: &TypeCheck,
        property: Option<&str>,
        key: Option<&Value>,
    ) -> Result<RuntimeError, RuntimeError> {
        let module = &self.program.module_name;
        let actual = export_type_name(reference, &self.heap, module)?;
        let referent = type_name(&reference.referent, module);
        let failed_type = type_name(failed, module);
        let union = !reference.referent.equivalent(failed);
        let mut error = if let Some(property) = property {
            let mut error = RuntimeError::new(
                ErrorKind::MissingMember,
                format!("Cannot find property `{property}` in object of type `{actual}`."),
            );
            error.hint = Some(
                if property == "output" && matches!(failed, TypeCheck::Class { id: MODULE_ID, .. })
                {
                    "Cannot reference property `output` because it is defined inside external class `Module`.".into()
                } else if property == "default"
                    && matches!(
                        failed,
                        TypeCheck::Collection(CollectionKind::Dynamic)
                            | TypeCheck::ParameterizedCollection {
                                kind: CollectionKind::Listing | CollectionKind::Mapping,
                                ..
                            }
                    )
                {
                    let name = match failed {
                        TypeCheck::Collection(k)
                        | TypeCheck::ParameterizedCollection { kind: k, .. } => format!("{k:?}"),
                        _ => unreachable!(),
                    };
                    format!("Cannot reference property `default` in class `{name}`.")
                } else if union {
                    format!(
                        "Cannot find property `{property}` in type `{referent}`.\n\t└─ Cannot find property `{property}` in type `{failed_type}`."
                    )
                } else {
                    format!("Cannot find property `{property}` in type `{referent}`.")
                },
            );
            error
        } else {
            let key = key.expect("subscript");
            let key_type = crate::diagnostic_values::type_name(key, &self.heap, module)?;
            let left =
                crate::diagnostic_values::render(&Value::Object(id), &self.heap, module, 80)?;
            let right = crate::diagnostic_values::render(key, &self.heap, module, 80)?;
            let mut error = RuntimeError::new(
                ErrorKind::Type,
                format!(
                    "Operator `[]` is not defined for operand types `{actual}` and `{key_type}`.\nLeft operand : {left}\nRight operand: {right}"
                ),
            );
            error.hint = Some(if union {
                format!(
                    "Operator `[]` is not defined for operand types `{referent}` and `{key_type}`.\n\t└─ Operator `[]` is not defined for operand types `{failed_type}` and `{key_type}`."
                )
            } else {
                format!(
                    "Operator `[]` is not defined for operand types `{referent}` and `{key_type}`."
                )
            });
            error
        };
        error.span = None;
        Ok(error)
    }

    fn reference_candidate(
        &self,
        ty: &TypeCheck,
        property: Option<&str>,
        key: Option<&Value>,
    ) -> Result<TypeCheck, RuntimeError> {
        let fail = || {
            RuntimeError::new(
                ErrorKind::MissingMember,
                format!(
                    "Cannot reference {} in type `{}`.",
                    property.map_or_else(|| "subscript".into(), |p| format!("property `{p}`")),
                    type_name(ty, &self.program.module_name)
                ),
            )
        };
        match ty {
            TypeCheck::Unknown => Ok(TypeCheck::Unknown),
            TypeCheck::Null => Ok(TypeCheck::Null),
            TypeCheck::Collection(CollectionKind::Dynamic) => {
                if property == Some("default") {
                    Err(fail())
                } else {
                    Ok(TypeCheck::Unknown)
                }
            }
            TypeCheck::Class { id: MODULE_ID, .. } if property == Some("output") => Err(fail()),
            TypeCheck::Class { id, .. } if property.is_some() => {
                let name = property.expect("property access");
                let mut current = self.heap.find_class(*id);
                let mut found = false;
                let mut visited = BTreeSet::new();
                while let Some(id) = current {
                    if !visited.insert(id) {
                        return Err(RuntimeError::new(ErrorKind::Cycle, "cyclic parent chain"));
                    }
                    let object = self.heap.get(id)?;
                    if let Some(member) = object
                        .members
                        .iter()
                        .find(|m| !m.local && !m.method && m.name.as_ref() == name)
                    {
                        found = true;
                        if let Some(ty) = &member.ty {
                            return Ok(normalize(ty, &self.program.module_name));
                        }
                    }
                    current = object.parent;
                }
                if found {
                    Ok(TypeCheck::Unknown)
                } else {
                    Err(fail())
                }
            }
            TypeCheck::ParameterizedCollection { kind, arguments } if property.is_none() => {
                let key = key.expect("subscript key");
                match kind {
                    CollectionKind::List | CollectionKind::Listing
                        if matches!(key, Value::Int(_)) =>
                    {
                        Ok(normalize(&arguments[0].0, &self.program.module_name))
                    }
                    CollectionKind::Mapping
                        if reference_key_matches(&arguments[0].0, key, &self.heap) =>
                    {
                        Ok(normalize(&arguments[1].0, &self.program.module_name))
                    }
                    _ => Err(fail()),
                }
            }
            TypeCheck::String if property == Some("length") => Ok(TypeCheck::Int),
            TypeCheck::String if property == Some("isEmpty") => Ok(TypeCheck::Boolean),
            _ => Err(fail()),
        }
    }
}
fn reference_key_matches(ty: &TypeCheck, key: &Value, heap: &Heap) -> bool {
    match ty {
        TypeCheck::Unknown => true,
        TypeCheck::Union { types, .. } => types.iter().any(|t| reference_key_matches(t, key, heap)),
        TypeCheck::Class { id, .. } => {
            matches!(key,Value::Object(object) if collections::typed_class(heap,*object).ok().flatten().is_some_and(|c| c.id==*id))
        }
        TypeCheck::Any
        | TypeCheck::NonNull
        | TypeCheck::Number
        | TypeCheck::IntegerAlias { .. } => false,
        _ => ty.accepts(key, heap),
    }
}

pub(crate) fn export_type_name(
    reference: &Reference,
    heap: &Heap,
    module: &str,
) -> Result<String, RuntimeError> {
    let domain = crate::diagnostic_values::type_name(&reference.domain, heap, module)?;
    Ok(format!(
        "pkl.ref#Reference<{domain}, {}>",
        type_name(&reference.referent, module)
    ))
}

fn constructor_error(message: String, fragment: &str) -> RuntimeError {
    let source = crate::stdlib_sources::reference();
    let declaration = source
        .text
        .find("external const function Reference")
        .expect("reference declaration");
    let start = declaration
        + source.text[declaration..]
            .find(fragment)
            .expect("constructor type");
    let span = Span::new(start, start + fragment.len());
    let mut error = RuntimeError::new(ErrorKind::Type, message).at(span);
    error.frames.push(RuntimeFrame {
        span,
        member: Arc::from("#Reference"),
        source: Some(source),
    });
    error
}

/// Ordinary bytecode backs Access defaults, computed flags, and its property constraint.
pub(super) fn access_members(
    blocks: &mut Vec<Vec<Instruction>>,
    metadata: &mut Vec<BlockMetadata>,
) -> Vec<Member> {
    let source = crate::stdlib_sources::reference();
    let span = |fragment: &str| {
        let start = source.text.rfind(fragment).expect("pinned Access fragment");
        Span::new(start, start + fragment.len())
    };
    let constraint = blocks.len();
    blocks.push(vec![
        Instruction::GetProperty {
            name: Arc::from("key"),
            owner_depth: 0,
            local: false,
        },
        Instruction::Constant(Value::Null),
        Instruction::Equal,
        Instruction::Return,
    ]);
    let predicate_span = span("key == null");
    metadata.push(BlockMetadata {
        source: Some(source.clone()),
        spans: vec![
            Some(Span::new(predicate_span.start, predicate_span.start + 3)),
            Some(predicate_span),
            Some(predicate_span),
            None,
        ],
        member: Arc::from("#Access.property"),
        ..BlockMetadata::default()
    });
    let property_type = TypeCheck::Nullable(Box::new(TypeCheck::Constrained {
        base: Box::new(TypeCheck::String),
        base_span: span("String(key == null)?"),
        constraints: vec![ConstraintCode {
            block: constraint,
            span: predicate_span,
            module_environment: false,
            source: Arc::from("key == null"),
        }],
    }));
    let mut members = Vec::new();
    for (name, ty, expression, code) in [
        (
            "isProperty",
            TypeCheck::Boolean,
            "property != null",
            vec![
                Instruction::LoadThis,
                Instruction::GetMember(Arc::from("property")),
                Instruction::Constant(Value::Null),
                Instruction::NotEqual,
                Instruction::Return,
            ],
        ),
        (
            "isSubscript",
            TypeCheck::Boolean,
            "property == null",
            vec![
                Instruction::LoadThis,
                Instruction::GetMember(Arc::from("property")),
                Instruction::Constant(Value::Null),
                Instruction::Equal,
                Instruction::Return,
            ],
        ),
        (
            "property",
            property_type,
            "String(key == null)?",
            vec![Instruction::Constant(Value::Null), Instruction::Return],
        ),
        (
            "key",
            TypeCheck::Any,
            "key: Any",
            vec![
                Instruction::Undefined(Arc::from("key")),
                Instruction::Return,
            ],
        ),
    ] {
        let index = blocks.len();
        let declaration = span(expression);
        let mut spans = vec![Some(declaration); code.len()];
        if matches!(name, "isProperty" | "isSubscript") {
            spans[1] = Some(Span::new(declaration.start, declaration.start + 8));
        }
        if name == "key" {
            spans[0] = Some(Span::new(declaration.start, declaration.start + 3));
        }
        metadata.push(BlockMetadata {
            source: Some(source.clone()),
            spans,
            member: Arc::from(format!("#Access.{name}")),
            declaration: Some(declaration),
            ..BlockMetadata::default()
        });
        blocks.push(code);
        members.push(Member {
            entry: false,
            name: Arc::from(name),
            code: index,
            local: false,
            method: false,
            ty: Some(ty),
            type_span: Some(declaration),
            constant: false,
            fixed: matches!(name, "isProperty" | "isSubscript"),
        });
    }
    members
}
