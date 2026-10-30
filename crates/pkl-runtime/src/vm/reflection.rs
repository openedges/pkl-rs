//! Pinned mirror factories. Factory fields read their declaration owner's
//! payload; amendments retain ordinary virtual member lookup and type checks.
use super::*;
use crate::heap::CollectionKind;
mod declarations;

const FACTORY_BASE: usize = usize::MAX - 1200;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Module,
    Class,
    TypeAlias,
    Property,
    Method,
    MethodParameter,
    TypeParameter,
    DeclaredType,
    StringLiteralType,
    UnionType,
    NullableType,
    FunctionType,
    ModuleType,
    UnknownType,
    NothingType,
    TypeVariable,
    SourceLocation,
}
impl Kind {
    fn name(self) -> &'static str {
        match self {
            Self::Module => "Module",
            Self::Class => "Class",
            Self::TypeAlias => "TypeAlias",
            Self::Property => "Property",
            Self::Method => "Method",
            Self::MethodParameter => "MethodParameter",
            Self::TypeParameter => "TypeParameter",
            Self::DeclaredType => "DeclaredType",
            Self::StringLiteralType => "StringLiteralType",
            Self::UnionType => "UnionType",
            Self::NullableType => "NullableType",
            Self::FunctionType => "FunctionType",
            Self::ModuleType => "ModuleType",
            Self::UnknownType => "UnknownType",
            Self::NothingType => "NothingType",
            Self::TypeVariable => "TypeVariable",
            Self::SourceLocation => "SourceLocation",
        }
    }
    fn fields(self) -> &'static [&'static str] {
        match self {
            Self::Module => &[
                "location",
                "docComment",
                "annotations",
                "modifiers",
                "name",
                "uri",
                "reflectee",
                "moduleClass",
                "supermodule",
                "isAmend",
                "imports",
                "classes",
                "typeAliases",
            ],
            Self::Class => &[
                "location",
                "docComment",
                "annotations",
                "modifiers",
                "name",
                "reflectee",
                "typeParameters",
                "superclass",
                "supertype",
                "properties",
                "allProperties",
                "enclosingDeclaration",
                "methods",
                "allMethods",
            ],
            Self::TypeAlias => &[
                "location",
                "docComment",
                "annotations",
                "modifiers",
                "name",
                "reflectee",
                "typeParameters",
                "enclosingDeclaration",
                "referent",
            ],
            Self::Property => &[
                "location",
                "docComment",
                "annotations",
                "allAnnotations",
                "modifiers",
                "allModifiers",
                "name",
                "type",
                "defaultValue",
            ],
            Self::Method => &[
                "location",
                "docComment",
                "annotations",
                "modifiers",
                "typeParameters",
                "name",
                "parameters",
                "returnType",
            ],
            Self::MethodParameter => &["name", "type"],
            Self::TypeParameter => &["name", "variance"],
            Self::DeclaredType => &["referent", "typeArguments"],
            Self::StringLiteralType => &["value"],
            Self::UnionType => &["members"],
            Self::NullableType => &["member"],
            Self::FunctionType => &["parameterTypes", "returnType"],
            Self::TypeVariable => &["referent"],
            Self::SourceLocation => &["line", "column", "displayUri"],
            Self::ModuleType | Self::UnknownType | Self::NothingType => &[],
        }
    }
}
const KINDS: [Kind; 17] = [
    Kind::Module,
    Kind::Class,
    Kind::TypeAlias,
    Kind::Property,
    Kind::Method,
    Kind::MethodParameter,
    Kind::TypeParameter,
    Kind::DeclaredType,
    Kind::StringLiteralType,
    Kind::UnionType,
    Kind::NullableType,
    Kind::FunctionType,
    Kind::ModuleType,
    Kind::UnknownType,
    Kind::NothingType,
    Kind::TypeVariable,
    Kind::SourceLocation,
];

#[derive(Clone, Copy, Debug)]
pub enum Operation {
    Module,
    ModuleOf,
    Class,
    TypeAlias,
    DeclaredType,
    FunctionType,
    StringLiteralType,
    UnionType,
    TypeVariable,
    ModuleType,
    UnknownType,
    NothingType,
    Nullable,
    IsSubclassOf,
    WithTypeArgument,
    WithTypeArguments,
}
impl Operation {
    pub(super) fn binding(name: &PklString) -> Option<Self> {
        Some(match name.as_str()? {
            "#Module" => Self::Module,
            "#moduleOf" => Self::ModuleOf,
            "#Class" => Self::Class,
            "#TypeAlias" => Self::TypeAlias,
            "#DeclaredType" => Self::DeclaredType,
            "#FunctionType" => Self::FunctionType,
            "#StringLiteralType" => Self::StringLiteralType,
            "#UnionType" => Self::UnionType,
            "#TypeVariable" => Self::TypeVariable,
            "#moduleType" => Self::ModuleType,
            "#unknownType" => Self::UnknownType,
            "#nothingType" => Self::NothingType,
            "#Type.nullable" => Self::Nullable,
            "#Class.isSubclassOf" => Self::IsSubclassOf,
            "#DeclaredType.withTypeArgument" => Self::WithTypeArgument,
            "#DeclaredType.withTypeArguments" => Self::WithTypeArguments,
            _ => return None,
        })
    }
    fn arity(self) -> usize {
        match self {
            Self::FunctionType => 2,
            Self::ModuleType | Self::UnknownType | Self::NothingType | Self::Nullable => 0,
            _ => 1,
        }
    }
}

/// Source descriptors never contain guest references. The runtime payload keeps
/// the declaration/prototype and executable environment alive independently.
#[derive(Clone, Debug)]
pub struct Payload {
    pub(crate) kind: Kind,
    pub(crate) data: Data,
}
#[derive(Clone, Debug)]
pub(crate) enum Data {
    Module(ObjectId),
    Class(ObjectId),
    TypeAlias(ObjectId),
    Member(declarations::Declaration),
    Recipe {
        ty: crate::syntax::Type,
        context: declarations::Context,
    },
    Fields(Vec<(&'static str, Value)>),
    Location(RuntimeFrame),
}
impl Payload {
    pub(crate) fn roots(&self) -> Vec<Value> {
        match &self.data {
            Data::Module(id) | Data::TypeAlias(id) => vec![Value::Object(*id)],
            Data::Member(declaration) => vec![Value::Object(declaration.owner)],
            Data::Recipe { context, .. } => vec![Value::Object(context.owner)],
            Data::Class(id) => vec![Value::Class(*id)],
            Data::Fields(fields) => fields.iter().map(|(_, value)| value.clone()).collect(),
            Data::Location(_) => Vec::new(),
        }
    }
    fn fields(kind: Kind, fields: Vec<(&'static str, Value)>) -> Self {
        Self {
            kind,
            data: Data::Fields(fields),
        }
    }
}

pub(super) fn builtin_members(
    blocks: &mut Vec<Vec<Instruction>>,
    metadata: &mut Vec<BlockMetadata>,
) -> BTreeMap<usize, Vec<Member>> {
    let mut factories = BTreeMap::new();
    for kind in KINDS {
        let members = kind
            .fields()
            .iter()
            .map(|name| {
                let code = blocks.len();
                blocks.push(vec![
                    Instruction::MirrorProperty((*name).into()),
                    Instruction::Return,
                ]);
                // VmObjectFactory's synthetic field has no source section and no
                // authored modifier/type-check node. The prototype remains intact.
                metadata.push(BlockMetadata {
                    spans: vec![None; 2],
                    ..Default::default()
                });
                Member {
                    flags: 0,
                    entry: false,
                    name: (*name).into(),
                    code,
                    local: false,
                    method: false,
                    ty: None,
                    type_span: None,
                    constant: false,
                    fixed: false,
                }
            })
            .collect();
        factories.insert(FACTORY_BASE - kind as usize, members);
    }
    factories
}

#[derive(Clone, Debug)]
enum Task {
    Call(Operation, Value, Vec<Value>),
    Value(Value),
    DefaultDeclaredType,
    NullableClass(Value),
    Annotations(declarations::Declaration),
    ExecuteAnnotations(declarations::Declaration),
    FinishAnnotations(declarations::Declaration),
    ConcatLists(usize),
    ProbeDefault(declarations::Declaration),
    AfterDefault(declarations::Declaration),
    Make(Payload),
    ImportReflect,
    Read(ObjectId, PklString),
    FactoryClass(Payload),
    Allocate(Payload),
    Field(Payload, PklString),
    BuildList(usize),
    BuildFields(Kind, Vec<&'static str>),
    ReplaceArguments {
        arguments: Value,
        single: bool,
    },
    BuildMap(Vec<PklString>),
    Type(crate::syntax::Type, declarations::Context),
    ResolveName {
        ty: crate::syntax::Type,
        context: declarations::Context,
    },
    ResolveImportedScope {
        ty: crate::syntax::Type,
        context: declarations::Context,
    },
    ResolveSimpleBase {
        module: ObjectId,
        name: PklString,
        span: Span,
    },
    ResolveQualifiedType {
        name: PklString,
        span: Span,
        name_span: Span,
    },
    ReadDeclared {
        owner: ObjectId,
        name: PklString,
        local: bool,
    },
    ReadPath {
        rest: Vec<PklString>,
    },
    AsMirror(Span),
    TypeArguments {
        ty: crate::syntax::Type,
        context: declarations::Context,
    },
    ExplicitTypeArguments {
        arguments: Vec<crate::syntax::Type>,
        context: declarations::Context,
    },
    DefaultArguments,
    ImportModule(PklString),
    AsModuleMirror,
}
#[derive(Clone, Debug)]
pub(super) struct Run {
    tasks: Vec<Task>,
    values: Vec<Value>,
    waiting: bool,
}
impl Run {
    pub(super) fn roots(&self) -> Vec<Value> {
        let mut roots = self.values.clone();
        for task in &self.tasks {
            match task {
                Task::Call(_, receiver, arguments) => {
                    roots.push(receiver.clone());
                    roots.extend(arguments.iter().cloned());
                }
                Task::Make(payload)
                | Task::FactoryClass(payload)
                | Task::Allocate(payload)
                | Task::Field(payload, _) => roots.extend(payload.roots()),
                Task::Read(id, _)
                | Task::ReadDeclared { owner: id, .. }
                | Task::ResolveSimpleBase { module: id, .. } => roots.push(Value::Object(*id)),
                Task::ReplaceArguments { arguments, .. }
                | Task::Value(arguments)
                | Task::NullableClass(arguments) => roots.push(arguments.clone()),
                Task::ProbeDefault(declaration)
                | Task::AfterDefault(declaration)
                | Task::Annotations(declaration)
                | Task::ExecuteAnnotations(declaration)
                | Task::FinishAnnotations(declaration) => {
                    roots.push(Value::Object(declaration.owner))
                }
                Task::Type(_, context)
                | Task::ResolveName { context, .. }
                | Task::ResolveImportedScope { context, .. }
                | Task::TypeArguments { context, .. }
                | Task::ExplicitTypeArguments { context, .. } => {
                    roots.push(Value::Object(context.owner))
                }
                Task::ImportReflect
                | Task::ConcatLists(_)
                | Task::DefaultDeclaredType
                | Task::BuildList(_)
                | Task::BuildFields(_, _)
                | Task::BuildMap(_)
                | Task::ReadPath { .. }
                | Task::AsMirror(_)
                | Task::ResolveQualifiedType { .. }
                | Task::DefaultArguments
                | Task::ImportModule(_)
                | Task::AsModuleMirror => (),
            }
        }
        roots
    }
    fn new(task: Task) -> Self {
        Self {
            tasks: vec![task],
            values: Vec::new(),
            waiting: false,
        }
    }
    fn make(&mut self, kind: Kind, fields: Vec<(&'static str, Value)>) {
        self.tasks.push(Task::Make(Payload::fields(kind, fields)));
    }
}

impl Vm {
    pub(super) fn begin_reflection(&mut self, operation: Operation) -> Result<(), RuntimeError> {
        let frame = self
            .frames
            .last()
            .ok_or_else(|| invalid("reflection call without frame"))?;
        let arity = operation.arity();
        let start = frame
            .environment
            .locals
            .len()
            .checked_sub(arity)
            .ok_or_else(|| invalid("reflection argument frame"))?;
        let arguments = frame.environment.locals[start..].to_vec();
        let receiver = self.this_receiver()?;
        self.frames.last_mut().unwrap().reflection = Some(Box::new(Run::new(Task::Call(
            operation, receiver, arguments,
        ))));
        Ok(())
    }

    pub(super) fn begin_mirror_property(&mut self, name: PklString) -> Result<(), RuntimeError> {
        let owner = self
            .frames
            .last()
            .and_then(|frame| frame.force.as_ref())
            .map(|force| force.owner)
            .ok_or_else(|| invalid("mirror field without declaration owner"))?;
        let payload = self
            .heap
            .get(owner)?
            .mirror
            .clone()
            .ok_or_else(|| invalid("missing mirror factory payload"))?;
        self.frames.last_mut().unwrap().reflection =
            Some(Box::new(Run::new(Task::Field(*payload, name))));
        Ok(())
    }

    pub(super) fn step_reflection(&mut self) -> Result<(), RuntimeError> {
        let index = self.frames.len() - 1;
        let mut run = self.frames[index]
            .reflection
            .take()
            .ok_or_else(|| invalid("missing reflection continuation"))?;
        if run.waiting {
            run.values.push(self.pop()?);
            run.waiting = false;
        }
        let Some(task) = run.tasks.pop() else {
            let value = run
                .values
                .pop()
                .ok_or_else(|| invalid("mirror result missing"))?;
            if !run.values.is_empty() {
                return Err(invalid("unconsumed mirror results"));
            }
            self.stack.push(value);
            return Ok(());
        };
        match task {
            Task::ImportReflect | Task::ImportModule(_) => {
                let uri = if let Task::ImportModule(uri) = task {
                    uri
                } else {
                    "pkl:reflect".into()
                };
                run.waiting = true;
                self.frames[index].reflection = Some(run);
                self.import_stdlib(&uri)?;
                return Ok(());
            }
            Task::ReadDeclared { owner, name, local } => {
                run.waiting = true;
                self.frames[index].reflection = Some(run);
                self.force(owner, owner, name, local, true, false)?;
                return Ok(());
            }
            Task::Read(id, name) => {
                run.waiting = true;
                self.frames[index].reflection = Some(run);
                let local = self
                    .heap
                    .get(id)?
                    .members
                    .iter()
                    .any(|member| member.name == name && !member.method && member.local);
                self.force(id, id, name, local, true, false)?;
                return Ok(());
            }
            Task::Make(payload) => {
                let cached = match &payload.data {
                    Data::Module(id) => self.heap.get(*id)?.module_mirror,
                    Data::TypeAlias(id) => self.heap.get(*id)?.declaration_mirror,
                    Data::Class(id) => {
                        self.heap
                            .get(self.heap.get_class(*id)?.prototype)?
                            .declaration_mirror
                    }
                    _ => None,
                };
                if let Some(id) = cached {
                    run.values.push(Value::Object(id));
                } else {
                    run.tasks.push(Task::FactoryClass(payload));
                    run.tasks.push(Task::ImportReflect);
                }
            }
            Task::FactoryClass(payload) => {
                let module = run
                    .values
                    .pop()
                    .and_then(|value| value.heap_id())
                    .ok_or_else(|| invalid("reflect module missing"))?;
                let name = payload.kind.name().into();
                run.tasks.push(Task::Allocate(payload));
                run.tasks.push(Task::Read(module, name));
            }
            Task::Allocate(payload) => {
                let Some(Value::Class(class)) = run.values.pop() else {
                    return Err(invalid("mirror prototype is not a class"));
                };
                // Import/class initialization can suspend and permit a nested
                // evaluation to publish the same declaration's mirror first.
                // Keep the canonical cached identity after that reentry.
                let cached = match &payload.data {
                    Data::Module(owner) => self.heap.get(*owner)?.module_mirror,
                    Data::TypeAlias(owner) => self.heap.get(*owner)?.declaration_mirror,
                    Data::Class(owner) => {
                        self.heap
                            .get(self.heap.get_class(*owner)?.prototype)?
                            .declaration_mirror
                    }
                    _ => None,
                };
                if let Some(id) = cached {
                    run.values.push(Value::Object(id));
                    self.frames[index].reflection = Some(run);
                    return Ok(());
                }
                let prototype = self.heap.get_class(class)?.prototype;
                let id = self.heap.allocate(Object {
                    parent: Some(prototype),
                    instance_class: Some(class),
                    members: self.program.builtin_members[&(FACTORY_BASE - payload.kind as usize)]
                        .clone(),
                    mirror: Some(Box::new(payload.clone())),
                    ..Object::default()
                });
                match &payload.data {
                    Data::Module(owner) => self.heap.get_mut(*owner)?.module_mirror = Some(id),
                    Data::TypeAlias(owner) => {
                        self.heap.get_mut(*owner)?.declaration_mirror = Some(id)
                    }
                    Data::Class(owner) => {
                        let prototype = self.heap.get_class(*owner)?.prototype;
                        self.heap.get_mut(prototype)?.declaration_mirror = Some(id);
                    }
                    _ => (),
                }
                run.values.push(Value::Object(id));
            }
            Task::BuildList(length) => {
                let start = run
                    .values
                    .len()
                    .checked_sub(length)
                    .ok_or_else(|| invalid("mirror list operands"))?;
                self.stack.extend(run.values.drain(start..));
                self.collection_instruction(Instruction::MakeList { length })?;
                run.values.push(self.pop()?);
            }
            Task::BuildFields(kind, names) => {
                let start = run
                    .values
                    .len()
                    .checked_sub(names.len())
                    .ok_or_else(|| invalid("mirror field operands"))?;
                let values = run.values.drain(start..).collect::<Vec<_>>();
                run.make(kind, names.into_iter().zip(values).collect());
            }
            Task::ReplaceArguments { arguments, single } => {
                let referent = run
                    .values
                    .pop()
                    .ok_or_else(|| invalid("declared type referent"))?;
                let expected = self.mirror_parameter_count(&referent)?;
                let actual = if single {
                    1
                } else {
                    self.heap
                        .get(
                            arguments
                                .heap_id()
                                .ok_or_else(|| invalid("type argument list"))?,
                        )?
                        .length
                };
                if actual != expected {
                    let mut error = RuntimeError::new(
                        ErrorKind::Type,
                        format!("Expected {expected} type argument(s) but got {actual}."),
                    );
                    // DeclaredTypeNodes throws from an external node whose
                    // source section is unavailable. Keep the caller frame,
                    // without inventing a frame at the native declaration.
                    error.unavailable_origin = true;
                    return Err(error);
                }
                let arguments = if single {
                    self.stack.push(arguments);
                    self.collection_instruction(Instruction::MakeList { length: 1 })?;
                    self.pop()?
                } else {
                    arguments
                };
                run.make(
                    Kind::DeclaredType,
                    vec![("referent", referent), ("typeArguments", arguments)],
                );
            }
            Task::Annotations(declaration) => {
                let object = self.heap.get(declaration.owner)?;
                if object.builtin_type.is_none() && declaration.code.is_some() {
                    let values = self.reflection_existing_annotations(&declaration)?;
                    run.values.extend(values.iter().cloned());
                    run.tasks.push(Task::BuildList(values.len()));
                } else if let Some(values) = object
                    .mirror_annotations
                    .get(&declaration.syntax.header_span)
                    .cloned()
                {
                    run.values.extend(values.iter().cloned());
                    run.tasks.push(Task::BuildList(values.len()));
                } else {
                    run.tasks
                        .push(Task::ExecuteAnnotations(declaration.clone()));
                    run.tasks
                        .push(Task::ImportModule(declaration.source.uri.clone()));
                }
            }
            Task::ExecuteAnnotations(declaration) => {
                let module = run
                    .values
                    .pop()
                    .and_then(|value| value.heap_id())
                    .ok_or_else(|| invalid("builtin annotation module"))?;
                if let Some(block) = self
                    .program
                    .reflection_annotations
                    .get(&(
                        declaration.source.uri.clone(),
                        declaration.syntax.header_span,
                    ))
                    .copied()
                {
                    let mut environment = self.heap.get(module)?.environment.clone();
                    environment.this_value = Some(Value::Object(module));
                    environment.scopes.insert(
                        0,
                        Scope {
                            owner: module,
                            receiver: module,
                        },
                    );
                    let function = self.heap.allocate(Object {
                        function: Some(Function {
                            block,
                            arity: 0,
                            method: false,
                            captured_this: None,
                        }),
                        environment,
                        ..Object::default()
                    });
                    run.tasks.push(Task::FinishAnnotations(declaration));
                    run.waiting = true;
                    self.frames[index].reflection = Some(run);
                    self.invoke_function(function, Vec::new(), false)?;
                    return Ok(());
                } else {
                    self.heap
                        .get_mut(declaration.owner)?
                        .mirror_annotations
                        .insert(declaration.syntax.header_span, Vec::new());
                    run.tasks.push(Task::BuildList(0));
                }
            }
            Task::FinishAnnotations(declaration) => {
                let collector = run
                    .values
                    .pop()
                    .and_then(|value| value.heap_id())
                    .ok_or_else(|| invalid("annotation collector missing"))?;
                let values = self.heap.get(collector)?.annotations.clone();
                self.heap
                    .get_mut(declaration.owner)?
                    .mirror_annotations
                    .insert(declaration.syntax.header_span, values.clone());
                run.values.extend(values.iter().cloned());
                run.tasks.push(Task::BuildList(values.len()));
            }
            Task::ConcatLists(length) => {
                let start = run
                    .values
                    .len()
                    .checked_sub(length)
                    .ok_or_else(|| invalid("annotation concatenation"))?;
                let lists = run.values.drain(start..).collect::<Vec<_>>();
                let mut values = Vec::new();
                for list in lists {
                    let id = list.heap_id().ok_or_else(|| invalid("annotation list"))?;
                    for member in collections::ordered_members(&self.heap, id)? {
                        values.push(
                            collections::cached_render_value(&self.heap, id, &member)?
                                .ok_or_else(|| invalid("strict annotation list value"))?,
                        );
                    }
                }
                let length = values.len();
                run.values.extend(values);
                run.tasks.push(Task::BuildList(length));
            }
            Task::Value(value) => run.values.push(value),
            Task::NullableClass(receiver) => {
                let Some(Value::Class(expected)) = run.values.pop() else {
                    return Err(invalid("nullable mirror class"));
                };
                let actual = self.heap.object_class(
                    receiver
                        .heap_id()
                        .ok_or_else(|| invalid("nullable mirror receiver"))?,
                )?;
                if actual == Some(expected) {
                    run.values.push(receiver);
                } else {
                    run.make(Kind::NullableType, vec![("member", receiver)]);
                }
            }
            Task::DefaultDeclaredType => {
                let referent = run
                    .values
                    .pop()
                    .ok_or_else(|| invalid("default declared type referent"))?;
                self.reflection_call(
                    Operation::DeclaredType,
                    Value::Null,
                    vec![referent],
                    &mut run,
                )?;
            }
            Task::ProbeDefault(declaration) => {
                // Keep the declaration's lexical environment while resolving
                // its default recipe, and do not replay the caller opcode.
                let object = self.heap.get(declaration.owner)?;
                let member = declaration
                    .code
                    .and_then(|code| object.members.iter().find(|member| member.code == code));
                let ty = member.and_then(|member| member.ty.clone());
                if let Some(ty) = ty {
                    let mut environment = object.environment.clone();
                    environment.this_value = Some(Value::Object(declaration.owner));
                    environment.scopes.insert(
                        0,
                        Scope {
                            owner: declaration.owner,
                            receiver: declaration.owner,
                        },
                    );
                    run.tasks.push(Task::ProbeDefault(declaration.clone()));
                    self.frames[index].reflection = Some(run);
                    let Some(ty) = self.resolve_runtime_type(&ty, &environment, true)? else {
                        return Ok(());
                    };
                    run = self.frames[index].reflection.take().unwrap();
                    run.tasks.pop();
                    run.tasks.push(Task::AfterDefault(declaration));
                    run.waiting = true;
                    self.frames[index].generic_default =
                        Some(generics::GenericDefaultRun::probe(ty));
                } else {
                    run.values.push(Value::Null);
                }
            }
            Task::AfterDefault(declaration) => {
                if run.values.pop() == Some(Value::Bool(true)) {
                    run.tasks.push(Task::Read(
                        declaration.owner,
                        declaration.syntax.name.clone(),
                    ));
                } else {
                    run.values.push(Value::Null);
                }
            }
            Task::Call(operation, receiver, args) => {
                self.reflection_call(operation, receiver, args, &mut run)?
            }
            Task::Field(payload, name) => self.reflection_field(payload, &name, &mut run)?,
            task => self.reflection_declaration_task(task, &mut run)?,
        }
        self.frames[index].reflection = Some(run);
        Ok(())
    }

    fn reflection_call(
        &mut self,
        operation: Operation,
        receiver: Value,
        args: Vec<Value>,
        run: &mut Run,
    ) -> Result<(), RuntimeError> {
        match operation {
            Operation::Module | Operation::ModuleOf => {
                let mut id = args[0]
                    .heap_id()
                    .ok_or_else(|| invalid("module mirror argument"))?;
                // ModuleInfo belongs only to an actual loaded module. An object
                // amendment can inherit module behavior without owning a unit.
                if matches!(operation, Operation::Module)
                    && self.heap.get(id)?.module_unit.is_none()
                {
                    return Err(RuntimeError::new(
                        ErrorKind::Type,
                        "Expected a module as argument, but got an object that amends a module.",
                    ));
                }
                while self.heap.get(id)?.module_unit.is_none() {
                    id = self
                        .heap
                        .get(id)?
                        .parent
                        .ok_or_else(|| invalid("No module found in prototype chain."))?;
                }
                run.tasks.push(Task::Make(Payload {
                    kind: Kind::Module,
                    data: Data::Module(id),
                }));
            }
            Operation::Class => {
                let Value::Class(id) = args[0] else {
                    return Err(invalid("class mirror argument"));
                };
                run.tasks.push(Task::Make(Payload {
                    kind: Kind::Class,
                    data: Data::Class(id),
                }));
            }
            Operation::TypeAlias => {
                let Value::TypeAlias(id) = args[0] else {
                    return Err(invalid("alias mirror argument"));
                };
                run.tasks.push(Task::Make(Payload {
                    kind: Kind::TypeAlias,
                    data: Data::TypeAlias(id),
                }));
            }
            Operation::DeclaredType => {
                let count = self.mirror_parameter_count(&args[0])?;
                run.values.push(args[0].clone());
                run.tasks.push(Task::BuildFields(
                    Kind::DeclaredType,
                    vec!["referent", "typeArguments"],
                ));
                run.tasks.push(Task::BuildList(count));
                for _ in 0..count {
                    run.tasks
                        .push(Task::Make(Payload::fields(Kind::UnknownType, vec![])));
                }
            }
            Operation::FunctionType => run.make(
                Kind::FunctionType,
                vec![
                    ("parameterTypes", args[0].clone()),
                    ("returnType", args[1].clone()),
                ],
            ),
            Operation::StringLiteralType => {
                run.make(Kind::StringLiteralType, vec![("value", args[0].clone())])
            }
            Operation::UnionType => run.make(Kind::UnionType, vec![("members", args[0].clone())]),
            Operation::TypeVariable => {
                run.make(Kind::TypeVariable, vec![("referent", args[0].clone())])
            }
            Operation::ModuleType => run.make(Kind::ModuleType, vec![]),
            Operation::UnknownType => run.make(Kind::UnknownType, vec![]),
            Operation::NothingType => run.make(Kind::NothingType, vec![]),
            Operation::Nullable => {
                run.tasks.push(Task::NullableClass(receiver));
                run.tasks.push(Task::ReadPath {
                    rest: vec!["NullableType".into()],
                });
                run.tasks.push(Task::ImportReflect);
            }
            Operation::IsSubclassOf => {
                let left = self.mirror_class(&receiver)?;
                let right = self.mirror_class(&args[0])?;
                run.values
                    .push(Value::Bool(self.heap.is_subclass(left, right)));
            }
            Operation::WithTypeArgument | Operation::WithTypeArguments => {
                run.tasks.push(Task::ReplaceArguments {
                    arguments: args[0].clone(),
                    single: matches!(operation, Operation::WithTypeArgument),
                });
                run.tasks.push(Task::Read(
                    receiver
                        .heap_id()
                        .ok_or_else(|| invalid("declared mirror receiver"))?,
                    "referent".into(),
                ));
            }
        }
        Ok(())
    }

    fn mirror_class(&self, value: &Value) -> Result<ObjectId, RuntimeError> {
        let payload = self
            .heap
            .get(value.heap_id().ok_or_else(|| invalid("mirror receiver"))?)?
            .mirror
            .as_deref()
            .ok_or_else(|| invalid("missing direct mirror payload"))?;
        if let Data::Class(id) = payload.data {
            Ok(id)
        } else {
            Err(invalid("not a class mirror payload"))
        }
    }

    fn mirror_parameter_count(&self, value: &Value) -> Result<usize, RuntimeError> {
        let payload = self
            .heap
            .get(
                value
                    .heap_id()
                    .ok_or_else(|| invalid("type declaration mirror"))?,
            )?
            .mirror
            .as_deref()
            .ok_or_else(|| invalid("missing type declaration payload"))?;
        if let Data::Class(id) = payload.data
            && self.heap.get(self.heap.get_class(id)?.prototype)?.is_module
        {
            return Ok(0);
        }
        let code = match payload.data {
            Data::Class(id) => {
                self.heap
                    .get(self.heap.get_class(id)?.prototype)?
                    .declaration_code
            }
            Data::TypeAlias(id) => self.heap.get(id)?.declaration_code,
            _ => return Err(invalid("type declaration payload kind")),
        };
        let declaration = code.and_then(|code| self.program.declaration_sources.get(&code));
        match declaration.map(|declaration| &declaration.recipe) {
            Some(crate::syntax::DeclarationRecipe::Class {
                type_parameters, ..
            })
            | Some(crate::syntax::DeclarationRecipe::TypeAlias {
                type_parameters, ..
            }) => Ok(type_parameters.len()),
            _ => self.builtin_mirror_parameter_count(&payload.data),
        }
    }
}
