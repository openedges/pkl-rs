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
        "Map" => TypeCheck::Collection(CollectionKind::Map),
        "Set" => TypeCheck::Collection(CollectionKind::Set),
        "Collection" => TypeCheck::CollectionBase {
            arguments: Vec::new(),
        },
        _ => return None,
    })
}

pub(crate) fn type_name<'a>(ty: &'a TypeCheck, module: &str) -> String {
    enum Part<'a> {
        Type(&'a TypeCheck),
        Text(String),
    }
    let mut output = String::new();
    let mut pending = vec![Part::Type(ty)];
    while let Some(part) = pending.pop() {
        let ty = match part {
            Part::Text(text) => {
                output.push_str(&text);
                continue;
            }
            Part::Type(ty) => ty.resolved(),
        };
        let mut sequence = Vec::new();
        let mut list = |types: Vec<&'a TypeCheck>, separator: &str| {
            for (index, ty) in types.into_iter().enumerate() {
                if index != 0 {
                    sequence.push(Part::Text(separator.to_owned()));
                }
                sequence.push(Part::Type(ty));
            }
        };
        match ty {
            TypeCheck::Unknown => output.push_str("unknown"),
            TypeCheck::NamedAlias {
                name, arguments, ..
            } => {
                output.push_str(&format!("{module}#{name}"));
                if !arguments.is_empty() {
                    output.push('<');
                    list(arguments.iter().collect(), ", ");
                    sequence.push(Part::Text(">".into()));
                }
            }
            TypeCheck::Function {
                parameters,
                result,
                syntax,
            } => {
                let mut args: Vec<_> = parameters.iter().flatten().collect();
                if *syntax {
                    output.push('(');
                    list(args, ", ");
                    sequence.push(Part::Text(") -> ".into()));
                    sequence.push(Part::Type(result));
                } else {
                    output.push_str(&super::type_name(ty, module));
                    output.push('<');
                    args.push(result);
                    list(args, ", ");
                    sequence.push(Part::Text(">".into()));
                }
            }
            TypeCheck::RefDomain => output.push_str("pkl.ref#Domain"),
            TypeCheck::Reference { domain, referent } => {
                output.push_str("pkl.ref#Reference<");
                list(vec![domain, referent], ", ");
                sequence.push(Part::Text(">".into()));
            }
            TypeCheck::Class { id: MODULE_ID, .. } => output.push_str(module),
            TypeCheck::Class { id: ACCESS_ID, .. } => output.push_str("pkl.ref#Access"),
            TypeCheck::Class { name, .. } => output.push_str(&format!("{module}#{name}")),
            TypeCheck::Union { types, .. } => list(types.iter().collect(), " | "),
            TypeCheck::Nullable(inner) => {
                sequence.push(Part::Type(inner));
                sequence.push(Part::Text("?".into()));
            }
            TypeCheck::Alias(inner)
            | TypeCheck::GenericAlias(inner)
            | TypeCheck::Constrained { base: inner, .. } => sequence.push(Part::Type(inner)),
            TypeCheck::ParameterizedCollection { kind, arguments } => {
                output.push_str(&format!("{kind:?}<"));
                list(arguments.iter().map(|(ty, _)| ty).collect(), ", ");
                sequence.push(Part::Text(">".into()));
            }
            TypeCheck::Collection(kind) => output.push_str(&format!("{kind:?}")),
            TypeCheck::CollectionBase { arguments } => {
                output.push_str("Collection");
                if !arguments.is_empty() {
                    output.push('<');
                    list(arguments.iter().map(|(ty, _)| ty).collect(), ", ");
                    sequence.push(Part::Text(">".into()));
                }
            }
            TypeCheck::StringLiteral(value) => output.push_str(&format!("\"{value}\"")),
            _ => output.push_str(&super::type_name(ty, module)),
        }
        pending.extend(sequence.into_iter().rev());
    }
    output
}

/// Fold an immutable metadata DAG in postorder without using the native stack.
/// Pointer keys are local traversal identities, never exported type identities.
fn fold_metadata<T: Clone>(
    root: &TypeCheck,
    children: impl Fn(&TypeCheck) -> Vec<&TypeCheck>,
    mut fold: impl FnMut(&TypeCheck, Vec<T>) -> T,
) -> T {
    let mut values: BTreeMap<usize, T> = BTreeMap::new();
    let mut pending = vec![(root, false)];
    while let Some((ty, ready)) = pending.pop() {
        let ty = ty.resolved();
        let key = ty as *const TypeCheck as usize;
        if values.contains_key(&key) {
            continue;
        }
        let children = children(ty);
        if !ready {
            pending.push((ty, true));
            pending.extend(children.into_iter().rev().map(|child| (child, false)));
        } else {
            let args = children
                .into_iter()
                .map(|child| values[&(child.resolved() as *const TypeCheck as usize)].clone())
                .collect();
            values.insert(key, fold(ty, args));
        }
    }
    values
        .remove(&(root.resolved() as *const TypeCheck as usize))
        .expect("metadata root")
}

/// Java exports nullable types as flattened unions and erases constraints.
pub(crate) fn normalize(ty: &TypeCheck, module: &str) -> TypeCheck {
    fold_metadata(
        ty,
        |ty| match ty {
            TypeCheck::NamedAlias {
                expanded: inner, ..
            }
            | TypeCheck::Alias(inner)
            | TypeCheck::GenericAlias(inner)
            | TypeCheck::Constrained { base: inner, .. }
            | TypeCheck::Nullable(inner) => vec![inner],
            TypeCheck::Union { types, .. } => types.iter().collect(),
            TypeCheck::ParameterizedCollection { arguments, .. }
            | TypeCheck::CollectionBase { arguments } => {
                arguments.iter().map(|(ty, _)| ty).collect()
            }
            TypeCheck::Function {
                parameters,
                result,
                syntax: false,
            } => parameters
                .iter()
                .flatten()
                .chain(std::iter::once(result.as_ref()))
                .collect(),
            TypeCheck::Reference { domain, referent } => vec![domain, referent],
            _ => Vec::new(),
        },
        |ty, mut args| {
            let normalized = match ty {
                TypeCheck::NonNull => TypeCheck::Any,
                TypeCheck::NamedAlias { .. }
                | TypeCheck::Alias(_)
                | TypeCheck::GenericAlias(_)
                | TypeCheck::Constrained { .. } => args.pop().expect("expanded type"),
                TypeCheck::Nullable(_) => {
                    args.push(TypeCheck::Null);
                    union(args, module)
                }
                TypeCheck::Union { .. } => union(args, module),
                TypeCheck::Collection(
                    kind @ (CollectionKind::List
                    | CollectionKind::Listing
                    | CollectionKind::Mapping
                    | CollectionKind::Map
                    | CollectionKind::Set),
                ) => TypeCheck::ParameterizedCollection {
                    kind: *kind,
                    arguments: vec![
                        (TypeCheck::Unknown, Span::new(0, 0));
                        if matches!(kind, CollectionKind::Mapping | CollectionKind::Map) {
                            2
                        } else {
                            1
                        }
                    ],
                },
                TypeCheck::CollectionBase { .. } => TypeCheck::CollectionBase {
                    arguments: if args.is_empty() {
                        vec![(TypeCheck::Unknown, Span::new(0, 0))]
                    } else {
                        args.into_iter().map(|ty| (ty, Span::new(0, 0))).collect()
                    },
                },
                TypeCheck::ParameterizedCollection { kind, .. } => {
                    TypeCheck::ParameterizedCollection {
                        kind: *kind,
                        arguments: args.into_iter().map(|ty| (ty, Span::new(0, 0))).collect(),
                    }
                }
                TypeCheck::Function {
                    parameters,
                    syntax: false,
                    ..
                } => {
                    let result = Box::new(args.pop().expect("function result"));
                    TypeCheck::Function {
                        parameters: parameters.as_ref().map(|_| args),
                        result,
                        syntax: false,
                    }
                }
                TypeCheck::Reference { .. } => {
                    let referent = Box::new(args.pop().expect("referent"));
                    let domain = Box::new(args.pop().expect("domain"));
                    TypeCheck::Reference { domain, referent }
                }
                ty => ty.clone(),
            };
            normalized.shared()
        },
    )
}
fn union(types: Vec<TypeCheck>, module: &str) -> TypeCheck {
    let mut result = Vec::new();
    for ty in types {
        if let TypeCheck::Union { types, .. } = ty.resolved() {
            result.extend(types.iter().cloned());
        } else {
            result.push(ty);
        }
    }
    if result
        .iter()
        .any(|ty| matches!(ty.resolved(), TypeCheck::Unknown))
    {
        return TypeCheck::Unknown;
    }
    if result
        .iter()
        .any(|ty| matches!(ty.resolved(), TypeCheck::Any))
    {
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
    .shared()
}

/// Equality/hash identity of exported PType metadata, separate from checking
/// equivalence. Arrow types retain nested aliases and exact constraint source.
/// Interned postorder records avoid recursively escaping nested keys or expanding
/// shared argument DAGs. Local pointer identities do not enter this encoding.
pub(crate) fn metadata_key(ty: &TypeCheck) -> String {
    let mut interned = BTreeMap::new();
    let mut records = Vec::new();
    let root = fold_metadata(
        ty,
        |ty| match ty {
            TypeCheck::NamedAlias { arguments, .. } => arguments.iter().collect(),
            TypeCheck::Alias(inner)
            | TypeCheck::GenericAlias(inner)
            | TypeCheck::Nullable(inner)
            | TypeCheck::Constrained { base: inner, .. } => vec![inner],
            TypeCheck::Function {
                parameters, result, ..
            } => parameters
                .iter()
                .flatten()
                .chain(std::iter::once(result.as_ref()))
                .collect(),
            TypeCheck::Union { types, .. } => types.iter().collect(),
            TypeCheck::ParameterizedCollection { arguments, .. }
            | TypeCheck::CollectionBase { arguments } => {
                arguments.iter().map(|(ty, _)| ty).collect()
            }
            TypeCheck::Reference { domain, referent } => vec![domain, referent],
            _ => Vec::new(),
        },
        |ty, args: Vec<usize>| {
            let record = match ty {
                TypeCheck::Alias(_) | TypeCheck::GenericAlias(_) => return args[0],
                TypeCheck::NamedAlias { declaration, .. } => {
                    format!("alias:{declaration}:{args:?}")
                }
                TypeCheck::Function {
                    parameters, syntax, ..
                } => format!("function:{syntax}:{}:{args:?}", parameters.is_some()),
                TypeCheck::Constrained { constraints, .. } => format!(
                    "constraint:{}:{:?}",
                    args[0],
                    constraints
                        .iter()
                        .map(|c| c.source.as_ref())
                        .collect::<Vec<_>>()
                ),
                TypeCheck::Nullable(_) => format!("nullable:{}", args[0]),
                TypeCheck::Union { .. } => format!("union:{args:?}"),
                TypeCheck::ParameterizedCollection { kind, .. } => {
                    format!("collection:{kind:?}:{args:?}")
                }
                TypeCheck::CollectionBase { .. } => format!("collection:Collection:{args:?}"),
                TypeCheck::Reference { .. } => format!("reference:{args:?}"),
                other => format!("leaf:{other:?}"),
            };
            if let Some(index) = interned.get(&record) {
                return *index;
            }
            let index = records.len();
            records.push(record.clone());
            interned.insert(record, index);
            index
        },
    );
    format!("{root}:{records:?}")
}

pub(crate) fn is_subtype(a: &TypeCheck, b: &TypeCheck, heap: &Heap) -> bool {
    if matches!(a.resolved(), TypeCheck::Unknown)
        || matches!(b.resolved(), TypeCheck::Unknown | TypeCheck::Any)
    {
        return true;
    }
    if matches!(b.resolved(), TypeCheck::Nothing) {
        return false;
    }
    subtype(a, b, heap)
}
fn function_syntax_flags(root: &TypeCheck) -> BTreeMap<usize, bool> {
    let mut flags = BTreeMap::new();
    fold_metadata(
        root,
        |ty| match ty {
            TypeCheck::Function {
                parameters,
                result,
                syntax: false,
            } => parameters
                .iter()
                .flatten()
                .chain(std::iter::once(result.as_ref()))
                .collect(),
            TypeCheck::ParameterizedCollection { arguments, .. }
            | TypeCheck::CollectionBase { arguments } => {
                arguments.iter().map(|(ty, _)| ty).collect()
            }
            TypeCheck::Reference { domain, referent } => vec![domain, referent],
            TypeCheck::Union { types, .. } => types.iter().collect(),
            _ => Vec::new(),
        },
        |ty, args: Vec<bool>| {
            let found = matches!(ty, TypeCheck::Function { syntax: true, .. })
                || args.into_iter().any(|x| x);
            flags.insert(ty as *const TypeCheck as usize, found);
            found
        },
    );
    flags
}

fn class_subtype(id: usize, other: usize, heap: &Heap) -> bool {
    if id == other {
        return true;
    }
    let mut current = heap.find_class(id);
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
            .is_some_and(|c| c.id == other)
        {
            return true;
        }
        current = object.parent;
    }
    false
}

fn subtype(a: &TypeCheck, b: &TypeCheck, heap: &Heap) -> bool {
    type Pair<'a> = (&'a TypeCheck, &'a TypeCheck);
    enum Task<'a> {
        Compare(Pair<'a>),
        Save((usize, usize)),
        Group {
            pairs: Vec<Pair<'a>>,
            next: usize,
            all: bool,
        },
        ResumeGroup {
            pairs: Vec<Pair<'a>>,
            next: usize,
            all: bool,
        },
        Union {
            left: &'a [TypeCheck],
            right: &'a [TypeCheck],
            next: usize,
        },
        ResumeUnion {
            left: &'a [TypeCheck],
            right: &'a [TypeCheck],
            next: usize,
        },
    }
    let mut flags = function_syntax_flags(a);
    flags.extend(function_syntax_flags(b));
    let mut memo = BTreeMap::new();
    let mut tasks = vec![Task::Compare((a, b))];
    let mut value = false;
    while let Some(task) = tasks.pop() {
        match task {
            Task::Save(key) => {
                memo.insert(key, value);
            }
            Task::Group { pairs, next, all } => {
                if next == pairs.len() {
                    value = all;
                    continue;
                }
                let pair = pairs[next];
                tasks.push(Task::ResumeGroup {
                    pairs,
                    next: next + 1,
                    all,
                });
                tasks.push(Task::Compare(pair));
            }
            Task::ResumeGroup { pairs, next, all } => {
                if value == all {
                    tasks.push(Task::Group { pairs, next, all });
                }
            }
            Task::Union { left, right, next } => {
                if next == left.len() {
                    value = true;
                    continue;
                }
                tasks.push(Task::ResumeUnion {
                    left,
                    right,
                    next: next + 1,
                });
                tasks.push(Task::Group {
                    pairs: right.iter().map(|b| (&left[next], b)).collect(),
                    next: 0,
                    all: false,
                });
            }
            Task::ResumeUnion { left, right, next } => {
                if value {
                    tasks.push(Task::Union { left, right, next });
                }
            }
            Task::Compare((a, b)) => {
                let (a, b) = (a.resolved(), b.resolved());
                let key = (
                    a as *const TypeCheck as usize,
                    b as *const TypeCheck as usize,
                );
                if let Some(cached) = memo.get(&key) {
                    value = *cached;
                    continue;
                }
                tasks.push(Task::Save(key));
                if matches!(a, TypeCheck::Function { syntax: true, .. })
                    || matches!(b, TypeCheck::Function { syntax: true, .. })
                {
                    value = false;
                    continue;
                }
                // These pairs have explicit recursive rules below. Rechecking
                // their complete structural equality at every depth would make
                // a covariant differing-leaf comparison quadratic.
                let structural = matches!(
                    (a, b),
                    (TypeCheck::Function { .. }, TypeCheck::Function { .. })
                        | (TypeCheck::Reference { .. }, TypeCheck::Reference { .. })
                        | (
                            TypeCheck::ParameterizedCollection { .. },
                            TypeCheck::ParameterizedCollection { .. }
                        )
                        | (
                            TypeCheck::CollectionBase { .. },
                            TypeCheck::CollectionBase { .. }
                        )
                        | (
                            TypeCheck::ParameterizedCollection { .. },
                            TypeCheck::CollectionBase { .. }
                        )
                        | (TypeCheck::Union { .. }, TypeCheck::Union { .. })
                );
                if !structural
                    && !flags.get(&key.0).copied().unwrap_or(false)
                    && !flags.get(&key.1).copied().unwrap_or(false)
                    && a.equivalent(b)
                {
                    value = true;
                    continue;
                }
                value = match (a, b) {
                    (
                        TypeCheck::IntegerAlias { name: a, .. },
                        TypeCheck::IntegerAlias { name: b, .. },
                    ) => match a.as_ref() {
                        "Int8" => matches!(b.as_ref(), "Int16" | "Int32"),
                        "Int16" => b.as_ref() == "Int32",
                        "UInt8" => {
                            matches!(b.as_ref(), "Int16" | "Int32" | "UInt16" | "UInt32" | "UInt")
                        }
                        "UInt16" => matches!(b.as_ref(), "Int32" | "UInt32" | "UInt"),
                        "UInt32" => b.as_ref() == "UInt",
                        _ => false,
                    },
                    (
                        TypeCheck::IntegerAlias { .. },
                        TypeCheck::Int | TypeCheck::Float | TypeCheck::Number,
                    ) => true,
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
                        | TypeCheck::CollectionBase { .. }
                        | TypeCheck::Reference { .. },
                        TypeCheck::Any,
                    ) => true,
                    (TypeCheck::Class { id, .. }, TypeCheck::RefDomain) => {
                        class_subtype(*id, DOMAIN_ID, heap)
                    }
                    (
                        TypeCheck::Reference {
                            domain: ad,
                            referent: at,
                        },
                        TypeCheck::Reference {
                            domain: bd,
                            referent: bt,
                        },
                    ) => {
                        tasks.push(Task::Group {
                            pairs: vec![(ad, bd), (at, bt)],
                            next: 0,
                            all: true,
                        });
                        continue;
                    }
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
                        let parameters = match (a, b) {
                            (None, None) => Some(Vec::new()),
                            (Some(a), None) if a.is_empty() => Some(Vec::new()),
                            (Some(a), Some(b)) if a.len() == b.len() => {
                                Some(a.iter().zip(b).map(|(a, b)| (b, a)).collect::<Vec<_>>())
                            }
                            _ => None,
                        };
                        if let Some(parameters) = parameters {
                            let mut pairs = vec![(ar.as_ref(), br.as_ref())];
                            pairs.extend(parameters);
                            tasks.push(Task::Group {
                                pairs,
                                next: 0,
                                all: true,
                            });
                            continue;
                        }
                        false
                    }
                    (TypeCheck::StringLiteral(_), TypeCheck::String)
                    | (TypeCheck::Int | TypeCheck::Float, TypeCheck::Number) => true,
                    (TypeCheck::Class { id, .. }, TypeCheck::Class { id: other, .. }) => {
                        class_subtype(*id, *other, heap)
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
                        if a == b && aa.len() == ba.len() {
                            tasks.push(Task::Group {
                                pairs: aa.iter().zip(ba).map(|((a, _), (b, _))| (a, b)).collect(),
                                next: 0,
                                all: true,
                            });
                            continue;
                        }
                        false
                    }
                    (
                        TypeCheck::CollectionBase { arguments: aa },
                        TypeCheck::CollectionBase { arguments: ba },
                    )
                    | (
                        TypeCheck::ParameterizedCollection {
                            kind: CollectionKind::List | CollectionKind::Set,
                            arguments: aa,
                        },
                        TypeCheck::CollectionBase { arguments: ba },
                    ) => {
                        if aa.len() == ba.len() {
                            tasks.push(Task::Group {
                                pairs: aa.iter().zip(ba).map(|((a, _), (b, _))| (a, b)).collect(),
                                next: 0,
                                all: true,
                            });
                            continue;
                        }
                        false
                    }
                    (
                        TypeCheck::Union { types: left, .. },
                        TypeCheck::Union { types: right, .. },
                    ) => {
                        tasks.push(Task::Union {
                            left,
                            right,
                            next: 0,
                        });
                        continue;
                    }
                    (a, TypeCheck::Union { types, .. }) => {
                        tasks.push(Task::Group {
                            pairs: types.iter().map(|b| (a, b)).collect(),
                            next: 0,
                            all: false,
                        });
                        continue;
                    }
                    _ => false,
                };
            }
        }
    }
    value
}

impl Vm {
    pub(super) fn builtin_class(&mut self, ty: TypeCheck) -> Value {
        let ty = ty.into_resolved();
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
                        TypeCheck::Collection(CollectionKind::Map) => 25,
                        TypeCheck::Collection(CollectionKind::Set) => 26,
                        TypeCheck::CollectionBase { .. } => 27,
                        _ => 2,
                    },
                super::type_name(&ty, &self.program.module_name),
                matches!(ty, TypeCheck::CollectionBase { .. }),
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
            let text = if self.is_set(&native.receiver) {
                crate::diagnostic_values::render_multiline(
                    &native.receiver,
                    &self.heap,
                    &self.program.module_name,
                )?
            } else {
                match &native.receiver {
                    Value::TypeAlias(_)
                    | Value::Class(_)
                    | Value::Function(_)
                    | Value::Object(_) => crate::diagnostic_values::render(
                        &native.receiver,
                        &self.heap,
                        &self.program.module_name,
                        usize::MAX,
                    )?,
                    value => scalar_string(value)?,
                }
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
            Value::Null | Value::NullWithDefault(_) => TypeCheck::Null,
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
                            .map(|f| match f.name.as_ref() {
                                "Reference" => 3,
                                "containsKey" | "getOrNull" | "contains" => 1,
                                _ => 0,
                            })
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
                captured_this: None,
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
        let types = if let TypeCheck::Union { types, .. } = reference.referent.resolved() {
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
        let failed = failed.resolved();
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
        let ty = ty.resolved();
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
                    CollectionKind::Mapping | CollectionKind::Map
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
    let mut pending = vec![ty];
    while let Some(ty) = pending.pop() {
        let ty = ty.resolved();
        let matches = match ty {
            TypeCheck::Unknown => true,
            TypeCheck::Union { types, .. } => {
                pending.extend(types.iter().rev());
                false
            }
            TypeCheck::Class { id, .. } => {
                matches!(key,Value::Object(object) if collections::typed_class(heap,*object).ok().flatten().is_some_and(|c| c.id==*id))
            }
            TypeCheck::Any
            | TypeCheck::NonNull
            | TypeCheck::Number
            | TypeCheck::IntegerAlias { .. } => false,
            _ => ty.accepts(key, heap),
        };
        if matches {
            return true;
        }
    }
    false
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
            arguments: BTreeMap::new(),
            argument_sources: BTreeMap::new(),
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

#[cfg(test)]
mod generic_metadata_tests {
    use super::*;

    fn collection(kind: CollectionKind, arguments: Vec<TypeCheck>) -> TypeCheck {
        TypeCheck::ParameterizedCollection {
            kind,
            arguments: arguments
                .into_iter()
                .map(|ty| (ty, Span::new(0, 0)))
                .collect(),
        }
        .shared()
    }

    fn nested_list(mut ty: TypeCheck, depth: usize) -> TypeCheck {
        for _ in 0..depth {
            ty = collection(CollectionKind::List, vec![ty]);
        }
        ty
    }

    #[test]
    fn exported_metadata_handles_deep_arguments_on_small_stack() {
        std::thread::Builder::new()
            .stack_size(256 * 1024)
            .spawn(|| {
                let depth = 2048;
                let integer = nested_list(TypeCheck::Int, depth);
                let same = nested_list(TypeCheck::Int, depth);
                let number = nested_list(TypeCheck::Number, depth);
                let normalized = normalize(&integer, "test");
                assert_eq!(metadata_key(&integer), metadata_key(&normalized));
                assert_eq!(metadata_key(&integer), metadata_key(&same));
                assert_ne!(metadata_key(&integer), metadata_key(&number));
                assert_eq!(
                    type_name(&normalized, "test"),
                    format!("{}Int{}", "List<".repeat(depth), ">".repeat(depth))
                );
                let heap = Heap::default();
                assert!(is_subtype(&integer, &number, &heap));
                assert!(!is_subtype(&number, &integer, &heap));
            })
            .expect("small-stack thread")
            .join()
            .expect("deep metadata remains stack safe");
    }

    #[test]
    fn exported_metadata_preserves_sharing_without_changing_identity() {
        let mut branching = TypeCheck::Int.shared();
        for _ in 0..256 {
            branching = collection(CollectionKind::Mapping, vec![branching.clone(), branching]);
        }
        let key = metadata_key(&branching);
        assert!(
            key.len() < 32 * 1024,
            "metadata encoding must not expand the shared tree"
        );
        assert_eq!(key, metadata_key(&normalize(&branching, "test")));
        let shared = collection(CollectionKind::List, vec![TypeCheck::Int]);
        let shared_twice = collection(CollectionKind::Mapping, vec![shared.clone(), shared]);
        let independent = collection(
            CollectionKind::Mapping,
            vec![
                collection(CollectionKind::List, vec![TypeCheck::Int]),
                collection(CollectionKind::List, vec![TypeCheck::Int]),
            ],
        );
        assert_eq!(metadata_key(&shared_twice), metadata_key(&independent));
        let changed = collection(
            CollectionKind::Mapping,
            vec![
                collection(CollectionKind::List, vec![TypeCheck::Int]),
                collection(CollectionKind::List, vec![TypeCheck::Float]),
            ],
        );
        assert_ne!(metadata_key(&shared_twice), metadata_key(&changed));
    }

    #[test]
    fn shared_exported_metadata_keeps_arrow_alias_and_constraint_identity() {
        assert!(is_subtype(
            &TypeCheck::Class {
                id: DOMAIN_ID,
                name: Arc::from("Domain")
            }
            .shared(),
            &TypeCheck::RefDomain,
            &Heap::default(),
        ));
        let alias = |declaration| {
            TypeCheck::NamedAlias {
                declaration,
                name: Arc::from("Alias"),
                arguments: vec![TypeCheck::String],
                expanded: Box::new(TypeCheck::Int),
            }
            .shared()
        };
        let arrow = |ty| {
            TypeCheck::Function {
                parameters: Some(vec![ty]),
                result: Box::new(TypeCheck::Boolean),
                syntax: true,
            }
            .shared()
        };
        let a = arrow(alias(1));
        let b = arrow(alias(2));
        assert_ne!(
            metadata_key(&normalize(&a, "test")),
            metadata_key(&normalize(&b, "test"))
        );
        assert_eq!(type_name(&a, "test"), "(test#Alias<String>) -> Boolean");
        assert!(!is_subtype(&a, &a, &Heap::default()));
        let constrained = |source: &str| {
            TypeCheck::Constrained {
                base: Box::new(TypeCheck::Int),
                base_span: Span::new(0, 0),
                constraints: vec![ConstraintCode {
                    block: 0,
                    arguments: BTreeMap::new(),
                    argument_sources: BTreeMap::new(),
                    source: Arc::from(source),
                    span: Span::new(0, 0),
                    module_environment: false,
                }],
            }
            .shared()
        };
        let a = arrow(constrained("this > 0"));
        let b = arrow(constrained("this>0"));
        assert_ne!(metadata_key(&a), metadata_key(&b));
        assert_eq!(
            metadata_key(&normalize(&constrained("this > 0"), "test")),
            metadata_key(&TypeCheck::Int)
        );
    }
}
