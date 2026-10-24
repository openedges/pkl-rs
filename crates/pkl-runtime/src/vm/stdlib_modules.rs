//! Evaluator-owned standard-library modules share ordinary module/class/member
//! identities and the same code arena as user declarations. External bindings
//! retain their pinned declaration source; missing bindings fail explicitly.
use super::*;
use crate::modules::{ModuleIdentity, UnitId};
use crate::syntax::ExprKind;
use crate::value::RuntimeSource;

impl Vm {
    pub(super) fn stdlib_source(&self, uri: &str) -> Result<Arc<RuntimeSource>, RuntimeError> {
        Ok(match uri {
            "pkl:base" => crate::stdlib_sources::base(),
            "pkl:test" => crate::stdlib_sources::test(),
            "pkl:ref" => crate::stdlib_sources::reference(),
            "pkl:json" => crate::stdlib_sources::json(),
            _ if uri.starts_with("pkl:") => {
                if crate::modules::stdlib_kind(uri) != Some(crate::modules::StdlibKind::Missing) {
                    return Err(RuntimeError::new(
                        ErrorKind::Unsupported,
                        format!("Standard-library module `{uri}` is not implemented."),
                    ));
                }
                let mut error = RuntimeError::new(
                    ErrorKind::Type,
                    PklString::from("Cannot find standard library module `pkl:")
                        .concat(
                            &crate::host::resources::decoded_scheme_specific_part(uri)
                                .unwrap_or_else(|| uri.strip_prefix("pkl:").unwrap_or(uri).into()),
                        )
                        .concat(&"`.".into()),
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
            _ => return Err(invalid("not a bundled standard-library URI")),
        })
    }

    pub(super) fn stdlib_entry(&mut self, uri: &str) -> Result<usize, RuntimeError> {
        let source = self.stdlib_source(uri)?;
        let program = Arc::make_mut(&mut self.program);
        let unit = append_definition(program, uri, source)?;
        Ok(program.units[unit.0 as usize].entry)
    }

    // Implicit prelude bindings are not authored ImportNodes and have no import
    // edge. Explicit imports go through ModuleRun before reaching this source.
    pub(super) fn import_stdlib(&mut self, uri: &str) -> Result<bool, RuntimeError> {
        let cache = if matches!(
            crate::modules::stdlib_kind(uri),
            Some(crate::modules::StdlibKind::Singleton(_))
        ) {
            &self.modules_stdlib
        } else {
            &self.modules_original
        };
        if let Some(cached) = cache.get(uri).cloned() {
            let value = self.module_cached(cached)?;
            self.stack.push(value);
            return Ok(true);
        }
        let entry = self.stdlib_entry(uri)?;
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
        unit.standard_library
            && unit
                .identity
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
    let result = append_members(program, uri, source, &parsed).inspect(|unit| {
        program.units[unit.0 as usize].standard_library = true;
    });
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
    // Pure Pkl declarations keep their actual bodies, annotations, class identity
    // and source owners. The JSON parser's external binding remains explicit.
    header.members.retain(|member| {
        (uri == "pkl:json" && matches!(member.name.as_str(), "Property" | "Value"))
            || (uri == "pkl:base"
                && matches!(member.name.as_str(), "DataSizeUnit" | "DurationUnit"))
    });
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
    let compiled_members = program.units[unit.0 as usize].initial_members.clone();
    let mut members = Vec::new();
    for declaration in &parsed.members {
        if let Some(member) = compiled_members
            .iter()
            .find(|member| member.name.as_ref() == declaration.name)
        {
            members.push(member.clone());
            continue;
        }
        let method = matches!(
            declaration.expr.kind,
            ExprKind::Function { method: true, .. }
        );
        let mut ty = None;
        let mut type_span = None;
        let mut body = if uri == "pkl:base" {
            base_binding(declaration)
        } else if uri == "pkl:json" {
            unsupported_binding(uri, declaration)
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
                    let block = program.alloc_block(
                        vec![
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
                        ],
                        BlockMetadata {
                            unit: Some(unit),
                            source: Some(source.clone()),
                            member: format!("#{}", declaration.name).into(),
                            declaration: Some(*header_span),
                            spans: vec![None, Some(argument_span), None, None],
                            ..Default::default()
                        },
                    )?;
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
        let spans = vec![Some(declaration.name_span); body.len()];
        let block = program.alloc_block(
            body,
            BlockMetadata {
                unit: Some(unit),
                source: Some(source.clone()),
                member: format!("#{}", declaration.name).into(),
                declaration: Some(declaration.name_span),
                spans,
                ..Default::default()
            },
        )?;
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
        ExprKind::Function { method: true, .. } if name == "Null" => {
            return vec![Instruction::NullConstructor];
        }
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
                f64::NAN.into()
            } else {
                f64::INFINITY.into()
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
                VmPoll::Module(request) => {
                    assert!(
                        matches!(
                            request.kind,
                            crate::modules::ModuleRequestKind::CheckImport { .. }
                                | crate::modules::ModuleRequestKind::SelectKey { .. }
                        ),
                        "singleton imports require admission and actual key selection"
                    );
                    vm.complete_module(
                        request.id,
                        crate::modules::default_completion(&request).unwrap(),
                    )
                    .unwrap();
                }
                event => panic!("stdlib initialization leaked Host work: {event:?}"),
            }
        }
        panic!("stdlib initialization did not settle")
    }

    #[test]
    fn pinned_annotations_and_base_output_share_heap_with_explicit_default_import_policy() {
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
