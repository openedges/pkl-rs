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
            "pkl:reflect" => crate::stdlib_sources::reflect(),
            "pkl:json" => crate::stdlib_sources::json(),
            "pkl:yaml" => crate::stdlib_sources::yaml(),
            "pkl:math" => crate::stdlib_sources::math(),
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
    if uri == "pkl:reflect" {
        // Compile the actual module, including distinct class/property and
        // method namespaces. External bodies bind in the shared compiler.
        // Publish the singleton before recursively forcing its constants, as
        // StdLibModule.doLoad does for the JVM's statically shared modules.
        let resolved = crate::resolver::resolve_stdlib_module(parsed)
            .map_err(|error| RuntimeError::new(ErrorKind::Unsupported, error.message))?;
        let unit = compile_unit_into(
            program,
            &resolved,
            CompileOptions {
                fuse_constant_add: true,
            },
            false,
            Some(source),
            Some(ModuleIdentity {
                original_uri: uri.into(),
                resolved_uri: uri.into(),
            }),
        )?;
        append_bootstrap(program, unit, uri);
        return Ok(unit);
    }
    // Compile the real authored annotation/initialization path. External member
    // bodies are bound below; they are not rewritten into Pkl source strings.
    let mut header = parsed.clone();
    // Pure Pkl declarations keep their actual bodies, annotations, class identity
    // and source owners. The JSON parser's external binding remains explicit.
    header.members.retain(|member| {
        (matches!(uri, "pkl:json" | "pkl:yaml")
            && matches!(member.name.as_str(), "Property" | "Value"))
            || (uri == "pkl:base"
                && matches!(
                    member.name.as_str(),
                    "DataSizeUnit"
                        | "DurationUnit"
                        | "Charset"
                        | "Comparable"
                        | "Char"
                        | "Uri"
                        | "Mixin"
                ))
    });
    let resolved = crate::resolver::resolve_stdlib_module(&header)
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
        } else if uri == "pkl:math" {
            math_binding(declaration)
        } else if matches!(uri, "pkl:json" | "pkl:yaml") {
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
        program
            .declaration_sources
            .insert(block, declaration.declaration_source());
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
    definition.syntax = Some(Arc::new(parsed.clone()));
    append_bootstrap(program, unit, uri);
    compile_reflection_annotations(program, unit, uri, source, parsed)?;
    Ok(unit)
}

fn append_bootstrap(program: &mut Program, unit: UnitId, uri: &str) {
    let entry = program.units[unit.0 as usize].entry;
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
}

fn base_binding(declaration: &crate::syntax::Member) -> Vec<Instruction> {
    let name = declaration.name.as_str();
    match &declaration.expr.kind {
        ExprKind::Function { method: true, .. } if name == "Regex" => {
            return vec![Instruction::RegexConstructor];
        }
        ExprKind::Function { method: true, .. } if name == "Pair" => {
            return vec![Instruction::PairConstructor];
        }
        ExprKind::Function { method: true, .. } if name == "IntSeq" => {
            return vec![Instruction::IntSeqConstructor];
        }
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

// Every property constant declared by pinned MathNodes. Transcendental and
// arithmetic functions retain explicit unsupported bindings.
fn math_binding(declaration: &crate::syntax::Member) -> Vec<Instruction> {
    let name = declaration.name.as_str();
    let integer = match name {
        "minInt" => Some(i64::MIN),
        "maxInt" | "maxUInt" => Some(i64::MAX),
        "minInt8" => Some(i8::MIN as i64),
        "maxInt8" => Some(i8::MAX as i64),
        "minInt16" => Some(i16::MIN as i64),
        "maxInt16" => Some(i16::MAX as i64),
        "minInt32" => Some(i32::MIN as i64),
        "maxInt32" => Some(i32::MAX as i64),
        "maxUInt8" => Some(u8::MAX as i64),
        "maxUInt16" => Some(u16::MAX as i64),
        "maxUInt32" => Some(u32::MAX as i64),
        _ => None,
    };
    let value = if let Some(value) = integer {
        Value::Int(value.into())
    } else {
        Value::Float(
            match name {
                "minFiniteFloat" => -f64::MAX,
                "maxFiniteFloat" => f64::MAX,
                "minPositiveFloat" => f64::from_bits(1),
                "e" => std::f64::consts::E,
                "pi" => std::f64::consts::PI,
                _ => return unsupported_binding("pkl:math", declaration),
            }
            .into(),
        )
    };
    vec![Instruction::Constant(value)]
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

// Builtin declaration annotations have ordinary executable bodies even when
// their enclosing generic class uses native members. Compile only these exact
// annotation ASTs, retaining source spans and the standard module code owner.
fn compile_reflection_annotations(
    program: &mut Program,
    unit: UnitId,
    uri: &str,
    source: Arc<RuntimeSource>,
    parsed: &crate::syntax::Module,
) -> Result<(), RuntimeError> {
    let resolved = crate::resolver::resolve_stdlib_module(parsed)
        .map_err(|error| RuntimeError::new(ErrorKind::Unsupported, error.message))?;
    let declaration_base = program.next_declaration;
    program.next_declaration = declaration_base
        .checked_add(source.text.len() + 2)
        .ok_or_else(|| invalid("annotation declaration identity limit"))?;
    let mut compiler = Compiler {
        resolved: &resolved,
        program,
        unit,
        declaration_base,
        source: Some(source),
        identity: Some(ModuleIdentity {
            original_uri: uri.into(),
            resolved_uri: uri.into(),
        }),
        options: CompileOptions {
            fuse_constant_add: true,
        },
        member: PklString::default(),
        super_method_allowed: false,
        module_annotation: false,
        let_members: Vec::new(),
        type_scopes: Vec::new(),
        runtime_type_owner: true,
        runtime_type_fallback: None,
        modifiers: Vec::new(),
        constraint_module: false,
        alias_frames: Vec::new(),
        constraint_origins: BTreeMap::new(),
        active_constraints: BTreeMap::new(),
        shared_constraint_roots: BTreeMap::new(),
        active_type_parameters: BTreeMap::new(),
        constructor_parameters_bound: false,
        parameter_spans: BTreeMap::new(),
        argument_spans: BTreeMap::new(),
        function_counts: BTreeMap::new(),
        entry_counts: BTreeMap::new(),
        function_names: BTreeMap::new(),
        instrument: false,
    };
    let mut pending: Vec<_> = parsed
        .members
        .iter()
        .rev()
        .map(|member| (format!("#{}", member.name), member))
        .collect();
    while let Some((name, member)) = pending.pop() {
        if !member.annotations.is_empty() {
            compiler.member = name.as_str().into();
            let mut code = Code::default();
            code.push(Instruction::BeginMemberAnnotations);
            compiler.annotations(&member.annotations, &mut code)?;
            code.push(Instruction::Return);
            let block = compiler.block(code, Some(member.declaration_source().header_span))?;
            compiler
                .program
                .reflection_annotations
                .insert((uri.into(), member.declaration_source().header_span), block);
        }
        if let ExprKind::Class { members, .. } = &member.expr.kind {
            pending.extend(
                members
                    .iter()
                    .rev()
                    .map(|member| (format!("{name}.{}", member.name), member)),
            );
        }
    }
    compiler
        .program
        .constraint_origins
        .extend(compiler.constraint_origins);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ready(vm: &mut Vm) -> Result<Value, RuntimeError> {
        ready_with_limit(vm, 100_000)
    }

    fn ready_reflection(vm: &mut Vm) -> Result<Value, RuntimeError> {
        // Eager standard mirrors exceed the smaller module bootstrap budget.
        // Keep the same single-instruction polling and GC stress condition.
        ready_with_limit(vm, 400_000)
    }

    fn ready_with_limit(vm: &mut Vm, limit: usize) -> Result<Value, RuntimeError> {
        for _ in 0..limit {
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
    fn reflection_bootstrap_forces_standard_mirrors_without_user_frame_callbacks() {
        let parsed = crate::parser::parse_module(
            r#"reflect = import("pkl:reflect")
class User {}
unknownSame = reflect.listType.typeArguments.first == reflect.unknownType
unknownSetSize = Set(reflect.listType.typeArguments.first, reflect.unknownType).length
regularSame = reflect.StringLiteralType("a") == reflect.StringLiteralType("a")
regularDifferent = reflect.StringLiteralType("a") == reflect.StringLiteralType("b")
userLocation = reflect.Class(User).location.displayUri
"#,
        )
        .unwrap();
        let program = compile_module(&crate::resolver::resolve_module(&parsed).unwrap()).unwrap();
        let mut vm = Vm::with_context(
            program.clone(),
            EvaluationContext {
                primary_source: Some(Arc::new(crate::value::RuntimeSource {
                    text: parsed.source.clone(),
                    uri: "file:///reflection-bootstrap.pkl".into(),
                    module_name: "main".into(),
                })),
                frame_transformer: crate::logging::FrameTransformer::Host,
                ..Default::default()
            },
        );
        vm.start(program.entry, vec![]).unwrap();
        let module = ready_reflection(&mut vm).unwrap().as_object().unwrap();
        vm.start_member(module, "reflect").unwrap();
        // ready() rejects any Host event. In particular, eager standard mirror
        // displayUri reads must finish with the default transformer.
        let reflect = ready_reflection(&mut vm).unwrap().as_object().unwrap();
        let mut current = reflect;
        for name in ["intType", "referent", "location"] {
            let LazyState::Completed(Value::Object(next)) = vm.member_state(current, name).unwrap()
            else {
                panic!("standard mirror {name} was not eagerly cached")
            };
            current = next;
        }
        let LazyState::Completed(Value::String(uri)) =
            vm.member_state(current, "displayUri").unwrap()
        else {
            panic!("standard declaration location was not eagerly cached")
        };
        assert!(
            uri.as_str()
                .unwrap()
                .starts_with("https://github.com/apple/pkl/blob/0.32.0/stdlib/base.pkl#L")
        );
        // UnknownType has only the inherited external nullable property. Two
        // distinct instances compare/hash without requesting a nullable cache.
        for (name, expected) in [
            ("unknownSame", Value::Bool(true)),
            ("unknownSetSize", Value::Int(1.into())),
            ("regularSame", Value::Bool(true)),
            ("regularDifferent", Value::Bool(false)),
        ] {
            vm.start_member(module, name).unwrap();
            assert_eq!(ready_reflection(&mut vm).unwrap(), expected, "{name}");
        }
        // The isolated bootstrap policy must not leak into user declarations.
        vm.start_member(module, "userLocation").unwrap();
        let mut request = None;
        for _ in 0..100_000 {
            match vm.poll(1) {
                VmPoll::Yielded => {
                    vm.collect(&[]);
                }
                VmPoll::TransformFrame(pending) => {
                    request = Some(pending);
                    break;
                }
                event => panic!("user location did not suspend for Host: {event:?}"),
            }
        }
        let request = request.expect("bounded user location Host callback");
        assert_eq!(request.frame.module_uri, "file:///reflection-bootstrap.pkl");
        vm.complete_frame_transform(request.id, Ok((*request.frame).clone()))
            .unwrap();
        assert_eq!(
            ready_reflection(&mut vm).unwrap(),
            Value::String("file:///reflection-bootstrap.pkl".into())
        );
        vm.close();
        assert_eq!(vm.heap_stats().slots, 0);
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
                    vm.heap.get(standard).unwrap().forced,
                    "base initialization must complete recursive forcing"
                );
                vm.start_member(standard, "Char").unwrap();
                let Value::TypeAlias(char_alias) = ready(&mut vm).unwrap() else {
                    panic!("Char was not a type alias")
                };
                assert_eq!(
                    vm.heap.get(char_alias).unwrap().type_alias.as_deref(),
                    Some("pkl.base#Char")
                );
                assert!(vm.heap.get(char_alias).unwrap().type_alias_initialized);
                vm.collect(&[]);
                vm.start_member(standard, "Char").unwrap();
                assert_eq!(ready(&mut vm).unwrap(), Value::TypeAlias(char_alias));
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
