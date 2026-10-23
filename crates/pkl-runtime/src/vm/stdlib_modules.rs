//! Evaluator-owned standard-library modules share ordinary module/class/member
//! identities and the same code arena as user declarations. External bindings
//! retain their pinned declaration source; missing bindings fail explicitly.
use super::*;
use crate::modules::{ModuleIdentity, UnitId};
use crate::syntax::ExprKind;

impl Vm {
    pub(super) fn import_stdlib(&mut self, uri: &str) -> Result<bool, RuntimeError> {
        let source = match uri {
            "pkl:base" => crate::stdlib_sources::base(),
            "pkl:test" => crate::stdlib_sources::test(),
            "pkl:ref" => crate::stdlib_sources::reference(),
            _ if uri.starts_with("pkl:") => {
                if include_str!("../stdlib/modules.txt")
                    .lines()
                    .any(|name| name == uri)
                {
                    return Err(RuntimeError::new(
                        ErrorKind::Unsupported,
                        format!("Standard-library module `{uri}` is not implemented."),
                    ));
                }
                let mut error = RuntimeError::new(
                    ErrorKind::Type,
                    format!("Cannot find standard library module `{uri}`."),
                );
                error.hint = Some(
                    format!(
                        "Available standard library modules:\n{}",
                        include_str!("../stdlib/modules.txt")
                            .strip_suffix('\n')
                            .unwrap()
                    )
                    .into(),
                );
                return Err(error);
            }
            _ => return Ok(false),
        };
        if let Some(cached) = self.modules_original.get(uri).cloned() {
            let value = self.module_cached(cached)?;
            self.stack.push(value);
            return Ok(true);
        }
        let program = Arc::make_mut(&mut self.program);
        let unit = append_definition(program, uri, source)?;
        let entry = program.units[unit.0 as usize].entry;
        // The ordinary module initializer publishes its receiver before forcing
        // pinned metadata and supported declarations, just like loaded modules.
        self.push_module_initializer(entry, None);
        Ok(true)
    }

    pub(super) fn in_stdlib_bootstrap(&self) -> bool {
        self.frames.iter().any(|frame| {
            self.program
                .metadata
                .get(frame.block)
                .is_some_and(|metadata| metadata.stdlib_bootstrap)
        })
    }

    pub(super) fn bootstrap_stdlib(&mut self) -> Result<(), RuntimeError> {
        let root = self.pop()?;
        self.force_stdlib_frame(root)
    }
}

fn append_definition(
    program: &mut Program,
    uri: &str,
    source: Arc<crate::value::RuntimeSource>,
) -> Result<UnitId, RuntimeError> {
    if let Some(unit) = program.units.iter().find(|unit| {
        unit.identity
            .as_ref()
            .is_some_and(|identity| identity.original_uri.as_ref() == uri)
    }) {
        return Ok(unit.id);
    }
    let parsed = crate::parser::parse_module(&source.text).map_err(|error| {
        RuntimeError::new(
            ErrorKind::Unsupported,
            PklString::from("Standard-library declaration parsing: ").concat(&error.message),
        )
    })?;
    let checkpoint = modules::ArenaCheckpoint::new(program);
    let result = append_members(program, uri, source, &parsed);
    if result.is_err() {
        checkpoint.rollback(program);
    }
    result
}

fn append_members(
    program: &mut Program,
    uri: &str,
    source: Arc<crate::value::RuntimeSource>,
    parsed: &crate::syntax::Module,
) -> Result<UnitId, RuntimeError> {
    // Compile the real authored annotation/initialization path. External member
    // bodies are bound below; they are not rewritten into Pkl source strings.
    let mut header = parsed.clone();
    header.members.clear();
    let resolved = crate::resolver::resolve_module(&header)
        .map_err(|error| RuntimeError::new(ErrorKind::Unsupported, error.message))?;
    let unit = compile_unit_into(
        program,
        &resolved,
        CompileOptions {
            fuse_constant_add: true,
        },
        false,
        Some(source.clone()),
        Some(ModuleIdentity {
            original_uri: uri.into(),
            resolved_uri: uri.into(),
        }),
    )?;
    let mut members = Vec::new();
    for declaration in &parsed.members {
        let method = matches!(
            declaration.expr.kind,
            ExprKind::Function { method: true, .. }
        );
        let mut ty = None;
        let mut type_span = None;
        let mut body = if uri == "pkl:base" {
            base_binding(declaration)
        } else if uri == "pkl:ref" {
            match declaration.name.as_str() {
                "Reference" if !method => vec![Instruction::BuiltinClass(TypeCheck::Reference {
                    domain: Box::new(TypeCheck::Any),
                    referent: Box::new(TypeCheck::Any),
                })],
                "Domain" if !method => vec![Instruction::BuiltinClass(TypeCheck::RefDomain)],
                "Access" if !method => vec![Instruction::BuiltinClass(TypeCheck::Class {
                    module: None,
                    id: reference::ACCESS_ID,
                    name: "Access".into(),
                })],
                _ => unsupported_binding(uri, declaration),
            }
        } else {
            match declaration.name.as_str() {
                "facts" | "examples" if !method => {
                    let value_type = if declaration.name == "facts" {
                        TypeCheck::Boolean
                    } else {
                        TypeCheck::Unknown
                    };
                    let listing = TypeCheck::ParameterizedCollection {
                        kind: crate::heap::CollectionKind::Listing,
                        arguments: vec![(value_type, declaration.ty.as_ref().unwrap().span)],
                    };
                    let mapping = TypeCheck::ParameterizedCollection {
                        kind: crate::heap::CollectionKind::Mapping,
                        arguments: vec![
                            (TypeCheck::String, declaration.ty.as_ref().unwrap().span),
                            (listing, declaration.ty.as_ref().unwrap().span),
                        ],
                    };
                    let check = TypeCheck::Nullable(Box::new(mapping.clone()));
                    ty = Some(check.clone());
                    type_span = declaration.ty.as_ref().map(|ty| ty.span);
                    // A nullable Mapping retains its collection default for amendments.
                    vec![Instruction::ResolvedDefault {
                        ty: check,
                        origin: Some(RuntimeFrame {
                            source: Some(source.clone()),
                            span: type_span.unwrap(),
                            member: format!("#{}", declaration.name).into(),
                        }),
                    }]
                }
                "catch" | "catchOrNull" if method => {
                    let ExprKind::Function {
                        parameters,
                        header_span,
                        ..
                    } = &declaration.expr.kind
                    else {
                        unreachable!()
                    };
                    let argument_span = parameters[0]
                        .ty
                        .as_ref()
                        .expect("pinned catch argument")
                        .span;
                    let block = program.blocks.len();
                    program.blocks.push(vec![
                        Instruction::LoadLocal(0),
                        Instruction::CheckType(TypeCheck::Function {
                            parameters: Some(Vec::new()),
                            result: Box::new(TypeCheck::Any),
                            syntax: true,
                        }),
                        Instruction::GuestCatch {
                            or_null: declaration.name == "catchOrNull",
                        },
                        Instruction::Return,
                    ]);
                    program.metadata.push(BlockMetadata {
                        unit: Some(unit),
                        source: Some(source.clone()),
                        member: format!("#{}", declaration.name).into(),
                        declaration: Some(*header_span),
                        spans: vec![None, Some(argument_span), None, None],
                        ..Default::default()
                    });
                    vec![Instruction::MakeFunction {
                        block,
                        arity: 1,
                        method: true,
                        capture_custom_this: false,
                    }]
                }
                _ => unsupported_binding(uri, declaration),
            }
        };
        body.push(Instruction::Return);
        let block = program.blocks.len();
        let spans = vec![Some(declaration.name_span); body.len()];
        program.blocks.push(body);
        program.metadata.push(BlockMetadata {
            unit: Some(unit),
            source: Some(source.clone()),
            member: format!("#{}", declaration.name).into(),
            declaration: Some(declaration.name_span),
            spans,
            ..Default::default()
        });
        members.push(Member {
            flags: Member::declaration_flags(declaration),
            entry: false,
            name: declaration.name.clone().into(),
            code: block,
            local: declaration.local,
            method,
            ty,
            type_span,
            constant: declaration
                .modifiers
                .iter()
                .any(|modifier| modifier.kind == crate::syntax::ModifierKind::Const)
                || matches!(
                    declaration.expr.kind,
                    ExprKind::Class { .. } | ExprKind::TypeAlias { .. }
                ),
            fixed: false,
        });
    }
    let definition = &mut program.units[unit.0 as usize];
    program.objects[definition.template] = members.clone();
    definition.initial_members = members;
    let entry = definition.entry;
    let finish = program.blocks[entry]
        .iter()
        .position(|instruction| matches!(instruction, Instruction::FinishModule))
        .expect("ordinary module initializer");
    let mut bootstrap = vec![
        Instruction::Duplicate,
        Instruction::BootstrapStdlib,
        Instruction::Pop,
    ];
    if uri == "pkl:base" {
        bootstrap.extend([
            Instruction::Duplicate,
            Instruction::GetMember("output".into()),
            Instruction::BootstrapStdlib,
            Instruction::Pop,
        ]);
    }
    program.metadata[entry].stdlib_bootstrap = true;
    program.metadata[entry]
        .spans
        .splice(finish..finish, vec![None; bootstrap.len()]);
    program.blocks[entry].splice(finish..finish, bootstrap);
    Ok(unit)
}

fn base_binding(declaration: &crate::syntax::Member) -> Vec<Instruction> {
    let name = declaration.name.as_str();
    match &declaration.expr.kind {
        ExprKind::Class { .. } => {
            if let Some(ty) =
                reference::base_type(name).or_else(|| builtins::id(name).map(builtins::ty))
            {
                return vec![Instruction::BuiltinClass(ty)];
            }
        }
        ExprKind::TypeAlias { .. }
            if matches!(
                name,
                "NonNull" | "Int8" | "Int16" | "Int32" | "UInt8" | "UInt16" | "UInt32" | "UInt"
            ) =>
        {
            return vec![Instruction::BuiltinTypeAlias(name.into())];
        }
        _ if matches!(name, "NaN" | "Infinity") => {
            return vec![Instruction::Constant(Value::Float(if name == "NaN" {
                f64::NAN
            } else {
                f64::INFINITY
            }))];
        }
        _ => {}
    }
    unsupported_binding("pkl:base", declaration)
}

fn unsupported_binding(uri: &str, declaration: &crate::syntax::Member) -> Vec<Instruction> {
    vec![Instruction::Fail(RuntimeError::new(
        ErrorKind::Unsupported,
        format!(
            "Standard-library binding `{uri}#{}` is not implemented.",
            declaration.name
        ),
    ))]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ready(vm: &mut Vm) -> Result<Value, RuntimeError> {
        for _ in 0..100_000 {
            match vm.poll(1) {
                VmPoll::Ready(value) => return value,
                VmPoll::Yielded => {
                    vm.collect(&[]);
                }
                event => panic!("stdlib initialization leaked Host work: {event:?}"),
            }
        }
        panic!("stdlib initialization did not settle")
    }

    #[test]
    fn pinned_annotations_and_base_output_initialize_in_the_same_heap_without_host_effects() {
        let parsed = crate::parser::parse_module(
            "base = import(\"pkl:base\")\ntesting = import(\"pkl:test\")\n",
        )
        .unwrap();
        let program = compile_module(&crate::resolver::resolve_module(&parsed).unwrap()).unwrap();
        let mut vm = Vm::with_context(
            program.clone(),
            EvaluationContext {
                primary_source: Some(Arc::new(crate::value::RuntimeSource {
                    text: parsed.source.clone(),
                    uri: "file:///stdlib-structure.pkl".into(),
                    module_name: "main".into(),
                    line_anchors: false,
                })),
                ..Default::default()
            },
        );
        vm.start(program.entry, vec![]).unwrap();
        let module = ready(&mut vm).unwrap().as_object().unwrap();
        for name in ["base", "testing"] {
            vm.start_member(module, name).unwrap();
            let standard = ready(&mut vm).unwrap().as_object().unwrap();
            let annotations = vm.heap.get(standard).unwrap().annotations.clone();
            assert_eq!(annotations.len(), 1);
            let annotation = annotations[0].as_object().unwrap();
            let class = collections::typed_class(&vm.heap, annotation)
                .unwrap()
                .unwrap();
            assert_eq!(class.id, annotations::builtin_id("ModuleInfo").unwrap());
            assert_eq!(
                vm.member_state(annotation, "minPklVersion").unwrap(),
                LazyState::Completed(Value::String("0.32.0".into()))
            );
            if name == "base" {
                let LazyState::Completed(Value::Object(output)) =
                    vm.member_state(standard, "output").unwrap()
                else {
                    panic!("base output was not eager")
                };
                assert_eq!(
                    vm.member_state(output, "text").unwrap(),
                    LazyState::Completed(Value::String("".into()))
                );
                assert_eq!(
                    vm.member_state(output, "value").unwrap(),
                    LazyState::Completed(Value::Object(standard))
                );
                assert!(
                    !vm.heap.get(standard).unwrap().forced,
                    "unimplemented bindings must not be reported fully forced"
                );
                vm.start_member(standard, "Char").unwrap();
                assert_eq!(ready(&mut vm).unwrap_err().kind, ErrorKind::Unsupported);
            } else {
                assert!(matches!(
                    vm.member_state(standard, "facts").unwrap(),
                    LazyState::Completed(Value::NullWithDefault(_))
                ));
                assert_eq!(
                    vm.member_state(standard, "output").unwrap(),
                    LazyState::Unevaluated
                );
            }
            vm.collect(&[]);
            assert!(vm.heap.get(annotation).is_ok());
        }
        vm.close();
        assert_eq!(vm.heap_stats().slots, 0);
    }
}
