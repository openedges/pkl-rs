//! Development-only bounded lifecycle protocol; diagnostics remain separate raw files.
use pkl_runtime::{
    heap::Member,
    host::{HostPoll, HostSession},
    value::{ErrorKind, RuntimeError, Value},
    vm::{Instruction as I, Program, Vm, VmPoll},
};
use std::{path::Path, sync::Arc};

fn diagnostic(output: &Path, name: &str, error: &RuntimeError) {
    std::fs::write(output.join(format!("{name}.error.bin")), error.to_string()).unwrap();
    std::fs::write(
        output.join(format!("{name}.error-class.bin")),
        format!("{:?}", error.kind),
    )
    .unwrap();
}

fn accessor(program: &mut Program, name: &str) -> usize {
    let entry = program.blocks.len();
    program
        .blocks
        .push(vec![I::Input(0), I::GetMember(Arc::from(name)), I::Return]);
    entry
}

fn vm_ready(vm: &mut Vm) -> Value {
    match vm.poll(1024) {
        VmPoll::Ready(Ok(value)) => value,
        other => panic!("unexpected VM result: {other:?}"),
    }
}

fn host_ready(host: &mut HostSession) -> Value {
    match host.poll(0, 1024) {
        HostPoll::Ready(Ok(value)) => value,
        other => panic!("unexpected Host result: {other:?}"),
    }
}

fn text(value: Value) -> String {
    match value {
        Value::String(value) => value.to_string(),
        other => panic!("unexpected value: {other:?}"),
    }
}

// The bounded frontend has no read(...) call lowering. These two explicit
// bytecode translations are part of the probe contract, not parser evidence.
fn program(mode: &str, source: &str) -> Arc<Program> {
    let (expected, members, blocks) = match mode {
        "retry" => (
            "first = read(\"life:first\")\nsecond = read(\"life:second\")\nresult = first + second\n",
            vec![("first", 1), ("second", 2), ("result", 3)],
            vec![
                vec![
                    I::MakeObject {
                        template: 0,
                        has_parent: false,
                    },
                    I::Return,
                ],
                vec![
                    I::Constant(Value::string("life:first")),
                    I::ReadResource,
                    I::Return,
                ],
                vec![
                    I::Constant(Value::string("life:second")),
                    I::ReadResource,
                    I::Return,
                ],
                vec![
                    I::LoadThis,
                    I::GetMember(Arc::from("first")),
                    I::LoadThis,
                    I::GetMember(Arc::from("second")),
                    I::Add,
                    I::Return,
                ],
            ],
        ),
        "reentry" => (
            "seed = read(\"life:shared\")\nshared = seed + \"!\"\nresult = read(\"life:outer\") + shared\n",
            vec![("seed", 1), ("shared", 2), ("result", 3)],
            vec![
                vec![
                    I::MakeObject {
                        template: 0,
                        has_parent: false,
                    },
                    I::Return,
                ],
                vec![
                    I::Constant(Value::string("life:shared")),
                    I::ReadResource,
                    I::Return,
                ],
                vec![
                    I::LoadThis,
                    I::GetMember(Arc::from("seed")),
                    I::Constant(Value::string("!")),
                    I::Add,
                    I::Return,
                ],
                vec![
                    I::Constant(Value::string("life:outer")),
                    I::ReadResource,
                    I::LoadThis,
                    I::GetMember(Arc::from("shared")),
                    I::Add,
                    I::Return,
                ],
            ],
        ),
        _ => panic!("unknown mode"),
    };
    assert_eq!(
        source, expected,
        "input differs from explicit bytecode translation"
    );
    let mut program = Program::new(blocks);
    program.objects.push(
        members
            .into_iter()
            .map(|(name, code)| Member {
                entry: false,
                name: Arc::from(name),
                code,
                local: false,
                method: false,
                ty: None,
                type_span: None,
                constant: false,
                fixed: false,
            })
            .collect(),
    );
    Arc::new(program)
}

fn main() {
    let args: Vec<_> = std::env::args().collect();
    assert_eq!(args.len(), 4);
    let mode = args[1].as_str();
    let output = Path::new(&args[3]);
    let source = std::fs::read_to_string(&args[2]).unwrap();
    let compiled = program(mode, &source);
    if mode == "retry" {
        let mut vm = Vm::new(compiled.clone());
        vm.start(compiled.entry, vec![]).unwrap();
        let object = vm_ready(&mut vm).as_object().unwrap();
        let (mut first, mut second) = (0, 0);
        for attempt in 1..=4 {
            vm.start_member(object, "result").unwrap();
            loop {
                match vm.poll(1024) {
                    VmPoll::Pending(request) => {
                        let (count, value, message) = match request.uri.as_ref() {
                            "life:first" => (&mut first, "A", "transient first failure"),
                            "life:second" => (&mut second, "B", "transient second failure"),
                            other => panic!("unexpected read: {other}"),
                        };
                        *count += 1;
                        let result = if *count == 1 {
                            Err(RuntimeError::new(ErrorKind::Host, message))
                        } else {
                            Ok(Value::string(value))
                        };
                        vm.resume(request.id, result).unwrap();
                    }
                    VmPoll::Ready(Ok(value)) => {
                        println!(
                            "attempt={attempt} outcome=ok value={} first_calls={first} second_calls={second}",
                            text(value)
                        );
                        break;
                    }
                    VmPoll::Ready(Err(error)) => {
                        diagnostic(output, &format!("attempt-{attempt}"), &error);
                        println!(
                            "attempt={attempt} outcome=error first_calls={first} second_calls={second}"
                        );
                        break;
                    }
                    VmPoll::Yielded => panic!("bounded probe exhausted instruction budget"),
                }
            }
        }
        vm.close();
        vm.close();
        println!("close outcome=ok");
        let error = vm.start(compiled.entry, vec![]).unwrap_err();
        diagnostic(output, "after-close", &error);
        println!("after-close outcome=error");
    } else if mode == "reentry" {
        let mut program = (*compiled).clone();
        let outer = accessor(&mut program, "result");
        let shared = accessor(&mut program, "shared");
        let seed = accessor(&mut program, "seed");
        let entry = program.entry;
        let mut host = HostSession::new(Arc::new(program));
        host.start(entry, vec![], 0, None).unwrap();
        let object = host_ready(&mut host);
        host.start(seed, vec![object.clone()], 0, None).unwrap();
        let HostPoll::Read { token, uri } = host.poll(0, 1024) else {
            panic!("seed callback missing");
        };
        assert_eq!(&*uri, "life:shared");
        host.resume(&token, Ok(Value::string("S")), 0).unwrap();
        println!(
            "warmup value={} reader_calls=1",
            text(host_ready(&mut host))
        );
        host.start(outer, vec![object.clone()], 0, None).unwrap();
        let HostPoll::Read {
            token: outer_token,
            uri,
        } = host.poll(0, 1024)
        else {
            panic!("outer callback missing");
        };
        assert_eq!(&*uri, "life:outer");
        let calls = 2;
        host.reenter(&outer_token, shared, vec![object.clone()], 0, None)
            .unwrap();
        let inner = host_ready(&mut host);
        println!("inner value={} reader_calls={calls}", text(inner.clone()));
        let HostPoll::Read { token, uri } = host.poll(0, 1024) else {
            panic!("outer callback was not restored");
        };
        assert_eq!(token, outer_token);
        assert_eq!(&*uri, "life:outer");
        host.resume(&outer_token, Ok(inner), 0).unwrap();
        println!(
            "outer value={} reader_calls={calls}",
            text(host_ready(&mut host))
        );
        host.start(shared, vec![object.clone()], 0, None).unwrap();
        println!(
            "shared value={} reader_calls={calls}",
            text(host_ready(&mut host))
        );
        host.start(outer, vec![object], 0, None).unwrap();
        println!(
            "repeat value={} reader_calls={calls}",
            text(host_ready(&mut host))
        );
        host.close();
        host.close();
        println!("close outcome=ok");
        let error = host.start(entry, vec![], 0, None).unwrap_err();
        diagnostic(output, "after-close", &error);
        println!("after-close outcome=error");
    } else {
        panic!("unknown mode");
    }
}
