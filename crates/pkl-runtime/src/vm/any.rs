//! Any methods share the ordinary call, argument-check and continuation paths.
use super::*;

pub(super) fn blocks(
    blocks: &mut Vec<Vec<Instruction>>,
    metadata: &mut Vec<BlockMetadata>,
) -> BTreeMap<(Arc<str>, Arc<str>), usize> {
    let source = crate::stdlib_sources::base();
    let signature = "external function ifNonNull<Result>(transform: (NonNull) -> Result): Result?";
    let start = source.text.find(signature).unwrap();
    let type_start = start + signature.find("(NonNull) -> Result").unwrap();
    let block = blocks.len();
    blocks.push(vec![
        Instruction::LoadLocal(0),
        Instruction::CheckType(TypeCheck::Function {
            parameters: Some(vec![TypeCheck::NonNull]),
            result: Box::new(TypeCheck::Unknown),
            syntax: true,
        }),
        Instruction::Primitive(primitives::Operation::IfNonNull),
        Instruction::Return,
    ]);
    metadata.push(BlockMetadata {
        source: Some(source),
        member: "#Any.ifNonNull".into(),
        declaration: Some(Span::new(start, start + signature.len())),
        spans: vec![
            None,
            Some(Span::new(
                type_start,
                type_start + "(NonNull) -> Result".len(),
            )),
            None,
            None,
        ],
        ..Default::default()
    });
    BTreeMap::from([(("Any".into(), "ifNonNull".into()), block)])
}

impl Vm {
    pub(super) fn non_const_method_error(name: &str) -> RuntimeError {
        let mut error = RuntimeError::new(
            ErrorKind::Type,
            format!("Cannot call method `{name}` from here because it is not `const`."),
        );
        error.hint = Some(format!(
            "To fix, do either of:\n1. Add modifier `const` to method `{name}`\n2. Self-import this module, and reference this method from the import."
        ).into());
        error
    }

    pub(super) fn any_method(&mut self, receiver: Value) {
        let block = self.program.primitive_methods[&("Any".into(), "ifNonNull".into())];
        let id = self.heap.allocate(Object {
            function: Some(Function {
                block,
                arity: 1,
                method: true,
                captured_this: Some(receiver),
            }),
            ..Default::default()
        });
        self.stack.push(Value::Function(id));
    }

    pub(super) fn any_member(&mut self, receiver: &Value, name: &str, method: bool) -> bool {
        // Objects must search their ordinary method ancestry first. This also
        // preserves the distinct lookup origins of local and super calls.
        if method && name == "getClass" && !matches!(receiver, Value::Object(_) | Value::Class(_)) {
            // Lambdas capture custom this as their ordinary receiver. Both
            // explicit and implicit calls must dispatch Any.getClass on that
            // value, rather than trying to resolve a scalar object's member.
            let function = self.native_reference_method(receiver.clone(), "getClass");
            self.stack.push(function);
            return true;
        }
        if method && name == "ifNonNull" && !matches!(receiver, Value::Object(_)) {
            self.any_method(receiver.clone());
            return true;
        }
        false
    }

    pub(super) fn if_non_null(&mut self, receiver: Value) -> Result<(), RuntimeError> {
        let Value::Function(function) = self.pop()? else {
            return Err(invalid("checked ifNonNull function"));
        };
        if receiver.is_null() {
            // VmNull returns itself, retaining an attached amendment default.
            // Argument evaluation and the Function1 check have already run.
            self.stack.push(receiver);
            Ok(())
        } else {
            self.invoke_function(function, vec![receiver], false)
        }
    }
}
