//! Ordinary parsed expressions appended to an existing module's code arena.
//! This fixed-entry development boundary does not create another module or REPL.
use super::*;

#[derive(Debug)]
pub enum SessionExpressionError {
    Frontend(Box<crate::diagnostic::Diagnostic>),
    Runtime(Box<RuntimeError>),
}

/// Compile one expression with its synthetic declaration owner and independent
/// `repl:text` spans. All emitted code shares the caller's Program and heap.
pub fn compile_session_expression(
    program: &mut Program,
    module: &crate::syntax::Module,
    expression_source: impl Into<crate::source::SourceText>,
    options: CompileOptions,
    power_assertions: bool,
) -> Result<usize, SessionExpressionError> {
    compile_session_expression_for_unit(
        program,
        module,
        expression_source,
        options,
        power_assertions,
        crate::modules::UnitId(0),
    )
}

pub(super) fn compile_session_expression_for_unit(
    program: &mut Program,
    module: &crate::syntax::Module,
    expression_source: impl Into<crate::source::SourceText>,
    options: CompileOptions,
    power_assertions: bool,
    primary: crate::modules::UnitId,
) -> Result<usize, SessionExpressionError> {
    compile_session_expression_inner(
        program,
        module,
        expression_source,
        options,
        power_assertions,
        primary,
        true,
    )
}

pub(super) fn compile_session_expression_value_for_unit(
    program: &mut Program,
    module: &crate::syntax::Module,
    expression_source: impl Into<crate::source::SourceText>,
    options: CompileOptions,
    power_assertions: bool,
    primary: crate::modules::UnitId,
) -> Result<usize, SessionExpressionError> {
    compile_session_expression_inner(
        program,
        module,
        expression_source,
        options,
        power_assertions,
        primary,
        false,
    )
}

struct CachedParsedExpression {
    hash: u64,
    source: crate::source::SourceText,
    parsed: Arc<crate::parser::ParsedExpression>,
}

fn parsed_expression_cache() -> &'static std::sync::Mutex<Vec<CachedParsedExpression>> {
    static CACHE: std::sync::OnceLock<std::sync::Mutex<Vec<CachedParsedExpression>>> =
        std::sync::OnceLock::new();
    CACHE.get_or_init(|| std::sync::Mutex::new(Vec::with_capacity(16)))
}

fn parse_expression_cached(
    source: &crate::source::SourceText,
) -> Result<Arc<crate::parser::ParsedExpression>, crate::diagnostic::Diagnostic> {
    if source.len_bytes() <= 16 * 1024 {
        let hash = modules::source_text_hash(source);
        if let Ok(cache) = parsed_expression_cache().lock()
            && let Some(entry) = cache
                .iter()
                .rev()
                .find(|entry| entry.hash == hash && entry.source == *source)
        {
            return Ok(Arc::clone(&entry.parsed));
        }
        let parsed = Arc::new(crate::parser::parse_expression_for_evaluation(source)?);
        if let Ok(mut cache) = parsed_expression_cache().lock()
            && !cache
                .iter()
                .any(|entry| entry.hash == hash && entry.source == *source)
        {
            if cache.len() >= 16 {
                cache.remove(0);
            }
            cache.push(CachedParsedExpression {
                hash,
                source: source.clone(),
                parsed: Arc::clone(&parsed),
            });
        }
        return Ok(parsed);
    }
    crate::parser::parse_expression_for_evaluation(source).map(Arc::new)
}

fn compile_session_expression_inner(
    program: &mut Program,
    module: &crate::syntax::Module,
    expression_source: impl Into<crate::source::SourceText>,
    options: CompileOptions,
    power_assertions: bool,
    primary: crate::modules::UnitId,
    stringify: bool,
) -> Result<usize, SessionExpressionError> {
    let expression_source = expression_source.into();
    let parsed = parse_expression_cached(&expression_source)
        .map_err(|error| SessionExpressionError::Frontend(Box::new(error)))?;
    let resolved =
        crate::resolver::resolve_session_expression(module, expression_source.entire(), &parsed)
            .map_err(|error| SessionExpressionError::Frontend(Box::new(error)))?;
    let identity = crate::modules::ModuleIdentity {
        original_uri: "repl:text".into(),
        resolved_uri: "repl:text".into(),
    };
    append_guarded(
        program,
        &resolved,
        &parsed.expression,
        &identity,
        options,
        power_assertions,
        primary,
        stringify,
    )
    .map_err(|error| SessionExpressionError::Runtime(Box::new(error)))
}

#[allow(clippy::too_many_arguments)]
fn append_guarded(
    program: &mut Program,
    resolved: &crate::resolver::ResolvedModule,
    expression: &crate::syntax::Expr,
    identity: &crate::modules::ModuleIdentity,
    options: CompileOptions,
    power_assertions: bool,
    primary: crate::modules::UnitId,
    stringify: bool,
) -> Result<usize, RuntimeError> {
    let checkpoint = modules::ArenaCheckpoint::new(program);
    let result = append(
        program,
        resolved,
        expression,
        identity,
        options,
        power_assertions,
        primary,
        stringify,
    );
    if result.is_err() {
        checkpoint.rollback(program);
    }
    result
}

#[allow(clippy::too_many_arguments)]
fn append(
    program: &mut Program,
    resolved: &crate::resolver::ResolvedModule,
    expression: &crate::syntax::Expr,
    identity: &crate::modules::ModuleIdentity,
    options: CompileOptions,
    instrument: bool,
    primary: crate::modules::UnitId,
    stringify: bool,
) -> Result<usize, RuntimeError> {
    let primary = program
        .units
        .get(primary.0 as usize)
        .cloned()
        .ok_or_else(|| invalid("missing session module"))?;
    let unit = crate::modules::UnitId(
        program
            .units
            .next_id()
            .try_into()
            .map_err(|_| invalid("module unit limit"))?,
    );
    let source = Arc::new(crate::value::RuntimeSource {
        text: resolved.module.source.clone(),
        uri: "repl:text".into(),
        module_name: "".into(),
    });
    let declaration_base = program.next_declaration;
    program.next_declaration = declaration_base
        .checked_add(resolved.module.source.len_bytes() + 2)
        .ok_or_else(|| invalid("declaration identity limit"))?;
    let mut compiler = Compiler {
        resolved,
        type_lookup: build_type_lookup(resolved),
        program,
        unit,
        declaration_base,
        source: Some(source.clone()),
        identity: Some(identity.clone()),
        options,
        member: "repl:text".into(),
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
        parameter_label_spans: BTreeMap::new(),
        argument_spans: BTreeMap::new(),
        function_counts: BTreeMap::new(),
        entry_counts: BTreeMap::new(),
        function_names: BTreeMap::new(),
        declared_recipes: BTreeMap::new(),
        module_recipes: BTreeMap::new(),
        instrument,
    };
    let mut code = Code::default();
    code.push(Instruction::PrepareModuleImports(unit));
    if resolved.preparation_failure.is_none() {
        compiler.expression(expression, &mut code)?;
    }
    if stringify {
        code.push(Instruction::ToString);
    }
    code.push(Instruction::Return);
    let entry = compiler.block(code, Some(expression.span))?;
    compiler.program.metadata[entry].module_receiver_input = Some(0);
    // AstBuilder uses the synthetic ModuleInfo name for nested lexical nodes;
    // the expression's SimpleRootNode itself has an empty name.
    compiler.program.metadata[entry].member = PklString::default();
    compiler
        .program
        .constraint_origins
        .extend(compiler.constraint_origins);
    // This source unit owns bytecode provenance only. It shares the existing
    // module receiver and never runs an initializer or publishes a cache entry.
    compiler.program.units.try_push(modules::ModuleUnit {
        id: unit,
        identity: Some(identity.clone()),
        standard_library: false,
        singleton_cache_key: None,
        key: None,
        delegate_key: None,
        build: None,
        syntax: None,
        source: Some(source),
        name: "".into(),
        template: primary.template,
        entry: usize::MAX,
        blocks_start: 0,
        initial_members: Vec::new(),
        import_order: uri_resolution::module_import_order(resolved),
        class_id: primary.class_id,
        mode: modules::ModuleMode::Plain,
        modifiers: 0,
    })?;
    Ok(entry)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn failed_expression_append_rolls_back_emitted_lambda_type_recipe_and_source_unit_arena() {
        let module = crate::parser::parse_module("stable=7\n").unwrap();
        let resolved = crate::resolver::resolve_module(&module).unwrap();
        let mut program = Arc::unwrap_or_clone(compile_module(&resolved).unwrap());
        let earlier = compile_session_expression(
            &mut program,
            &module,
            "stable + 1",
            CompileOptions::default(),
            false,
        )
        .unwrap();
        let expression = "let (f=(x:remote.Person)->x) missing";
        let parsed = crate::parser::parse_expression_with_metadata(expression).unwrap();
        let mut resolved_expression =
            crate::resolver::resolve_session_expression(&module, expression, &parsed).unwrap();
        // Imports now prepare in the VM. Inject an invalid binding after a valid
        // lambda to exercise rollback after partial compilation deterministically.
        let crate::syntax::ExprKind::Let { body, .. } = &parsed.expression.kind else {
            unreachable!()
        };
        resolved_expression.names.insert(
            body.span,
            crate::resolver::Binding::Lexical { depth: 0, slot: 1 },
        );
        let identity = crate::modules::ModuleIdentity {
            original_uri: "repl:text".into(),
            resolved_uri: "repl:text".into(),
        };
        // Prove this is a compiler-stage failure after nested code emission,
        // rather than a parser/resolver rejection before the transaction.
        let mut unguarded = program.clone();
        let error = append(
            &mut unguarded,
            &resolved_expression,
            &parsed.expression,
            &identity,
            CompileOptions::default(),
            false,
            crate::modules::UnitId(0),
            true,
        )
        .unwrap_err();
        assert_eq!(error.kind, ErrorKind::InvalidBytecode);
        assert!(unguarded.blocks.len() > program.blocks.len());
        assert!(unguarded.metadata.len() > program.metadata.len());
        assert!(unguarded.type_recipes.len() > program.type_recipes.len());
        let before = (
            program.blocks.len(),
            program.metadata.len(),
            program.objects.len(),
            program.units.len(),
            program.next_declaration,
            program.constraint_origins.len(),
            program.type_recipes.len(),
            program
                .alias_definitions
                .keys()
                .copied()
                .collect::<Vec<_>>(),
        );
        assert!(
            append_guarded(
                &mut program,
                &resolved_expression,
                &parsed.expression,
                &identity,
                CompileOptions::default(),
                false,
                crate::modules::UnitId(0),
                true,
            )
            .is_err()
        );
        assert_eq!(
            before,
            (
                program.blocks.len(),
                program.metadata.len(),
                program.objects.len(),
                program.units.len(),
                program.next_declaration,
                program.constraint_origins.len(),
                program.type_recipes.len(),
                program
                    .alias_definitions
                    .keys()
                    .copied()
                    .collect::<Vec<_>>(),
            )
        );
        let module_entry = program.entry;
        let mut vm = Vm::new(Arc::new(program));
        vm.start(module_entry, vec![]).unwrap();
        let receiver = finish(&mut vm);
        vm.start(earlier, vec![receiver]).unwrap();
        assert_eq!(finish(&mut vm), Value::String("8".into()));
    }

    fn finish(vm: &mut Vm) -> Value {
        for _ in 0..100_000 {
            match vm.poll(7) {
                VmPoll::Ready(value) => return value.unwrap(),
                VmPoll::Yielded => {}
                other => panic!("unexpected effect: {other:?}"),
            }
        }
        panic!("expression did not finish");
    }
}
