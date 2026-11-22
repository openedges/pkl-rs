//! Declaration metadata and lazy type recipes. Original syntax is retained even
//! when the executable check expands an alias or erases a constraint.
use super::*;
use crate::syntax::{DeclarationRecipe, DeclarationSource, Type, TypeKind, TypeParameter};
use crate::value::RuntimeSource;
use crate::vm::classes::BuiltinClass;
use std::sync::OnceLock;

#[derive(Clone, Debug)]
pub(crate) struct Declaration {
    pub owner: ObjectId,
    pub code: Option<usize>,
    pub source: Arc<RuntimeSource>,
    pub syntax: Arc<DeclarationSource>,
}
#[derive(Clone, Debug)]
pub(crate) struct Context {
    pub owner: ObjectId,
    pub source: Arc<RuntimeSource>,
    pub parameters: Vec<TypeParameter>,
}
impl Declaration {
    fn parameters(&self) -> &[TypeParameter] {
        match &self.syntax.recipe {
            DeclarationRecipe::Class {
                type_parameters, ..
            }
            | DeclarationRecipe::Method {
                type_parameters, ..
            }
            | DeclarationRecipe::TypeAlias {
                type_parameters, ..
            } => type_parameters,
            _ => &[],
        }
    }
}
fn pinned_module(
    uri: impl Into<PklString>,
) -> Result<(&'static crate::syntax::Module, Arc<RuntimeSource>), RuntimeError> {
    static BASE: OnceLock<crate::syntax::Module> = OnceLock::new();
    static REF: OnceLock<crate::syntax::Module> = OnceLock::new();
    static REFLECT: OnceLock<crate::syntax::Module> = OnceLock::new();
    let uri = uri.into();
    let (cell, source) = match uri.as_str() {
        Some("pkl:base") => (&BASE, crate::stdlib_sources::base()),
        Some("pkl:ref") => (&REF, crate::stdlib_sources::reference()),
        Some("pkl:reflect") => (&REFLECT, crate::stdlib_sources::reflect()),
        _ => return Err(invalid("unknown builtin declaration source")),
    };
    Ok((
        cell.get_or_init(|| {
            crate::parser::parse_module(&source.text).expect("pinned metadata syntax")
        }),
        source,
    ))
}
fn unknown() -> Type {
    Type {
        kind: TypeKind::Unknown,
        span: Span::default(),
        parenthesized: false,
        unparenthesized_span: None,
    }
}
fn modifier_names(flags: u32, class: bool) -> Vec<PklString> {
    use crate::syntax::ModifierKind as M;
    [
        M::Abstract,
        M::Open,
        M::Hidden,
        M::External,
        M::Fixed,
        M::Const,
    ]
    .into_iter()
    .filter(|modifier| flags & modifier.flag() != 0 && (*modifier != M::External || class))
    .map(|modifier| modifier.spelling().into())
    .collect()
}
fn type_parameter(parameter: &TypeParameter) -> Payload {
    Payload::fields(
        Kind::TypeParameter,
        vec![
            ("name", Value::String(parameter.name.clone())),
            (
                "variance",
                match parameter.variance {
                    crate::syntax::Variance::In => Value::String("in".into()),
                    crate::syntax::Variance::Out => Value::String("out".into()),
                    crate::syntax::Variance::Invariant => Value::Null,
                },
            ),
        ],
    )
}
impl Run {
    fn sequence(&mut self, values: Vec<Value>) {
        let length = values.len();
        self.values.extend(values);
        self.tasks.push(Task::BuildList(length));
    }
    fn type_list(&mut self, types: Vec<Type>, context: &Context) {
        self.tasks.push(Task::BuildList(types.len()));
        self.tasks.extend(
            types
                .into_iter()
                .rev()
                .map(|ty| Task::Type(ty, context.clone())),
        );
    }
}
impl Vm {
    fn reflection_type_context(&self, declaration: &Declaration) -> Result<Context, RuntimeError> {
        let mut parameters = Vec::new();
        if matches!(
            declaration.syntax.recipe,
            DeclarationRecipe::Property { .. } | DeclarationRecipe::Method { .. }
        ) && !self.heap.get(declaration.owner)?.is_module
            && let Some(class) = self.heap.object_class(declaration.owner)?
        {
            parameters.extend(self.class_declaration(class)?.parameters().iter().cloned());
        }
        for parameter in declaration.parameters() {
            if let Some(index) = parameters
                .iter()
                .position(|previous| previous.name == parameter.name)
            {
                parameters[index] = parameter.clone();
            } else {
                parameters.push(parameter.clone());
            }
        }
        Ok(Context {
            owner: declaration.owner,
            source: declaration.source.clone(),
            parameters,
        })
    }

    fn declaration(&self, owner: ObjectId, code: usize) -> Result<Declaration, RuntimeError> {
        let syntax = self
            .program
            .declaration_source(code)
            .ok_or_else(|| invalid("missing original declaration recipe"))?
            .clone();
        let source = self
            .program
            .metadata
            .get(code)
            .and_then(|metadata| {
                metadata.source.clone().or_else(|| {
                    // compile_module can precede primary-source registration.
                    // Resolve only this declaration's compiling unit; detached
                    // code must never acquire an unrelated caller's source.
                    metadata
                        .unit
                        .and_then(|unit| self.program.units.get(unit.0 as usize))
                        .and_then(|unit| unit.source.clone())
                })
            })
            .ok_or_else(|| invalid("declaration source missing"))?;
        Ok(Declaration {
            owner,
            code: Some(code),
            source,
            syntax: Arc::new(syntax),
        })
    }
    fn class_declaration(&self, class: ObjectId) -> Result<Declaration, RuntimeError> {
        let prototype = self.heap.get_class(class)?.prototype;
        let object = self.heap.get(prototype)?;
        if let Some(code) = object.declaration_code {
            return self.declaration(prototype, code);
        }
        let class = self.heap.class_definition(class)?;
        let (parsed, source) = pinned_module(
            class
                .origin_uri
                .clone()
                .unwrap_or_else(|| "pkl:base".into()),
        )?;
        let member = parsed
            .members
            .iter()
            .find(|member| {
                member.name == class.name
                    && matches!(member.expr.kind, crate::syntax::ExprKind::Class { .. })
            })
            .ok_or_else(|| invalid("builtin class metadata not found"))?;
        Ok(Declaration {
            owner: prototype,
            code: None,
            source,
            syntax: Arc::new(member.declaration_source()),
        })
    }
    fn alias_declaration(&self, alias: ObjectId) -> Result<Declaration, RuntimeError> {
        let object = self.heap.get(alias)?;
        if let Some(code) = object.declaration_code {
            return self.declaration(alias, code);
        }
        let qualified = object
            .type_alias
            .as_ref()
            .ok_or_else(|| invalid("alias declaration name"))?;
        let qualified_text = crate::source::SourceText::from(qualified);
        let name = qualified_text
            .entire()
            .rsplit_once("#")
            .map_or(qualified.clone(), |(_, name)| name.to_pkl_string());
        let (parsed, source) = pinned_module("pkl:base")?;
        let member = parsed
            .members
            .iter()
            .find(|member| {
                member.name == name
                    && matches!(member.expr.kind, crate::syntax::ExprKind::TypeAlias { .. })
            })
            .ok_or_else(|| invalid("builtin alias metadata not found"))?;
        Ok(Declaration {
            owner: alias,
            code: None,
            source,
            syntax: Arc::new(member.declaration_source()),
        })
    }
    pub(super) fn builtin_mirror_parameter_count(
        &self,
        data: &Data,
    ) -> Result<usize, RuntimeError> {
        let declaration = match data {
            Data::Class(class) => self.class_declaration(*class)?,
            Data::TypeAlias(alias) => self.alias_declaration(*alias)?,
            _ => return Err(invalid("type declaration parameter metadata")),
        };
        Ok(declaration.parameters().len())
    }

    pub(super) fn reflection_declaration_task(
        &mut self,
        task: Task,
        run: &mut Run,
    ) -> Result<(), RuntimeError> {
        match task {
            Task::BuildMap(names) => {
                let start = run
                    .values
                    .len()
                    .checked_sub(names.len())
                    .ok_or_else(|| invalid("mirror map operands"))?;
                let values: Vec<_> = run.values.drain(start..).collect();
                let id = self.heap.allocate(Object {
                    collection_kind: CollectionKind::Map,
                    ..Default::default()
                });
                // The keys are authored String names, so there are no guest hash
                // callbacks. Use the same persistent map association/order path.
                for (name, value) in names.into_iter().zip(values) {
                    self.insert_map_entry_ordered(id, Value::String(name), value, None)?;
                }
                run.values
                    .push(Value::Object(self.finish_strict_collection(id)?));
            }
            Task::Type(mut ty, context) => {
                while let TypeKind::Constrained { base, .. } = ty.kind {
                    ty = *base;
                }
                let kind = match &ty.kind {
                    TypeKind::Unknown => Kind::UnknownType,
                    TypeKind::Nothing => Kind::NothingType,
                    TypeKind::Module => Kind::ModuleType,
                    TypeKind::Nullable(_) => Kind::NullableType,
                    TypeKind::Union { .. } => Kind::UnionType,
                    TypeKind::Function { .. } => Kind::FunctionType,
                    TypeKind::String(_) => Kind::StringLiteralType,
                    TypeKind::Named { path, .. }
                        if path.len() == 1
                            && context
                                .parameters
                                .iter()
                                .any(|parameter| parameter.name == path[0]) =>
                    {
                        Kind::TypeVariable
                    }
                    TypeKind::Named { .. } => Kind::DeclaredType,
                    TypeKind::Constrained { .. } => unreachable!("peeled constraints"),
                };
                if matches!(
                    kind,
                    Kind::UnknownType | Kind::NothingType | Kind::ModuleType
                ) {
                    run.make(kind, Vec::new());
                } else if let TypeKind::String(value) = &ty.kind {
                    run.make(kind, vec![("value", Value::String(value.clone()))]);
                } else {
                    run.tasks.push(Task::Make(Payload {
                        kind,
                        data: Data::Recipe { ty, context },
                    }));
                }
            }
            Task::ReadPath { rest } => {
                let receiver = run
                    .values
                    .pop()
                    .and_then(|value| value.heap_id())
                    .ok_or_else(|| invalid("type path receiver"))?;
                let Some((name, rest)) = rest.split_first() else {
                    return Err(invalid("empty type path"));
                };
                if !rest.is_empty() {
                    run.tasks.push(Task::ReadPath {
                        rest: rest.to_vec(),
                    });
                }
                run.tasks.push(Task::Read(receiver, name.clone()));
            }
            Task::ResolveName { ty, context } => {
                let object = self.heap.get(context.owner)?;
                if object.is_module {
                    self.reflection_resolve_name(context.owner, ty, context, run)?;
                } else if object.environment.scopes.is_empty()
                    && context.source.uri.starts_with("pkl:")
                {
                    // Synthetic builtin prototypes have no lexical frame. Their
                    // original defining singleton supplies the same module scope.
                    let uri = context.source.uri.clone();
                    run.tasks.push(Task::ResolveImportedScope { ty, context });
                    run.tasks.push(Task::ImportModule(uri));
                } else {
                    let module = self.declaring_module(&object.environment, false)?;
                    self.reflection_resolve_name(module, ty, context, run)?;
                }
            }
            Task::ResolveImportedScope { ty, context } => {
                let module = run
                    .values
                    .pop()
                    .ok_or_else(|| invalid("type lexical module missing"))?
                    .as_object()?;
                self.reflection_resolve_name(module, ty, context, run)?;
            }
            Task::ResolveSimpleBase { module, name, span } => {
                let base = run
                    .values
                    .pop()
                    .ok_or_else(|| invalid("base declaration module missing"))?
                    .as_object()?;
                if let Some(member) = self.declared_type_member(base, &name, false, true, span)? {
                    run.tasks.push(Task::ReadDeclared {
                        owner: base,
                        name,
                        local: member.local,
                    });
                } else {
                    self.reflection_resolve_parent_type(module, name, span, run)?;
                }
            }
            Task::ResolveQualifiedType {
                name,
                span,
                name_span,
            } => {
                let module = run
                    .values
                    .pop()
                    .ok_or_else(|| invalid("qualified import missing"))?
                    .as_object()?;
                let mut current = Some(module);
                let mut found = false;
                while let Some(owner) = current {
                    if let Some(member) =
                        self.declared_type_member(owner, &name, false, false, span)?
                    {
                        run.tasks.push(Task::ReadDeclared {
                            owner,
                            name: name.clone(),
                            local: member.local,
                        });
                        found = true;
                        break;
                    }
                    current = self.heap.get(owner)?.parent;
                }
                if !found {
                    let module_name = self.reflection_type_module_name(module)?;
                    return Err(RuntimeError::new(
                        ErrorKind::Type,
                        format!("Cannot find type `{name}` in module `{module_name}`."),
                    )
                    .at(name_span));
                }
            }
            Task::AsMirror(span) => {
                let value = run
                    .values
                    .pop()
                    .ok_or_else(|| invalid("type referent missing"))?;
                let payload = match value {
                    Value::Class(id) => Payload {
                        kind: Kind::Class,
                        data: Data::Class(id),
                    },
                    Value::TypeAlias(id) => Payload {
                        kind: Kind::TypeAlias,
                        data: Data::TypeAlias(id),
                    },
                    Value::Object(module) => Payload {
                        kind: Kind::Class,
                        data: Data::Class(self.declared_module_class(module, span)?),
                    },
                    _ => return Err(invalid("resolved type declaration kind")),
                };
                run.tasks.push(Task::Make(payload));
            }
            Task::AsModuleMirror => {
                let id = run
                    .values
                    .pop()
                    .and_then(|value| value.heap_id())
                    .ok_or_else(|| invalid("enclosing module result"))?;
                run.tasks.push(Task::Make(Payload {
                    kind: Kind::Module,
                    data: Data::Module(id),
                }));
            }
            Task::TypeArguments { ty, context } => {
                let TypeKind::Named { arguments, .. } = &ty.kind else {
                    return Err(invalid("declared type argument recipe"));
                };
                if arguments.is_empty() {
                    run.tasks.push(Task::DefaultArguments);
                    run.tasks.push(Task::AsMirror(ty.span));
                    run.tasks.push(Task::ResolveName { ty, context });
                } else {
                    // Java erases only the canonical Class<T> declaration to
                    // FinalClassTypeNode. Resolve the referent before inspecting
                    // argument syntax so erased arguments remain unevaluated.
                    run.tasks.push(Task::ExplicitTypeArguments {
                        arguments: arguments.clone(),
                        context: context.clone(),
                    });
                    run.tasks.push(Task::ResolveName { ty, context });
                }
            }
            Task::ExplicitTypeArguments { arguments, context } => {
                let referent = run
                    .values
                    .pop()
                    .ok_or_else(|| invalid("explicit type arguments referent"))?;
                let erased = if let Value::Class(class) = referent {
                    self.canonical_type_class(class)? == Some(BuiltinClass::Class)
                } else {
                    false
                };
                if erased {
                    run.tasks.push(Task::BuildList(1));
                    run.tasks
                        .push(Task::Make(Payload::fields(Kind::UnknownType, vec![])));
                } else {
                    run.type_list(arguments, &context);
                }
            }
            Task::DefaultArguments => {
                let referent = run
                    .values
                    .pop()
                    .ok_or_else(|| invalid("default arguments referent"))?;
                let count = self.mirror_parameter_count(&referent)?;
                run.tasks.push(Task::BuildList(count));
                for _ in 0..count {
                    run.tasks
                        .push(Task::Make(Payload::fields(Kind::UnknownType, vec![])));
                }
            }
            _ => return Err(invalid("unexpected reflection declaration task")),
        }
        Ok(())
    }

    fn reflection_type_module_name(&self, module: ObjectId) -> Result<PklString, RuntimeError> {
        Ok(self
            .heap
            .get(module)?
            .module_unit
            .and_then(|id| self.program.units.get(id.0 as usize))
            .map_or_else(
                || self.program.module_name.clone(),
                |unit| unit.name.clone(),
            ))
    }

    fn reflection_resolve_parent_type(
        &self,
        module: ObjectId,
        name: PklString,
        span: Span,
        run: &mut Run,
    ) -> Result<(), RuntimeError> {
        let mut current = self.heap.get(module)?.parent;
        while let Some(owner) = current {
            if let Some(member) = self.declared_type_member(owner, &name, false, true, span)? {
                run.tasks.push(Task::ReadDeclared {
                    owner,
                    name,
                    local: member.local,
                });
                return Ok(());
            }
            current = self.heap.get(owner)?.parent;
        }
        let mut error = RuntimeError::new(
            ErrorKind::MissingType,
            PklString::from("Cannot find type `")
                .concat(&name)
                .concat(&"`.".into()),
        )
        .at(span);
        error.type_lookup_module = Some(self.reflection_type_module_name(module)?);
        Err(error)
    }

    fn reflection_resolve_name(
        &self,
        module: ObjectId,
        ty: Type,
        context: Context,
        run: &mut Run,
    ) -> Result<(), RuntimeError> {
        let TypeKind::Named {
            path, name_spans, ..
        } = &ty.kind
        else {
            return Err(invalid("non-named declaration lookup"));
        };
        let name: PklString = path
            .first()
            .ok_or_else(|| invalid("empty declared type path"))?
            .clone();
        if path.len() == 1 {
            // ResolveSimpleDeclaredTypeNode: local declaration/import, own
            // regular declaration, canonical base, then parent modules.
            let local = self.declared_type_member(module, &name, true, true, ty.span)?;
            let member = if local.is_some() {
                local
            } else {
                self.declared_type_member(module, &name, false, true, ty.span)?
            };
            if let Some(member) = member {
                run.tasks.push(Task::ReadDeclared {
                    owner: module,
                    name,
                    local: member.local,
                });
            } else if context.source.uri == "pkl:base" {
                self.reflection_resolve_parent_type(module, name, ty.span, run)?;
            } else {
                run.tasks.push(Task::ResolveSimpleBase {
                    module,
                    name,
                    span: ty.span,
                });
                run.tasks.push(Task::ImportModule("pkl:base".into()));
            }
            return Ok(());
        }
        if path.len() != 2 || name_spans.len() != 2 {
            return Err(invalid("qualified declared type shape"));
        }
        let member = self.declared_type_import(module, &name, name_spans[0])?;
        run.tasks.push(Task::ResolveQualifiedType {
            name: path[1].clone(),
            span: Span::new(name_spans[0].start, name_spans[1].end),
            name_span: name_spans[1],
        });
        run.tasks.push(Task::ReadDeclared {
            owner: module,
            name,
            local: member.local,
        });
        Ok(())
    }

    fn reflection_recipe_field(
        &mut self,
        ty: Type,
        context: Context,
        name: &str,
        run: &mut Run,
    ) -> Result<(), RuntimeError> {
        match (&ty.kind, name) {
            (TypeKind::Named { path, .. }, "referent")
                if path.len() == 1 && context.parameters.iter().any(|p| p.name == path[0]) =>
            {
                let parameter = context
                    .parameters
                    .iter()
                    .find(|p| p.name == path[0])
                    .unwrap();
                run.tasks.push(Task::Make(type_parameter(parameter)));
            }
            (TypeKind::Named { .. }, "referent") => {
                run.tasks.push(Task::AsMirror(ty.span));
                run.tasks.push(Task::ResolveName {
                    ty: ty.clone(),
                    context,
                });
            }
            (TypeKind::Named { .. }, "typeArguments") => {
                run.tasks.push(Task::TypeArguments { ty, context })
            }
            (TypeKind::String(value), "value") => run.values.push(Value::String(value.clone())),
            (TypeKind::Nullable(member), "member") => {
                run.tasks.push(Task::Type((**member).clone(), context))
            }
            (TypeKind::Union { types, .. }, "members") => run.type_list(types.clone(), &context),
            (TypeKind::Function { parameters, .. }, "parameterTypes") => {
                run.type_list(parameters.clone(), &context)
            }
            (TypeKind::Function { return_type, .. }, "returnType") => {
                run.tasks.push(Task::Type((**return_type).clone(), context))
            }
            _ => return Err(invalid("type recipe field")),
        }
        Ok(())
    }
}

impl Vm {
    pub(super) fn reflection_field(
        &mut self,
        payload: Payload,
        name: &PklString,
        run: &mut Run,
    ) -> Result<(), RuntimeError> {
        let name = name
            .as_str()
            .ok_or_else(|| invalid("non-scalar builtin mirror field"))?;
        match payload.data {
            Data::Fields(fields) => {
                run.values.push(
                    fields
                        .into_iter()
                        .find(|(field, _)| *field == name)
                        .map(|(_, value)| value)
                        .ok_or_else(|| invalid("missing mirror factory field"))?,
                );
            }
            Data::Recipe { ty, context } => self.reflection_recipe_field(ty, context, name, run)?,
            Data::Location(frame) => {
                let source = frame
                    .source
                    .as_ref()
                    .ok_or_else(|| invalid("location source"))?;
                match name {
                    "line" => {
                        let position = source
                            .text
                            .index()
                            .position(frame.span.start)
                            .ok_or_else(|| invalid("location start"))?;
                        run.values.push(Value::Int((position.line as i64).into()));
                    }
                    "column" => {
                        let position = source
                            .text
                            .index()
                            .position(frame.span.start)
                            .ok_or_else(|| invalid("location start"))?;
                        run.values
                            .push(Value::Int((position.column_utf16 as i64).into()));
                    }
                    "displayUri" => {
                        if source.text.index().position(frame.span.start).is_none() {
                            return Err(invalid("location start"));
                        }
                        if let Some(uri) = self.reflection_display_uri(&frame)? {
                            run.values.push(Value::String(uri));
                        } else {
                            run.waiting = true;
                        }
                    }
                    _ => return Err(invalid("source location field")),
                }
            }
            Data::Module(module) => self.reflection_module_field(module, name, run)?,
            Data::Class(class) => self.reflection_class_field(class, name, run)?,
            Data::TypeAlias(alias) => {
                let declaration = self.alias_declaration(alias)?;
                match name {
                    "reflectee" => run.values.push(Value::TypeAlias(alias)),
                    "referent" => {
                        let DeclarationRecipe::TypeAlias { referent, .. } =
                            &declaration.syntax.recipe
                        else {
                            return Err(invalid("alias recipe kind"));
                        };
                        run.tasks.push(Task::Type(
                            referent.clone(),
                            self.reflection_type_context(&declaration)?,
                        ));
                    }
                    "enclosingDeclaration" => self.reflection_enclosing_module(
                        declaration.owner,
                        &declaration.source,
                        run,
                    )?,
                    _ => self.reflection_declaration_field(&declaration, name, false, run)?,
                }
            }
            Data::Member(declaration) => match name {
                "type" => {
                    let DeclarationRecipe::Property { ty, .. } = &declaration.syntax.recipe else {
                        return Err(invalid("property type recipe"));
                    };
                    run.tasks.push(Task::Type(
                        ty.clone().unwrap_or_else(unknown),
                        self.reflection_type_context(&declaration)?,
                    ));
                }
                "defaultValue" => self.reflection_property_default(&declaration, run)?,
                "parameters" => {
                    let DeclarationRecipe::Method { parameters, .. } = &declaration.syntax.recipe
                    else {
                        return Err(invalid("method parameters recipe"));
                    };
                    run.tasks.push(Task::BuildMap(
                        parameters
                            .iter()
                            .map(|parameter| parameter.name.clone())
                            .collect(),
                    ));
                    for parameter in parameters.iter().rev() {
                        run.tasks.push(Task::BuildFields(
                            Kind::MethodParameter,
                            vec!["name", "type"],
                        ));
                        run.tasks.push(Task::Type(
                            parameter.ty.clone().unwrap_or_else(unknown),
                            self.reflection_type_context(&declaration)?,
                        ));
                        run.tasks
                            .push(Task::Value(Value::String(parameter.name.clone())));
                    }
                }
                "returnType" => {
                    let DeclarationRecipe::Method { return_type, .. } = &declaration.syntax.recipe
                    else {
                        return Err(invalid("method return type recipe"));
                    };
                    run.tasks.push(Task::Type(
                        return_type.clone().unwrap_or_else(unknown),
                        self.reflection_type_context(&declaration)?,
                    ));
                }
                "allAnnotations" => {
                    let declarations = self.reflection_property_chain(&declaration)?;
                    run.tasks.push(Task::ConcatLists(declarations.len()));
                    run.tasks
                        .extend(declarations.into_iter().rev().map(Task::Annotations));
                }
                "allModifiers" => {
                    let flags = self.reflection_property_chain(&declaration)?.iter().fold(
                        0,
                        |flags, declaration| {
                            flags | crate::syntax::modifier_flags(&declaration.syntax.modifiers)
                        },
                    );
                    run.values.push(self.reflection_modifier_set(flags, false)?);
                }
                _ => self.reflection_declaration_field(&declaration, name, false, run)?,
            },
        }
        Ok(())
    }

    fn reflection_declaration_field(
        &mut self,
        declaration: &Declaration,
        name: &str,
        class: bool,
        run: &mut Run,
    ) -> Result<(), RuntimeError> {
        match name {
            "name" => run
                .values
                .push(Value::String(declaration.syntax.name.clone())),
            "docComment" => run.values.push(
                declaration
                    .syntax
                    .doc_comment
                    .as_ref()
                    .and_then(|comment| comment.text(&declaration.source.text))
                    .map_or(Value::Null, Value::String),
            ),
            "location" => run.tasks.push(Task::Make(Payload {
                kind: Kind::SourceLocation,
                data: Data::Location(RuntimeFrame {
                    anonymous: false,
                    source: Some(declaration.source.clone()),
                    span: declaration.syntax.header_span,
                    member: PklString::default(),
                }),
            })),
            "annotations" => run.tasks.push(Task::Annotations(declaration.clone())),
            "modifiers" => run.values.push(self.reflection_modifier_set(
                crate::syntax::modifier_flags(&declaration.syntax.modifiers),
                class,
            )?),
            "typeParameters" => {
                run.tasks
                    .push(Task::BuildList(declaration.parameters().len()));
                run.tasks.extend(
                    declaration
                        .parameters()
                        .iter()
                        .rev()
                        .map(|parameter| Task::Make(type_parameter(parameter))),
                );
            }
            _ => return Err(invalid("declaration mirror field")),
        }
        Ok(())
    }

    pub(super) fn reflection_existing_annotations(
        &self,
        declaration: &Declaration,
    ) -> Result<Vec<Value>, RuntimeError> {
        let object = self.heap.get(declaration.owner)?;
        if let Some(code) = declaration.code {
            if matches!(
                declaration.syntax.recipe,
                DeclarationRecipe::Class { .. } | DeclarationRecipe::TypeAlias { .. }
            ) {
                Ok(object.annotations.clone())
            } else {
                Ok(object
                    .member_annotations
                    .get(&code)
                    .cloned()
                    .unwrap_or_default())
            }
        } else {
            Ok(object.annotations.clone())
        }
    }

    fn reflection_modifier_set(&mut self, flags: u32, class: bool) -> Result<Value, RuntimeError> {
        let id = self.heap.allocate(Object {
            collection_kind: CollectionKind::Set,
            ..Default::default()
        });
        self.frames.last_mut().unwrap().builders.push(id);
        for name in modifier_names(flags, class) {
            self.stack.extend([Value::String(name), Value::Bool(false)]);
            // Every modifier is a distinct immutable String. Shared Set put
            // cannot suspend for these keys or invoke a guest equality body.
            self.collection_instruction(Instruction::AddSetElement)?;
        }
        self.frames.last_mut().unwrap().builders.pop();
        Ok(Value::Object(self.finish_strict_collection(id)?))
    }

    fn reflection_module_field(
        &mut self,
        module: ObjectId,
        name: &str,
        run: &mut Run,
    ) -> Result<(), RuntimeError> {
        let object = self.heap.get(module)?;
        let unit = object
            .module_unit
            .and_then(|unit| self.program.units.get(unit.0 as usize))
            .ok_or_else(|| invalid("module mirror unit"))?
            .clone();
        let source = unit
            .source
            .clone()
            .ok_or_else(|| invalid("module mirror source"))?;
        let syntax = unit
            .syntax
            .as_deref()
            .ok_or_else(|| invalid("module mirror syntax"))?;
        match name {
            "reflectee" => run.values.push(Value::Object(module)),
            "moduleClass" => {
                let class = self
                    .heap
                    .object_class(module)?
                    .ok_or_else(|| invalid("module class missing"))?;
                run.tasks.push(Task::Make(Payload {
                    kind: Kind::Class,
                    data: Data::Class(class),
                }));
            }
            "name" => run.values.push(Value::String(unit.name.clone())),
            "uri" => run.values.push(Value::String(
                unit.identity
                    .as_ref()
                    .map_or(&source.uri, |identity| &identity.original_uri)
                    .into(),
            )),
            "docComment" => run.values.push(
                syntax
                    .declaration
                    .as_ref()
                    .and_then(|declaration| declaration.doc_comment.as_ref())
                    .and_then(|doc| doc.text(&source.text))
                    .map_or(Value::Null, Value::String),
            ),
            "location" => run.tasks.push(Task::Make(Payload {
                kind: Kind::SourceLocation,
                data: Data::Location(RuntimeFrame {
                    anonymous: false,
                    source: Some(source),
                    span: syntax
                        .declaration
                        .as_ref()
                        .map_or(Span::default(), |declaration| declaration.span),
                    member: PklString::default(),
                }),
            })),
            "annotations" => run.sequence(object.annotations.clone()),
            "modifiers" => run.values.push(self.reflection_modifier_set(
                if unit.mode == modules::ModuleMode::Amends {
                    0
                } else {
                    unit.modifiers
                },
                true,
            )?),
            "isAmend" => run
                .values
                .push(Value::Bool(unit.mode == modules::ModuleMode::Amends)),
            "supermodule" => {
                if let Some(parent) = object
                    .parent
                    .filter(|parent| self.heap.get(*parent).is_ok_and(|object| object.is_module))
                {
                    run.tasks.push(Task::Make(Payload {
                        kind: Kind::Module,
                        data: Data::Module(parent),
                    }));
                } else {
                    run.values.push(Value::Null);
                }
            }
            "imports" => {
                let mut names = Vec::new();
                for member in &syntax.members {
                    if let crate::syntax::ExprKind::Import {
                        uri,
                        clause_alias: Some(_),
                        ..
                    } = &member.expr.kind
                    {
                        let _ = uri; // The parsed URI is authored, not the prepared ImportNode URI.
                        let imported = object
                            .members
                            .iter()
                            .find(|compiled| compiled.name == member.name && compiled.import())
                            .and_then(|compiled| self.program.blocks.get(compiled.code))
                            .and_then(|block| {
                                block.iter().find_map(|instruction| match instruction {
                                    Instruction::ImportModule(spec) => {
                                        Some(spec.absolute_uri.clone())
                                    }
                                    _ => None,
                                })
                            })
                            .ok_or_else(|| invalid("prepared reflection import URI"))?;
                        names.push(member.name.clone());
                        run.values.push(Value::String(imported.clone()));
                    }
                }
                run.tasks.push(Task::BuildMap(names));
            }
            "classes" | "typeAliases" => {
                let members: Vec<_> = if unit.mode == modules::ModuleMode::Amends {
                    Vec::new()
                } else {
                    syntax
                        .members
                        .iter()
                        .filter(|member| {
                            !member.local
                                && if name == "classes" {
                                    matches!(
                                        member.expr.kind,
                                        crate::syntax::ExprKind::Class { .. }
                                    )
                                } else {
                                    matches!(
                                        member.expr.kind,
                                        crate::syntax::ExprKind::TypeAlias { .. }
                                    )
                                }
                        })
                        .collect()
                };
                run.tasks.push(Task::BuildMap(
                    members.iter().map(|member| member.name.clone()).collect(),
                ));
                for member in members.into_iter().rev() {
                    run.tasks.push(Task::AsMirror(member.span));
                    run.tasks.push(Task::Read(module, member.name.clone()));
                }
            }
            _ => return Err(invalid("module mirror field")),
        }
        Ok(())
    }

    fn reflection_class_field(
        &mut self,
        class: ObjectId,
        name: &str,
        run: &mut Run,
    ) -> Result<(), RuntimeError> {
        let prototype = self.heap.get_class(class)?.prototype;
        if self.heap.get(prototype)?.is_module
            && matches!(
                name,
                "location" | "docComment" | "annotations" | "modifiers"
            )
        {
            return self.reflection_module_field(prototype, name, run);
        }
        match name {
            "reflectee" => run.values.push(Value::Class(class)),
            "name" => run
                .values
                .push(Value::String(if self.heap.get(prototype)?.is_module {
                    "ModuleClass".into()
                } else {
                    self.heap.class_definition(class)?.name.clone()
                })),
            "superclass" => {
                if let Some(parent) = self.heap.get_class(class)?.superclass {
                    run.tasks.push(Task::Make(Payload {
                        kind: Kind::Class,
                        data: Data::Class(parent),
                    }));
                } else {
                    run.values.push(Value::Null);
                }
            }
            "supertype" => {
                if let Some(parent) = self.heap.get_class(class)?.superclass {
                    let declaration = if self.heap.get(prototype)?.is_module {
                        None
                    } else {
                        Some(self.class_declaration(class)?)
                    };
                    if let Some(declaration) = declaration
                        && let DeclarationRecipe::Class {
                            superclass: Some(ty),
                            ..
                        } = &declaration.syntax.recipe
                    {
                        run.tasks.push(Task::Type(
                            ty.clone(),
                            self.reflection_type_context(&declaration)?,
                        ));
                    } else {
                        run.tasks.push(Task::DefaultDeclaredType);
                        run.tasks.push(Task::Make(Payload {
                            kind: Kind::Class,
                            data: Data::Class(parent),
                        }));
                    }
                } else {
                    run.values.push(Value::Null);
                }
            }
            "properties" | "allProperties" | "methods" | "allMethods" => {
                let methods = matches!(name, "methods" | "allMethods");
                let all = name.starts_with("all");
                let declarations = self.reflection_class_members(class, methods, all)?;
                run.tasks.push(Task::BuildMap(
                    declarations
                        .iter()
                        .map(|declaration| declaration.syntax.name.clone())
                        .collect(),
                ));
                for declaration in declarations.into_iter().rev() {
                    run.tasks.push(Task::Make(Payload {
                        kind: if methods {
                            Kind::Method
                        } else {
                            Kind::Property
                        },
                        data: Data::Member(declaration),
                    }));
                }
            }
            "enclosingDeclaration" if self.heap.get(prototype)?.is_module => {
                run.tasks.push(Task::Make(Payload {
                    kind: Kind::Module,
                    data: Data::Module(prototype),
                }))
            }
            "enclosingDeclaration" => {
                let declaration = self.class_declaration(class)?;
                self.reflection_enclosing_module(prototype, &declaration.source, run)?;
            }
            "typeParameters" if self.heap.get(prototype)?.is_module => run.sequence(vec![]),
            _ => {
                let declaration = self.class_declaration(class)?;
                self.reflection_declaration_field(&declaration, name, true, run)?;
            }
        }
        Ok(())
    }

    fn reflection_enclosing_module(
        &self,
        owner: ObjectId,
        source: &RuntimeSource,
        run: &mut Run,
    ) -> Result<(), RuntimeError> {
        let object = self.heap.get(owner)?;
        let module = std::iter::once(owner)
            .chain(object.environment.scopes.iter().map(|scope| scope.owner))
            .find(|id| self.heap.get(*id).is_ok_and(|object| object.is_module));
        if let Some(module) = module {
            run.tasks.push(Task::Make(Payload {
                kind: Kind::Module,
                data: Data::Module(module),
            }));
        } else {
            run.tasks.push(Task::AsModuleMirror);
            run.tasks.push(Task::ImportModule(source.uri.clone()));
        }
        Ok(())
    }

    fn reflection_class_members(
        &self,
        class: ObjectId,
        methods: bool,
        inherited: bool,
    ) -> Result<Vec<Declaration>, RuntimeError> {
        let mut chain = vec![class];
        if inherited {
            while let Some(parent) = self.heap.get_class(*chain.last().unwrap())?.superclass {
                chain.push(parent);
            }
            chain.reverse();
        }
        let mut result: Vec<Declaration> = Vec::new();
        for class in chain {
            let owner = self.heap.get_class(class)?.prototype;
            let object = self.heap.get(owner)?;
            let declarations = if object.builtin_type.is_some() {
                let class = self.heap.class_definition(class)?;
                let (parsed, source) = pinned_module(
                    class
                        .origin_uri
                        .clone()
                        .unwrap_or_else(|| "pkl:base".into()),
                )?;
                let member = parsed
                    .members
                    .iter()
                    .find(|member| {
                        member.name == class.name
                            && matches!(member.expr.kind, crate::syntax::ExprKind::Class { .. })
                    })
                    .ok_or_else(|| invalid("builtin class members"))?;
                let crate::syntax::ExprKind::Class { members, .. } = &member.expr.kind else {
                    unreachable!()
                };
                members
                    .iter()
                    .filter(|member| {
                        !member.local
                            && !matches!(
                                member.expr.kind,
                                crate::syntax::ExprKind::Class { .. }
                                    | crate::syntax::ExprKind::TypeAlias { .. }
                                    | crate::syntax::ExprKind::Import { .. }
                            )
                            && matches!(
                                member.expr.kind,
                                crate::syntax::ExprKind::Function { method: true, .. }
                            ) == methods
                    })
                    .map(|member| Declaration {
                        owner,
                        code: object
                            .members
                            .iter()
                            .find(|compiled| {
                                compiled.name == member.name && compiled.method == methods
                            })
                            .map(|member| member.code),
                        source: source.clone(),
                        syntax: Arc::new(member.declaration_source()),
                    })
                    .collect::<Vec<_>>()
            } else {
                object
                    .members
                    .iter()
                    .filter(|member| {
                        !member.local
                            && member.method == methods
                            && !member.import()
                            && !member.definition()
                    })
                    .map(|member| self.declaration(owner, member.code))
                    .collect::<Result<Vec<_>, _>>()?
            };
            for declaration in declarations {
                if let Some(index) = result
                    .iter()
                    .position(|previous| previous.syntax.name == declaration.syntax.name)
                {
                    // Untyped property amendment changes the prototype's value,
                    // but retains the inherited class-property declaration.
                    if methods
                        || !matches!(
                            declaration.syntax.recipe,
                            DeclarationRecipe::Property { ty: None, .. }
                        )
                    {
                        result[index] = declaration;
                    }
                } else {
                    result.push(declaration);
                }
            }
        }
        Ok(result)
    }

    fn reflection_property_chain(
        &self,
        declaration: &Declaration,
    ) -> Result<Vec<Declaration>, RuntimeError> {
        let mut result = Vec::new();
        let mut class = self.heap.object_class(declaration.owner)?;
        while let Some(current) = class {
            if let Some(member) = self
                .reflection_class_members(current, false, false)?
                .into_iter()
                .find(|member| member.syntax.name == declaration.syntax.name)
            {
                result.push(member);
            }
            class = self.heap.get_class(current)?.superclass;
        }
        Ok(result)
    }
}

impl Vm {
    fn reflection_property_default(
        &self,
        declaration: &Declaration,
        run: &mut Run,
    ) -> Result<(), RuntimeError> {
        let flags = crate::syntax::modifier_flags(&declaration.syntax.modifiers);
        use crate::syntax::ModifierKind as M;
        if flags & (M::Abstract.flag() | M::External.flag()) != 0 {
            run.values.push(Value::Null);
            return Ok(());
        }
        let DeclarationRecipe::Property { undefined, .. } = declaration.syntax.recipe else {
            return Err(invalid("property default recipe"));
        };
        if undefined {
            run.tasks.push(Task::ProbeDefault(declaration.clone()));
        } else {
            run.tasks.push(Task::Read(
                declaration.owner,
                declaration.syntax.name.clone(),
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod source_provenance_tests {
    use super::*;

    #[test]
    fn declaration_source_uses_its_registered_unit_without_borrowing_caller_provenance() {
        let text = "class User { value: String }\n";
        let parsed = crate::parser::parse_module(text).unwrap();
        let program = compile_module(&crate::resolver::resolve_module(&parsed).unwrap()).unwrap();
        let code = *program.declaration_sources.keys().next().unwrap();
        assert!(program.metadata[code].source.is_none());
        assert_eq!(program.metadata[code].unit, Some(crate::modules::UnitId(0)));
        let primary = Arc::new(RuntimeSource {
            text: text.into(),
            uri: "file:///registered-primary.pkl".into(),
            module_name: "primary".into(),
        });
        let mut vm = Vm::with_context(
            program,
            EvaluationContext {
                primary_source: Some(primary.clone()),
                ..Default::default()
            },
        );
        let owner = vm.heap.allocate(Object::default());
        // Exercise the public compile_module + Vm::with_context path unchanged.
        assert!(Arc::ptr_eq(
            &vm.declaration(owner, code).unwrap().source,
            &primary
        ));
        let authored = Arc::new(RuntimeSource {
            text: text.into(),
            uri: "file:///explicit-declaration.pkl".into(),
            module_name: "authored".into(),
        });
        Arc::make_mut(&mut vm.program).metadata[code].source = Some(authored.clone());
        assert!(Arc::ptr_eq(
            &vm.declaration(owner, code).unwrap().source,
            &authored
        ));
        // Explicitly detached/replaced code cannot inherit the active primary.
        let metadata = &mut Arc::make_mut(&mut vm.program).metadata[code];
        metadata.source = None;
        metadata.unit = None;
        assert_eq!(
            vm.declaration(owner, code).unwrap_err().message,
            "declaration source missing"
        );
        Arc::make_mut(&mut vm.program).metadata[code].unit = Some(crate::modules::UnitId(0));
        Arc::make_mut(&mut vm.program).units[0].source = None;
        assert_eq!(
            vm.declaration(owner, code).unwrap_err().message,
            "declaration source missing"
        );
        vm.close();
        assert_eq!(vm.heap_stats().slots, 0);
    }
}
