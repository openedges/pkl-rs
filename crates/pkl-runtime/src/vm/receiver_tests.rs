use super::*;
use crate::heap::CollectionKind;

fn invoke(
    vm: &mut Vm,
    body: usize,
    environment: Environment,
    receiver: Option<Value>,
) -> Result<Value, RuntimeError> {
    let function = vm.heap.allocate(Object {
        function: Some(Function {
            block: body,
            arity: 0,
            method: false,
            captured_this: receiver,
        }),
        environment,
        ..Default::default()
    });
    vm.start(0, vec![Value::Function(function)]).unwrap();
    for _ in 0..100 {
        vm.collect(&[]);
        let event = vm.poll(1);
        let roots = match &event {
            VmPoll::Ready(Ok(value)) => vec![value.clone()],
            _ => Vec::new(),
        };
        vm.collect(&roots);
        match event {
            VmPoll::Yielded => {}
            VmPoll::Ready(value) => return value,
            other => panic!("unexpected receiver event: {other:?}"),
        }
    }
    panic!("receiver poll bound")
}

fn vm() -> Vm {
    Vm::new(Arc::new(Program::new(vec![
        vec![
            Instruction::Input(0),
            Instruction::Call {
                arity: 0,
                method: false,
            },
            Instruction::Return,
        ],
        vec![Instruction::LoadThis, Instruction::Return],
        vec![
            Instruction::GetImplicitProperty("isPositive".into()),
            Instruction::Return,
        ],
        vec![
            Instruction::GetImplicitMethod("toUpperCase".into()),
            Instruction::Call {
                arity: 0,
                method: true,
            },
            Instruction::Return,
        ],
        vec![
            Instruction::GetImplicitProperty("length".into()),
            Instruction::Return,
        ],
    ])))
}

#[test]
fn explicit_receiver_executes_without_a_lexical_object_scope() {
    for (body, receiver, expected) in [
        (1, Value::Null, Value::Null),
        (1, Value::string("four"), Value::string("four")),
        (2, Value::Int(5.into()), Value::Bool(true)),
        (3, Value::string("four"), Value::string("FOUR")),
    ] {
        let mut vm = vm();
        assert_eq!(
            invoke(&mut vm, body, Environment::default(), Some(receiver)).unwrap(),
            expected
        );
        vm.close();
        assert_eq!(vm.heap_stats().slots, 0);
    }
}

#[test]
fn lexical_receiver_is_used_only_when_no_explicit_receiver_exists() {
    for (body, explicit, expected) in [
        (1, None, None),
        (4, None, Some(Value::Int(0.into()))),
        (1, Some(Value::string("four")), Some(Value::string("four"))),
        (2, Some(Value::Int(5.into())), Some(Value::Bool(true))),
        (3, Some(Value::string("four")), Some(Value::string("FOUR"))),
    ] {
        let mut vm = vm();
        let lexical = vm.heap.allocate(Object {
            collection_kind: CollectionKind::List,
            ..Default::default()
        });
        let environment = Environment {
            scopes: vec![Scope {
                receiver: lexical,
                owner: lexical,
            }],
            ..Default::default()
        };
        let expected = expected.unwrap_or(Value::Object(lexical));
        assert_eq!(
            invoke(&mut vm, body, environment, explicit).unwrap(),
            expected
        );
        vm.close();
        assert_eq!(vm.heap_stats().slots, 0);
    }
    let mut vm = vm();
    assert_eq!(
        invoke(&mut vm, 1, Environment::default(), None)
            .unwrap_err()
            .message,
        "invalid object scope"
    );
}
