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
enum Candidate {
    Type(TypeCheck),
    Unavailable,
    Pending,
}

#[derive(Clone, Debug)]
pub struct NativeFunction {
    pub receiver: Value,
    pub name: Arc<str>,
}

pub(crate) fn base_type(name: &str) -> Option<TypeCheck> {
    Some(match name {
        "Class" | "TypeAlias" => TypeCheck::MetaClass(name.into()),
        "Function" | "Function0" | "Function1" | "Function2" | "Function3" | "Function4"
        | "Function5" => TypeCheck::Function {
            parameters: name
                .strip_prefix("Function")
                .and_then(|n| n.parse::<usize>().ok())
                .map(|n| vec![TypeCheck::Unknown; n]),
            result: Box::new(TypeCheck::Unknown),
            syntax: false,
        },
        "Pair" => TypeCheck::Pair {
            arguments: Vec::new(),
        },
        "Any" => TypeCheck::Any,
        "Null" => TypeCheck::Null,
        "Boolean" => TypeCheck::Boolean,
        "Int" => TypeCheck::Int,
        "Float" => TypeCheck::Float,
        "Duration" => TypeCheck::Duration,
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
        _ => return super::classes::named_type(name),
    })
}

pub(crate) fn type_name(ty: &TypeCheck, module: &PklString) -> PklString {
    type_name_value(ty, module)
}

pub(crate) fn type_name_value<'a>(ty: &'a TypeCheck, module: &PklString) -> PklString {
    enum Part<'a> {
        Type(&'a TypeCheck),
        Text(String),
    }
    let mut output = crate::pkl_string::PklStringBuilder::default();
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
                name,
                arguments,
                default_origin,
                ..
            } => {
                let module = default_origin
                    .as_ref()
                    .and_then(|frame| frame.source.as_ref())
                    .map_or(module, |source| &source.module_name);
                output.push_value(module);
                output.push('#');
                output.push_value(name);
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
                    output.push_value(&super::type_name_value(ty, module));
                    output.push('<');
                    args.push(result);
                    list(args, ", ");
                    sequence.push(Part::Text(">".into()));
                }
            }
            TypeCheck::VarArgs { element, .. } => {
                output.push_str("VarArgs<");
                list(vec![element], ", ");
                sequence.push(Part::Text(">".into()));
            }
            TypeCheck::RefDomain => output.push_str("pkl.ref#Domain"),
            TypeCheck::Reference { domain, referent } => {
                output.push_str("pkl.ref#Reference<");
                list(vec![domain, referent], ", ");
                sequence.push(Part::Text(">".into()));
            }
            TypeCheck::Class { id: MODULE_ID, .. } => {
                output.push_value(&crate::heap::display_class_name(module, "ModuleClass"));
            }
            TypeCheck::Class { id: ACCESS_ID, .. } => output.push_str("pkl.ref#Access"),
            TypeCheck::Class {
                id,
                name,
                module: declaring_module,
            } => {
                if super::builtins::name(*id).is_some()
                    || super::classes::base_class_name(*id).is_some()
                {
                    output.push_value(name);
                } else {
                    output.push_value(&crate::heap::display_class_name(
                        declaring_module.as_ref().unwrap_or(module),
                        name,
                    ));
                }
            }
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
            TypeCheck::Pair { arguments } => {
                output.push_str("Pair");
                if !arguments.is_empty() {
                    output.push('<');
                    list(arguments.iter().map(|(t, _)| t).collect(), ", ");
                    sequence.push(Part::Text(">".into()));
                }
            }
            TypeCheck::CollectionBase { arguments } => {
                output.push_str("Collection");
                if !arguments.is_empty() {
                    output.push('<');
                    list(arguments.iter().map(|(ty, _)| ty).collect(), ", ");
                    sequence.push(Part::Text(">".into()));
                }
            }
            TypeCheck::StringLiteral(value) => {
                output.push('"');
                output.push_value(value);
                output.push('"');
            }
            TypeCheck::Parameter { name, .. } => output.push_value(name),
            _ => output.push_value(&super::type_name_value(ty, module)),
        }
        pending.extend(sequence.into_iter().rev());
    }
    output.finish()
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
pub(crate) fn normalize(ty: &TypeCheck, module: &PklString) -> TypeCheck {
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
            | TypeCheck::Pair { arguments }
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
            TypeCheck::VarArgs { element, .. } => vec![element],
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
                TypeCheck::Pair { .. } => TypeCheck::Pair {
                    arguments: if args.is_empty() {
                        vec![(TypeCheck::Unknown, Span::new(0, 0)); 2]
                    } else {
                        args.into_iter().map(|ty| (ty, Span::new(0, 0))).collect()
                    },
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
                TypeCheck::VarArgs { .. } => TypeCheck::VarArgs {
                    element: Box::new(args.pop().expect("VarArgs element")),
                    span: Span::new(0, 0),
                },
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
fn union(types: Vec<TypeCheck>, module: &PklString) -> TypeCheck {
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
        type_name_value(a, module)
            .encode_utf16()
            .cmp(type_name_value(b, module).encode_utf16())
    });
    result.dedup_by(|a, b| metadata_key(a) == metadata_key(b));
    if result.len() == 1 {
        return result.remove(0);
    }
    TypeCheck::Union {
        string_literals: false,
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
            | TypeCheck::Pair { arguments }
            | TypeCheck::CollectionBase { arguments } => {
                arguments.iter().map(|(ty, _)| ty).collect()
            }
            TypeCheck::VarArgs { element, .. } => vec![element],
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
                        .map(|c| c.source.original().encode_utf16().collect::<Vec<_>>())
                        .collect::<Vec<_>>()
                ),
                TypeCheck::Nullable(_) => format!("nullable:{}", args[0]),
                TypeCheck::Union { .. } => format!("union:{args:?}"),
                TypeCheck::ParameterizedCollection { kind, .. } => {
                    format!("collection:{kind:?}:{args:?}")
                }
                TypeCheck::Pair { .. } => format!("pair:{args:?}"),
                TypeCheck::CollectionBase { .. } => format!("collection:Collection:{args:?}"),
                TypeCheck::Reference { .. } => format!("reference:{args:?}"),
                TypeCheck::VarArgs { .. } => format!("varargs:{args:?}"),
                TypeCheck::StringLiteral(value) => {
                    format!("string:{:?}", value.encode_utf16().collect::<Vec<_>>())
                }
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
            | TypeCheck::Pair { arguments }
            | TypeCheck::CollectionBase { arguments } => {
                arguments.iter().map(|(ty, _)| ty).collect()
            }
            TypeCheck::VarArgs { element, .. } => vec![element],
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
    match (heap.find_class(id), heap.find_class(other)) {
        (Some(class), Some(expected)) => heap.is_subclass(class, expected),
        _ => false,
    }
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
                        | (TypeCheck::Pair { .. }, TypeCheck::Pair { .. })
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
                    ) => match a.as_str() {
                        Some("Int8") => matches!(b.as_str(), Some("Int16" | "Int32")),
                        Some("Int16") => b.as_str() == Some("Int32"),
                        Some("UInt8") => {
                            matches!(
                                b.as_str(),
                                Some("Int16" | "Int32" | "UInt16" | "UInt32" | "UInt")
                            )
                        }
                        Some("UInt16") => matches!(b.as_str(), Some("Int32" | "UInt32" | "UInt")),
                        Some("UInt32") => b.as_str() == Some("UInt"),
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
                        | TypeCheck::Duration
                        | TypeCheck::Number
                        | TypeCheck::String
                        | TypeCheck::Bytes
                        | TypeCheck::Class { .. }
                        | TypeCheck::RefDomain
                        | TypeCheck::Collection(_)
                        | TypeCheck::ParameterizedCollection { .. }
                        | TypeCheck::CollectionBase { .. }
                        | TypeCheck::Reference { .. }
                        | TypeCheck::VarArgs { .. }
                        | TypeCheck::Pair { .. },
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
                    (TypeCheck::Pair { arguments: a }, TypeCheck::Pair { arguments: b }) => {
                        if b.is_empty() {
                            true
                        } else if a.len() == b.len() {
                            tasks.push(Task::Group {
                                pairs: a.iter().zip(b).map(|((a, _), (b, _))| (a, b)).collect(),
                                next: 0,
                                all: true,
                            });
                            continue;
                        } else {
                            false
                        }
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
    pub(super) fn native_reference_method(&mut self, receiver: Value, name: &str) -> Value {
        let object = self.heap.allocate(Object {
            native_function: Some(NativeFunction {
                receiver,
                name: name.into(),
            }),
            ..Object::default()
        });
        Value::Function(object)
    }

    pub(super) fn reference_member(
        &mut self,
        id: ObjectId,
        name: &PklString,
        method: bool,
    ) -> Result<bool, RuntimeError> {
        if method && name == "ifNonNull" {
            // Any methods use the same inherited lookup as ordinary objects.
            return Ok(false);
        }
        if self.heap.get(id)?.ref_module {
            let value = match (name.as_str(), method) {
                (Some("Reference"), true) => {
                    self.native_reference_method(Value::Object(id), "Reference")
                }
                (Some("Reference"), false) => self.builtin_class(TypeCheck::Reference {
                    domain: Box::new(TypeCheck::Any),
                    referent: Box::new(TypeCheck::Any),
                }),
                (Some("Domain"), false) => self.builtin_class(TypeCheck::RefDomain),
                (Some("Access"), false) => self.builtin_class(TypeCheck::Class {
                    module: None,
                    id: ACCESS_ID,
                    name: "Access".into(),
                }),
                // Ordinary Module/Typed/Any members and missing-member
                // diagnostics fall through to standard object dispatch.
                _ => return Ok(false),
            };
            self.stack.push(value);
            return Ok(true);
        }
        if self.heap.get(id)?.reference.is_none() {
            return Ok(false);
        }
        let value = if method {
            match name.as_str() {
                Some("getDomain" | "getData" | "getPath" | "toString") => self
                    .native_reference_method(
                        Value::Object(id),
                        name.as_str().expect("recognized Reference method"),
                    ),
                _ => {
                    return Err(self.builtin_member_error("pkl.ref#Reference", name, true));
                }
            }
        } else {
            // Keep the original operand below any declaration-resolution
            // continuation. A suspended lookup retries this instruction;
            // it must not rebuild or force the referent value.
            self.stack.push(Value::Object(id));
            let Some(value) = self.reference_access(id, Some(name), None)? else {
                return Ok(true);
            };
            self.pop()?;
            value
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
            let mut error = RuntimeError::new(
                ErrorKind::Type,
                format!(
                    "Expected {expected} function arguments but got {}.",
                    args.len()
                ),
            );
            // Native inherited Any targets retain their declaration just like
            // ordinary FunctionNode calls. The resolved target, not the receiver
            // class or method spelling, determines this frame.
            if let Some(declaration) = any::native_declaration(&native.name) {
                error.frames.push(declaration);
            }
            return Err(error);
        }
        if native.name.as_ref() == "getClass" {
            let value = self.reference_get_class(&native.receiver)?;
            self.stack.push(value);
            return Ok(());
        }
        if native.name.as_ref() == "objectToString" {
            return self.begin_object_string(native.receiver.clone());
        }
        if native.name.as_ref() == "scalarToString" {
            let text = if self.is_set(&native.receiver) || self.is_list(&native.receiver) {
                crate::diagnostic_values::render_multiline(
                    &native.receiver,
                    &self.heap,
                    &self.program.module_name,
                )
            } else {
                match &native.receiver {
                    Value::TypeAlias(_)
                    | Value::Class(_)
                    | Value::Function(_)
                    | Value::Object(_) => crate::diagnostic_values::render_string(
                        &native.receiver,
                        &self.heap,
                        &self.program.module_name,
                        usize::MAX,
                    ),
                    value => scalar_string(value),
                }
            };
            let text = match text {
                Ok(text) => text,
                Err(mut error) => {
                    if error.foreign_exception.is_some()
                        && let Some(declaration) = any::native_declaration(&native.name)
                    {
                        error.frames.push(declaration);
                    }
                    return Err(error);
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
                    PklString::from("Type constraint `this is Domain` violated.\nValue: ")
                        .concat(&value),
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
                    PklString::from(format!(
                        "Expected value of type `Class`, but got type `{actual}`.\nValue: "
                    ))
                    .concat(&value),
                    "Class<T>",
                ));
            };
            let object = self.heap.get(self.heap.get_class(class_id)?.prototype)?;
            let class = object
                .class_definition
                .as_ref()
                .ok_or_else(|| invalid("class metadata missing"))?;
            let ty = object
                .builtin_type
                .clone()
                .unwrap_or_else(|| TypeCheck::Class {
                    // Module Class values display their module name directly,
                    // independently of the caller's module. Keep their actual
                    // class id for property/subtype lookup.
                    module: if object.is_module {
                        Some(class.name.clone())
                    } else {
                        class.module.clone()
                    },
                    id: class.id,
                    name: if object.is_module {
                        "ModuleClass".into()
                    } else {
                        class.name.clone()
                    },
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

    pub(super) fn reference_get_class(&mut self, value: &Value) -> Result<Value, RuntimeError> {
        match classes::runtime_class(value, &self.heap)? {
            classes::RuntimeClass::Declared(class) => Ok(Value::Class(class)),
            classes::RuntimeClass::Builtin(classes::BuiltinClass::Function(Some(arity)))
                if arity > 5 =>
            {
                Err(RuntimeError::new(
                    ErrorKind::Host,
                    format!("Class `Function{arity}` does not exist."),
                ))
            }
            classes::RuntimeClass::Builtin(key) => {
                Ok(Value::Class(self.canonical_builtin_class(key)))
            }
        }
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
        if values.is_empty() {
            return Value::Object(self.empty_strict_collection(CollectionKind::List));
        }
        let mut object = Object {
            collection_kind: CollectionKind::List,
            length: values.len(),
            ..Object::default()
        };
        for (index, value) in values.into_iter().enumerate() {
            let name: PklString = format!("\0{index}").into();
            object.members.push(Member {
                flags: 0,
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
                key: Value::Int((index as i64).into()),
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

    fn reference_access_value(
        &mut self,
        property: Option<&PklString>,
        key: Option<Value>,
    ) -> Value {
        let Value::Class(parent) = self.builtin_class(TypeCheck::Class {
            module: None,
            id: ACCESS_ID,
            name: "Access".into(),
        }) else {
            unreachable!()
        };
        let class = parent;
        let parent = self.heap.get_class(class).expect("Access class").prototype;
        let mut object = Object {
            parent: Some(parent),
            instance_class: Some(class),
            ..Object::default()
        };
        for (name, value) in [
            ("property", property.map_or(Value::Null, Value::string)),
            ("key", key.unwrap_or(Value::Null)),
        ] {
            let name = PklString::from(name);
            object.members.push(Member {
                flags: 0,
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
            // VmObjectBuilder supplies constant declarations, not evaluated
            // member caches. Ordinary lookup publishes these values lazily;
            // the prototype's computed flags retain their real dependencies.
            object.member_values.insert(
                CacheKey {
                    entry: false,
                    owner: None,
                    name,
                    method: false,
                },
                value,
            );
        }
        Value::Object(self.heap.allocate(object))
    }

    pub(super) fn reference_access(
        &mut self,
        id: ObjectId,
        property: Option<&PklString>,
        key: Option<Value>,
    ) -> Result<Option<Value>, RuntimeError> {
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
            match self.reference_candidate(&ty, property, key.as_ref())? {
                Candidate::Type(candidate) => candidates.push(candidate),
                Candidate::Unavailable => {
                    return Err(self.reference_access_error(
                        id,
                        &reference,
                        &ty,
                        property,
                        key.as_ref(),
                    )?);
                }
                Candidate::Pending => {
                    // The declaration continuation owns the pending resolution.
                    // Restored operands are roots below that continuation.
                    return Ok(None);
                }
            }
        }
        reference.referent = union(candidates, &self.program.module_name);
        reference.forced = false;
        reference
            .path
            .push(self.reference_access_value(property, key));
        Ok(Some(Value::Object(self.heap.allocate(Object {
            reference: Some(reference),
            ..Object::default()
        }))))
    }

    fn reference_access_error(
        &self,
        id: ObjectId,
        reference: &Reference,
        failed: &TypeCheck,
        property: Option<&PklString>,
        key: Option<&Value>,
    ) -> Result<RuntimeError, RuntimeError> {
        let failed = failed.resolved();
        let module = &self.program.module_name;
        let actual = export_type_name_value(reference, &self.heap, module)?;
        let referent = type_name_value(&reference.referent, module);
        let failed_type = type_name_value(failed, module);
        let union = !reference.referent.equivalent(failed);
        let mut error = if let Some(property) = property {
            let mut error = RuntimeError::new(
                ErrorKind::MissingMember,
                PklString::join(
                    &[
                        PklString::from("Cannot find property `")
                            .concat(property)
                            .concat(&"` in object of type `".into()),
                        actual.clone(),
                        "`.".into(),
                    ],
                    "",
                ),
            );
            error.hint = Some(
                if property == "output" && self.reference_module_class(failed) {
                    "Cannot reference property `output` because it is defined inside external class `Module`.".into()
                } else if property == "default"
                    && matches!(
                        failed,
                        TypeCheck::Collection(
                            CollectionKind::Dynamic
                                | CollectionKind::Listing
                                | CollectionKind::Mapping
                        ) | TypeCheck::ParameterizedCollection {
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
                    format!("Cannot reference property `default` in class `{name}`.").into()
                } else if union {
                    PklString::join(
                        &[
                            PklString::from("Cannot find property `")
                                .concat(property)
                                .concat(&"` in type `".into()),
                            referent,
                            PklString::from("`.\n\t└─ Cannot find property `")
                                .concat(property)
                                .concat(&"` in type `".into()),
                            failed_type,
                            "`.".into(),
                        ],
                        "",
                    )
                } else {
                    PklString::join(
                        &[
                            PklString::from("Cannot find property `")
                                .concat(property)
                                .concat(&"` in type `".into()),
                            referent,
                            "`.".into(),
                        ],
                        "",
                    )
                },
            );
            error
        } else {
            let key = key.expect("subscript");
            let key_type = crate::diagnostic_values::type_name(key, &self.heap, module)?;
            let left =
                crate::diagnostic_values::render(&Value::Object(id), &self.heap, module, 80)?;
            let right = crate::diagnostic_values::render(key, &self.heap, module, 80)?;
            let headline = PklString::join(
                &[
                    "Operator `[]` is not defined for operand types `".into(),
                    actual,
                    PklString::from("` and `")
                        .concat(&key_type)
                        .concat(&"`.".into()),
                ],
                "",
            );
            let mut error = RuntimeError::with_program_values(
                ErrorKind::Type,
                headline,
                &[("Left operand", left), ("Right operand", right)],
            );
            error.hint = Some(if union {
                PklString::join(
                    &[
                        "Operator `[]` is not defined for operand types `".into(),
                        referent,
                        PklString::from("` and `").concat(&key_type).concat(
                            &"`.\n\t└─ Operator `[]` is not defined for operand types `".into(),
                        ),
                        failed_type,
                        PklString::from("` and `")
                            .concat(&key_type)
                            .concat(&"`.".into()),
                    ],
                    "",
                )
            } else {
                PklString::join(
                    &[
                        "Operator `[]` is not defined for operand types `".into(),
                        referent,
                        PklString::from("` and `")
                            .concat(&key_type)
                            .concat(&"`.".into()),
                    ],
                    "",
                )
            });
            error
        };
        error.span = None;
        Ok(error)
    }

    // VmReference checks the class relation to base.Module before looking at
    // a property declaration; an overriding annotation cannot remove the ban.
    fn reference_module_class(&self, ty: &TypeCheck) -> bool {
        let TypeCheck::Class { id, .. } = ty.resolved() else {
            return false;
        };
        if matches!(*id, MODULE_ID | output::MODULE) {
            return true;
        }
        self.heap
            .find_class(*id)
            .zip(self.heap.find_class(output::MODULE))
            .is_some_and(|(class, module)| self.heap.is_subclass(class, module))
    }

    fn reference_candidate(
        &mut self,
        ty: &TypeCheck,
        property: Option<&PklString>,
        key: Option<&Value>,
    ) -> Result<Candidate, RuntimeError> {
        let scalar_property = property.and_then(PklString::as_str);
        let ty = ty.resolved();
        let candidate = match ty {
            TypeCheck::Unknown => TypeCheck::Unknown,
            TypeCheck::Null => TypeCheck::Null,
            TypeCheck::Collection(CollectionKind::Dynamic) => {
                if scalar_property == Some("default") {
                    return Ok(Candidate::Unavailable);
                }
                TypeCheck::Unknown
            }
            TypeCheck::Class { .. }
                if scalar_property == Some("output") && self.reference_module_class(ty) =>
            {
                return Ok(Candidate::Unavailable);
            }
            TypeCheck::Class { id, .. } if property.is_some() => {
                let name = property.expect("property access");
                if *id == ACCESS_ID {
                    return Ok(match scalar_property {
                        Some("property") => Candidate::Type(normalize(
                            &TypeCheck::Nullable(Box::new(TypeCheck::String)),
                            &self.program.module_name,
                        )),
                        Some("key") => Candidate::Type(TypeCheck::Any),
                        _ => Candidate::Unavailable,
                    });
                }
                if let Some(TypeCheck::Class { id: ds_id, .. }) = classes::named_type("DataSize")
                    && *id == ds_id
                {
                    return Ok(match scalar_property {
                        Some("value") => Candidate::Type(TypeCheck::Number),
                        Some("unit") => Candidate::Type(string_literal_union(&[
                            "b", "gb", "gib", "kb", "kib", "mb", "mib", "pb", "pib", "tb", "tib",
                        ])),
                        Some("isPositive" | "isBinaryUnit" | "isDecimalUnit") => {
                            Candidate::Type(TypeCheck::Boolean)
                        }
                        _ => Candidate::Unavailable,
                    });
                }
                if *id == classes::REGEX {
                    return Ok(match scalar_property {
                        Some("pattern") => Candidate::Type(TypeCheck::String),
                        Some("groupCount") => Candidate::Type(TypeCheck::Int),
                        _ => Candidate::Unavailable,
                    });
                }
                if let Some(TypeCheck::Class { id: seq_id, .. }) = classes::named_type("IntSeq")
                    && *id == seq_id
                {
                    return Ok(match scalar_property {
                        Some("start" | "end" | "step") => Candidate::Type(TypeCheck::Int),
                        _ => Candidate::Unavailable,
                    });
                }
                if self.heap.find_class(*id).is_none() && builtins::name(*id).is_some() {
                    let _ = self.builtin_class(builtins::ty(*id));
                }
                let mut current = self
                    .heap
                    .find_class(*id)
                    .map(|class| self.heap.get_class(class).expect("live class").prototype);
                let mut found = false;
                let mut visited = BTreeSet::new();
                while let Some(owner) = current {
                    if !visited.insert(owner) {
                        return Err(RuntimeError::new(ErrorKind::Cycle, "cyclic parent chain"));
                    }
                    let object = self.heap.get(owner)?;
                    if let Some(member) = object
                        .members
                        .iter()
                        .find(|m| !m.local && !m.method && &m.name == name)
                    {
                        found = true;
                        if let Some(ty) = &member.ty {
                            let ty = ty.clone();
                            // Class metadata carries a lexical declaration recipe,
                            // not a value check. Resolve it in the declaring
                            // prototype's environment before exporting metadata.
                            let mut environment = object.environment.clone();
                            environment.this_value = Some(Value::Object(owner));
                            environment.scopes.insert(
                                0,
                                Scope {
                                    receiver: owner,
                                    owner,
                                },
                            );
                            let Some(resolved) = self.resolve_metadata_type(&ty, &environment)?
                            else {
                                return Ok(Candidate::Pending);
                            };
                            return Ok(Candidate::Type(normalize(
                                &resolved,
                                &self.program.module_name,
                            )));
                        }
                    }
                    current = object.parent;
                }
                if !found {
                    return Ok(Candidate::Unavailable);
                }
                TypeCheck::Unknown
            }
            TypeCheck::ParameterizedCollection { kind, arguments } if property.is_none() => {
                let key = key.expect("subscript key");
                match kind {
                    CollectionKind::List | CollectionKind::Listing
                        if matches!(key, Value::Int(_)) =>
                    {
                        normalize(&arguments[0].0, &self.program.module_name)
                    }
                    CollectionKind::Mapping | CollectionKind::Map
                        if reference_key_matches(&arguments[0].0, key, &self.heap) =>
                    {
                        normalize(&arguments[1].0, &self.program.module_name)
                    }
                    _ => return Ok(Candidate::Unavailable),
                }
            }
            // Note: Upstream VmModifier.export only exports Modifier.EXTERNAL on
            // classes, never on properties (VmModifier.java:174), so PClass.Property
            // .isExternal() is always false in Pkl 0.32.0 and all builtin properties
            // below are referenceable (only Listing/Mapping reject property access).
            TypeCheck::Collection(CollectionKind::List)
            | TypeCheck::ParameterizedCollection {
                kind: CollectionKind::List,
                ..
            } if property.is_some() => {
                let elem = builtin_type_param(usize::MAX - 501, "Element");
                let nullable = |inner: TypeCheck| {
                    normalize(
                        &TypeCheck::Nullable(Box::new(inner)),
                        &self.program.module_name,
                    )
                };
                match scalar_property {
                    Some("length" | "lastIndex") => TypeCheck::Int,
                    Some("isEmpty" | "isNotEmpty" | "isDistinct") => TypeCheck::Boolean,
                    Some("first" | "last" | "single" | "min" | "minOrNull" | "max") => elem,
                    Some("firstOrNull" | "lastOrNull" | "singleOrNull" | "maxOrNull") => {
                        nullable(elem)
                    }
                    Some("rest" | "distinct") => {
                        parameterized_collection(CollectionKind::List, vec![elem])
                    }
                    Some("restOrNull") => {
                        nullable(parameterized_collection(CollectionKind::List, vec![elem]))
                    }
                    _ => return Ok(Candidate::Unavailable),
                }
            }
            TypeCheck::Collection(CollectionKind::Set)
            | TypeCheck::ParameterizedCollection {
                kind: CollectionKind::Set,
                ..
            } if property.is_some() => {
                let elem = builtin_type_param(usize::MAX - 502, "Element");
                let nullable = |inner: TypeCheck| {
                    normalize(
                        &TypeCheck::Nullable(Box::new(inner)),
                        &self.program.module_name,
                    )
                };
                match scalar_property {
                    Some("length") => TypeCheck::Int,
                    Some("isEmpty" | "isNotEmpty") => TypeCheck::Boolean,
                    Some("first" | "last" | "single" | "min" | "minOrNull" | "max") => elem,
                    Some("firstOrNull" | "lastOrNull" | "singleOrNull" | "maxOrNull") => {
                        nullable(elem)
                    }
                    Some("rest") => parameterized_collection(CollectionKind::Set, vec![elem]),
                    Some("restOrNull") => {
                        nullable(parameterized_collection(CollectionKind::Set, vec![elem]))
                    }
                    _ => return Ok(Candidate::Unavailable),
                }
            }
            TypeCheck::Collection(CollectionKind::Map)
            | TypeCheck::ParameterizedCollection {
                kind: CollectionKind::Map,
                ..
            } if property.is_some() => {
                let key = builtin_type_param(usize::MAX - 503, "Key");
                let value = builtin_type_param(usize::MAX - 504, "Value");
                match scalar_property {
                    Some("length") => TypeCheck::Int,
                    Some("isEmpty" | "isNotEmpty") => TypeCheck::Boolean,
                    Some("keys") => parameterized_collection(CollectionKind::Set, vec![key]),
                    Some("values") => parameterized_collection(CollectionKind::List, vec![value]),
                    Some("entries") => parameterized_collection(
                        CollectionKind::List,
                        vec![TypeCheck::Pair {
                            arguments: vec![(key, Span::new(0, 0)), (value, Span::new(0, 0))],
                        }],
                    ),
                    _ => return Ok(Candidate::Unavailable),
                }
            }
            TypeCheck::CollectionBase { .. } if property.is_some() => {
                let elem = builtin_type_param(usize::MAX - 500, "Element");
                let nullable = |inner: TypeCheck| {
                    normalize(
                        &TypeCheck::Nullable(Box::new(inner)),
                        &self.program.module_name,
                    )
                };
                let rest = || TypeCheck::CollectionBase {
                    arguments: vec![(elem.clone(), Span::new(0, 0))],
                };
                match scalar_property {
                    Some("length") => TypeCheck::Int,
                    Some("isEmpty" | "isNotEmpty") => TypeCheck::Boolean,
                    Some("first" | "last" | "single" | "min" | "max" | "maxOrNull") => elem,
                    Some("firstOrNull" | "lastOrNull" | "singleOrNull" | "minOrNull") => {
                        nullable(elem)
                    }
                    Some("rest") => rest(),
                    Some("restOrNull") => nullable(rest()),
                    _ => return Ok(Candidate::Unavailable),
                }
            }
            TypeCheck::Pair { .. } if property.is_some() => match scalar_property {
                Some("first" | "key") => builtin_type_param(usize::MAX - 505, "First"),
                Some("second" | "value") => builtin_type_param(usize::MAX - 506, "Second"),
                _ => return Ok(Candidate::Unavailable),
            },
            TypeCheck::String if property.is_some() => match scalar_property {
                Some("length" | "lastIndex" | "sha256Int") => TypeCheck::Int,
                Some(
                    "isEmpty" | "isNotEmpty" | "isBlank" | "isNotBlank" | "isRegex"
                    | "isGlobPattern" | "isBase64",
                ) => TypeCheck::Boolean,
                Some("md5" | "sha1" | "sha256" | "base64" | "base64Decoded") => TypeCheck::String,
                Some("base64DecodedBytes") => TypeCheck::Bytes,
                Some("chars") => {
                    parameterized_collection(CollectionKind::List, vec![TypeCheck::String])
                }
                Some("codePoints") => {
                    parameterized_collection(CollectionKind::List, vec![TypeCheck::Int])
                }
                _ => return Ok(Candidate::Unavailable),
            },
            TypeCheck::Int if property.is_some() => match scalar_property {
                Some("ns" | "us" | "ms" | "s" | "min" | "h" | "d") => TypeCheck::Duration,
                Some(
                    "b" | "kb" | "mb" | "gb" | "tb" | "pb" | "kib" | "mib" | "gib" | "tib" | "pib",
                ) => classes::named_type("DataSize").expect("DataSize class"),
                Some("sign" | "abs" | "ceil" | "floor" | "inv") => TypeCheck::Int,
                Some(
                    "isPositive" | "isFinite" | "isInfinite" | "isNaN" | "isEven" | "isOdd"
                    | "isNonZero",
                ) => TypeCheck::Boolean,
                _ => return Ok(Candidate::Unavailable),
            },
            TypeCheck::Float if property.is_some() => match scalar_property {
                Some("ns" | "us" | "ms" | "s" | "min" | "h" | "d") => TypeCheck::Duration,
                Some(
                    "b" | "kb" | "mb" | "gb" | "tb" | "pb" | "kib" | "mib" | "gib" | "tib" | "pib",
                ) => classes::named_type("DataSize").expect("DataSize class"),
                Some("sign" | "abs" | "ceil" | "floor") => TypeCheck::Float,
                Some("isPositive" | "isFinite" | "isInfinite" | "isNaN" | "isNonZero") => {
                    TypeCheck::Boolean
                }
                _ => return Ok(Candidate::Unavailable),
            },
            TypeCheck::Number if property.is_some() => match scalar_property {
                Some("ns" | "us" | "ms" | "s" | "min" | "h" | "d") => TypeCheck::Duration,
                Some(
                    "b" | "kb" | "mb" | "gb" | "tb" | "pb" | "kib" | "mib" | "gib" | "tib" | "pib",
                ) => classes::named_type("DataSize").expect("DataSize class"),
                Some("sign" | "abs" | "ceil" | "floor") => TypeCheck::Number,
                Some("isPositive" | "isFinite" | "isInfinite" | "isNaN" | "isNonZero") => {
                    TypeCheck::Boolean
                }
                _ => return Ok(Candidate::Unavailable),
            },
            TypeCheck::Duration if property.is_some() => match scalar_property {
                Some("value") => TypeCheck::Number,
                Some("isoString") => TypeCheck::String,
                Some("unit") => string_literal_union(&["d", "h", "min", "ms", "ns", "s", "us"]),
                Some("isPositive") => TypeCheck::Boolean,
                _ => return Ok(Candidate::Unavailable),
            },
            TypeCheck::Bytes if property.is_some() => match scalar_property {
                Some("length" | "sha256Int") => TypeCheck::Int,
                Some("size") => classes::named_type("DataSize").expect("DataSize class"),
                Some("base64" | "hex" | "md5" | "sha1" | "sha256") => TypeCheck::String,
                _ => return Ok(Candidate::Unavailable),
            },
            TypeCheck::MetaClass(name)
                if property.is_some()
                    && name == "Class"
                    && scalar_property == Some("simpleName") =>
            {
                TypeCheck::String
            }
            _ => return Ok(Candidate::Unavailable),
        };
        Ok(Candidate::Type(candidate))
    }
}
fn builtin_type_param(block: usize, name: &str) -> TypeCheck {
    TypeCheck::Parameter {
        block,
        name: name.into(),
    }
}
fn parameterized_collection(kind: CollectionKind, arguments: Vec<TypeCheck>) -> TypeCheck {
    TypeCheck::ParameterizedCollection {
        kind,
        arguments: arguments
            .into_iter()
            .map(|ty| (ty, Span::new(0, 0)))
            .collect(),
    }
}
fn string_literal_union(literals: &[&str]) -> TypeCheck {
    TypeCheck::Union {
        string_literals: false,
        types: literals
            .iter()
            .map(|literal| TypeCheck::StringLiteral((*literal).into()))
            .collect(),
        spans: Vec::new(),
        labels: Vec::new(),
        default: None,
    }
    .shared()
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

pub(crate) fn export_type_name_value(
    reference: &Reference,
    heap: &Heap,
    module: &PklString,
) -> Result<PklString, RuntimeError> {
    let domain = crate::diagnostic_values::type_name(&reference.domain, heap, module)?;
    Ok(PklString::join(
        &[
            PklString::from("pkl.ref#Reference<")
                .concat(&domain)
                .concat(&", ".into()),
            type_name_value(&reference.referent, module),
            ">".into(),
        ],
        "",
    ))
}

fn constructor_error(message: impl Into<PklString>, fragment: &str) -> RuntimeError {
    let source = crate::stdlib_sources::reference();
    let declaration = source
        .text
        .find("external const function Reference")
        .expect("reference declaration");
    let start = declaration
        + source
            .text
            .entire()
            .range(declaration..)
            .find(fragment)
            .expect("constructor type");
    let span = Span::new(start, start + fragment.len());
    let mut error = RuntimeError::new(ErrorKind::Type, message).at(span);
    error.frames.push(RuntimeFrame {
        anonymous: false,
        span,
        member: "#Reference".into(),
        source: Some(source),
    });
    error
}

/// Ordinary bytecode backs Access defaults, computed flags, and its property constraint.
#[cfg_attr(target_arch = "wasm32", inline(never))]
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
            name: "key".into(),
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
        member: "#Access.property".into(),
        ..BlockMetadata::default()
    });
    let property_type = TypeCheck::Nullable(Box::new(TypeCheck::Constrained {
        base: Box::new(TypeCheck::String),
        base_span: span("String(key == null)?"),
        constraints: vec![ConstraintCode {
            section: None,
            block: constraint,
            arguments: BTreeMap::new(),
            argument_sources: BTreeMap::new(),
            span: predicate_span,
            module_environment: false,
            source: "key == null".into(),
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
                Instruction::GetMember("property".into()),
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
                Instruction::GetMember("property".into()),
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
            vec![Instruction::Undefined("key".into()), Instruction::Return],
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
            member: format!("#Access.{name}").into(),
            declaration: Some(declaration),
            ..BlockMetadata::default()
        });
        blocks.push(code);
        members.push(Member {
            flags: 0,
            entry: false,
            name: name.into(),
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

    #[test]
    fn reference_module_class_names_do_not_acquire_the_callers_module_prefix() {
        let mut vm = Vm::new(Arc::new(Program::new(vec![vec![Instruction::Return]])));
        let Value::Class(domain_class) = vm.builtin_class(TypeCheck::RefDomain) else {
            unreachable!()
        };
        let domain_parent = vm.heap.get_class(domain_class).unwrap().prototype;
        let domain = Value::Object(vm.heap.allocate(Object {
            parent: Some(domain_parent),
            instance_class: Some(domain_class),
            ..Default::default()
        }));
        for (is_module, name, module, expected) in [
            (true, "imported.module", None, "imported.module"),
            (
                false,
                "Child",
                Some("imported.module"),
                "imported.module#Child",
            ),
        ] {
            let prototype = vm.heap.allocate(Object {
                is_module,
                class_definition: Some(Class {
                    module: module.map(PklString::from),
                    origin_uri: None,
                    id: if is_module { 700 } else { 701 },
                    name: name.into(),
                    open: false,
                    abstract_: false,
                }),
                ..Default::default()
            });
            let class = vm.heap.allocate_class(prototype);
            vm.invoke_reference_native(
                NativeFunction {
                    receiver: Value::Null,
                    name: "Reference".into(),
                },
                vec![domain.clone(), Value::Class(class), Value::Null],
            )
            .unwrap();
            let reference = vm.stack.pop().unwrap().as_object().unwrap();
            let reference = vm.heap.get(reference).unwrap().reference.as_ref().unwrap();
            assert_eq!(
                export_type_name_value(reference, &vm.heap, &"caller".into()).unwrap(),
                PklString::from(format!("pkl.ref#Reference<pkl.ref#Domain, {expected}>"))
            );
        }
        vm.close();
        assert_eq!(vm.heap_stats().slots, 0);
    }

    fn access_member(vm: &mut Vm, access: &Value, name: &str) -> Value {
        // Native constant declarations publish through the ordinary equality
        // continuation, which belongs to the caller's executing bytecode frame.
        let entry = Arc::make_mut(&mut vm.program)
            .alloc_block(
                vec![
                    Instruction::Input(0),
                    Instruction::GetMember(name.into()),
                    Instruction::Return,
                ],
                BlockMetadata {
                    spans: vec![None; 3],
                    ..Default::default()
                },
            )
            .unwrap();
        vm.start(entry, vec![access.clone()]).unwrap();
        for _ in 0..1_000 {
            vm.collect(std::slice::from_ref(access));
            match vm.poll(1) {
                VmPoll::Yielded => {}
                VmPoll::Ready(result) => return result.unwrap(),
                event => panic!("Access field requested unexpected work: {event:?}"),
            }
        }
        panic!("Access field did not settle")
    }

    #[test]
    fn reference_access_fields_stay_lazy_and_computed_flags_use_normal_caches() {
        let mut vm = Vm::new(Arc::new(Program::new(vec![vec![Instruction::Return]])));
        let access = vm.reference_access_value(Some(&"name".into()), None);
        let id = access.as_object().unwrap();
        assert!(vm.heap.get(id).unwrap().cache.is_empty());
        assert_eq!(
            access_member(&mut vm, &access, "isProperty"),
            Value::Bool(true)
        );
        let cached: Vec<_> = vm
            .heap
            .get(id)
            .unwrap()
            .completed_cache_order
            .iter()
            .map(|key| key.name.clone())
            .collect();
        assert_eq!(cached, ["property", "isProperty"]);
        assert_eq!(
            access_member(&mut vm, &access, "property"),
            Value::string("name")
        );
        assert_eq!(
            access_member(&mut vm, &access, "isSubscript"),
            Value::Bool(false)
        );
        assert_eq!(access_member(&mut vm, &access, "key"), Value::Null);
        vm.close();
        assert_eq!(vm.heap_stats().slots, 0);
    }

    #[test]
    fn diagnostics_render_unforced_reference_path_constants_after_gc() {
        let mut vm = Vm::new(Arc::new(Program::new(vec![vec![Instruction::Return]])));
        let property = vm.reference_access_value(Some(&"name".into()), None);
        let key = vm.heap.allocate(Object::default());
        let subscript = vm.reference_access_value(None, Some(Value::Object(key)));
        let reference = Value::Object(vm.heap.allocate(Object {
            reference: Some(Reference {
                domain: Value::Null,
                data: Value::Null,
                path: vec![property.clone(), subscript.clone()],
                referent: TypeCheck::Int,
                classes: vec![],
                forced: false,
            }),
            ..Object::default()
        }));
        vm.collect(std::slice::from_ref(&reference));
        assert!(vm.heap.get(key).is_ok(), "unforced path key remains traced");
        let render =
            |vm: &Vm| crate::diagnostic_values::render(&reference, &vm.heap, "test", 1000).unwrap();
        assert_eq!(
            render(&vm),
            "Reference(null, Int, null).name[new Dynamic {}]"
        );
        for access in [&property, &subscript] {
            assert!(
                vm.heap
                    .get(access.as_object().unwrap())
                    .unwrap()
                    .cache
                    .is_empty()
            );
        }
        // Completed caches retain the same rendering behavior. Ordinary forcing
        // and its cache publication are covered by the adjacent Access tests.
        for access in [&property, &subscript] {
            let object = vm.heap.get_mut(access.as_object().unwrap()).unwrap();
            for (key, value) in object.member_values.clone() {
                object.cache.insert(key, LazyState::Completed(value));
            }
        }
        vm.collect(std::slice::from_ref(&reference));
        assert_eq!(
            render(&vm),
            "Reference(null, Int, null).name[new Dynamic {}]"
        );
        vm.close();
        assert_eq!(vm.heap_stats().slots, 0);
    }

    #[test]
    fn lazy_reference_access_payload_survives_gc_and_amendment_uses_its_own_cache() {
        let mut vm = Vm::new(Arc::new(Program::new(vec![vec![Instruction::Return]])));
        let key = vm.heap.allocate(Object::default());
        let parent = vm.reference_access_value(None, Some(Value::Object(key)));
        let parent_id = parent.as_object().unwrap();
        let instance_class = vm.heap.get(parent_id).unwrap().instance_class;
        let access = Value::Object(vm.heap.allocate(Object {
            parent: Some(parent_id),
            instance_class,
            ..Object::default()
        }));
        vm.collect(std::slice::from_ref(&access));
        assert!(
            vm.heap.get(key).is_ok(),
            "unforced native key must remain traced"
        );
        assert_eq!(
            access_member(&mut vm, &access, "isSubscript"),
            Value::Bool(true)
        );
        assert!(vm.heap.get(parent_id).unwrap().cache.is_empty());
        assert_eq!(access_member(&mut vm, &access, "key"), Value::Object(key));
        vm.collect(std::slice::from_ref(&access));
        assert!(vm.heap.get(key).is_ok());
        vm.close();
        assert_eq!(vm.heap_stats().slots, 0);
    }

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
                let normalized = normalize(&integer, &"test".into());
                assert_eq!(metadata_key(&integer), metadata_key(&normalized));
                assert_eq!(metadata_key(&integer), metadata_key(&same));
                assert_ne!(metadata_key(&integer), metadata_key(&number));
                assert_eq!(
                    type_name(&normalized, &"test".into()),
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
        assert_eq!(key, metadata_key(&normalize(&branching, &"test".into())));
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
                module: None,
                id: DOMAIN_ID,
                name: "Domain".into()
            }
            .shared(),
            &TypeCheck::RefDomain,
            &Heap::default(),
        ));
        let alias = |declaration| {
            TypeCheck::NamedAlias {
                declaration,
                default_origin: None,
                name: "Alias".into(),
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
            metadata_key(&normalize(&a, &"test".into())),
            metadata_key(&normalize(&b, &"test".into()))
        );
        assert_eq!(
            type_name(&a, &"test".into()),
            "(test#Alias<String>) -> Boolean"
        );
        assert!(!is_subtype(&a, &a, &Heap::default()));
        let constrained = |source: &str| {
            TypeCheck::Constrained {
                base: Box::new(TypeCheck::Int),
                base_span: Span::new(0, 0),
                constraints: vec![ConstraintCode {
                    section: None,
                    block: 0,
                    arguments: BTreeMap::new(),
                    argument_sources: BTreeMap::new(),
                    source: source.into(),
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
            metadata_key(&normalize(&constrained("this > 0"), &"test".into())),
            metadata_key(&TypeCheck::Int)
        );
    }
}
