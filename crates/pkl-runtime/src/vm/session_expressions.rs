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
    expression_source: &str,
    options: CompileOptions,
    power_assertions: bool,
) -> Result<usize, SessionExpressionError> {
    let parsed = crate::parser::parse_expression_with_metadata(expression_source)
        .map_err(|error| SessionExpressionError::Frontend(Box::new(error)))?;
    let resolved = crate::resolver::resolve_session_expression(module, expression_source, &parsed)
        .map_err(|error| SessionExpressionError::Frontend(Box::new(error)))?;
    let identity = crate::modules::ModuleIdentity {
        original_uri: "repl:text".into(),
        resolved_uri: "repl:text".into(),
    };
    let checkpoint = modules::ArenaCheckpoint::new(program);
    let result = append(
        program,
        &resolved,
        &parsed.expression,
        &identity,
        options,
        power_assertions,
    );
    if result.is_err() {
        checkpoint.rollback(program);
    }
    result.map_err(|error| SessionExpressionError::Runtime(Box::new(error)))
}

fn append(
    program: &mut Program,
    resolved: &crate::resolver::ResolvedModule,
    expression: &crate::syntax::Expr,
    identity: &crate::modules::ModuleIdentity,
    options: CompileOptions,
    instrument: bool,
) -> Result<usize, RuntimeError> {
    let primary = program
        .units
        .first()
        .cloned()
        .ok_or_else(|| invalid("missing session module"))?;
    let unit = crate::modules::UnitId(
        program
            .units
            .len()
            .try_into()
            .map_err(|_| invalid("module unit limit"))?,
    );
    let source = Arc::new(crate::value::RuntimeSource {
        text: resolved.module.source.clone(),
        uri: "repl:text".into(),
        module_name: "".into(),
        line_anchors: false,
    });
    let declaration_base = program.next_declaration;
    program.next_declaration = declaration_base
        .checked_add(resolved.module.source.len() + 2)
        .ok_or_else(|| invalid("declaration identity limit"))?;
    let mut compiler = Compiler {
        resolved,
        program,
        unit,
        declaration_base,
        source: Some(source.clone()),
        identity: Some(identity.clone()),
        options,
        member: "repl:text".into(),
        let_members: Vec::new(),
        type_scopes: Vec::new(),
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
        instrument,
    };
    let mut code = Code::default();
    compiler.expression(expression, &mut code)?;
    code.push(Instruction::ToString);
    code.push(Instruction::Return);
    let entry = compiler.block(code, None);
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
    compiler.program.units.push(modules::ModuleUnit {
        id: unit,
        identity: Some(identity.clone()),
        source: Some(source),
        name: "".into(),
        template: primary.template,
        entry: usize::MAX,
        initial_members: Vec::new(),
        class_id: primary.class_id,
    });
    Ok(entry)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn failed_expression_append_rolls_back_emitted_lambda_and_source_unit_arena() {
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
        let expression = "let (f=(x)->x) import(\"child.pkl\")";
        let parsed = crate::parser::parse_expression_with_metadata(expression).unwrap();
        let resolved_expression =
            crate::resolver::resolve_session_expression(&module, expression, &parsed).unwrap();
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
        )
        .unwrap_err();
        assert_eq!(error.kind, ErrorKind::Unsupported);
        assert!(unguarded.blocks.len() > program.blocks.len());
        assert!(unguarded.metadata.len() > program.metadata.len());
        let before = (
            program.blocks.len(),
            program.metadata.len(),
            program.objects.len(),
            program.units.len(),
            program.next_declaration,
            program.constraint_origins.len(),
        );
        assert!(matches!(
            compile_session_expression(
                &mut program,
                &module,
                expression,
                CompileOptions::default(),
                false
            ),
            Err(SessionExpressionError::Runtime(_))
        ));
        assert_eq!(
            before,
            (
                program.blocks.len(),
                program.metadata.len(),
                program.objects.len(),
                program.units.len(),
                program.next_declaration,
                program.constraint_origins.len()
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
