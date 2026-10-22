//! Development-only source-language annotation/Host comparison adapter.
use pkl_runtime::evaluation::runtime_error;
use pkl_runtime::host::{HostPoll, HostSession};
use pkl_runtime::parser::parse_module;
use pkl_runtime::resolver::resolve_module;
use pkl_runtime::source::{Source, SourceMap};
use pkl_runtime::value::{RuntimeError, Value};
use pkl_runtime::vm::{BlockMetadata, Instruction, compile_module};
use std::{fs, path::Path, sync::Arc};

const URI: &str = "file:///m2-platform.pkl";
const NAME: &str = "m2-platform";

fn expressions(path: &Path) -> Vec<String> {
    fs::read_to_string(path)
        .expect("read expressions")
        .lines()
        .map(str::to_owned)
        .collect()
}

fn event(session: &mut HostSession, module: &Value, collections: &mut usize) -> HostPoll {
    for _ in 0..100_000 {
        let event = session.poll(0, 32);
        let mut roots = vec![module.clone()];
        if let HostPoll::Ready(Ok(value)) = &event {
            roots.push(value.clone());
        }
        session.collect(&roots);
        *collections += 1;
        if !matches!(event, HostPoll::Yielded) {
            return event;
        }
    }
    panic!("annotation Host probe instruction budget exceeded");
}

fn scalar(value: Value) -> String {
    match value {
        Value::Null | Value::NullWithDefault(_) => "null".to_owned(),
        Value::Bool(value) => value.to_string(),
        Value::Int(value) => value.to_string(),
        Value::Float(value) => pkl_runtime::numeric::double_to_string(value),
        Value::String(value) => value.to_java_utf8(),
        _ => panic!("annotation Host probe requires scalar properties"),
    }
}

fn capture(
    out: &Path,
    prefix: &str,
    source: &Source,
    expression: &str,
    result: Result<Value, RuntimeError>,
) {
    let (status, stdout, stderr) = match result {
        Ok(value) => ("success\n", scalar(value), String::new()),
        Err(error) => (
            "error\n",
            String::new(),
            runtime_error(source, URI, NAME, expression, error, Some(expression.len())),
        ),
    };
    for (channel, bytes) in [
        ("status", status.as_bytes()),
        ("stdout", stdout.as_bytes()),
        ("stderr", stderr.as_bytes()),
        ("error-class", b""),
    ] {
        fs::write(out.join(format!("{prefix}.{channel}.bin")), bytes).expect("write call capture");
    }
}

fn main() {
    let args: Vec<_> = std::env::args().collect();
    assert_eq!(
        args.len(),
        5,
        "source outer-expressions inner-expressions output"
    );
    let text = fs::read_to_string(&args[1]).expect("read source");
    let outer = expressions(Path::new(&args[2]));
    let inner = expressions(Path::new(&args[3]));
    let out = Path::new(&args[4]);
    let mut sources = SourceMap::default();
    let source_id = sources.add(URI, &text);
    let source = sources.get(source_id).expect("source inserted");
    let parsed = parse_module(&text).expect("parse authored source");
    let resolved = resolve_module(&parsed).expect("resolve authored source");
    let mut program = compile_module(&resolved).expect("compile authored source");
    let program_mut = Arc::make_mut(&mut program);
    program_mut.module_name = NAME.into();
    let mut entries = Vec::new();
    for expression in outer.iter().chain(&inner) {
        let mut block = vec![Instruction::Input(0)];
        for member in expression.split('.') {
            assert!(
                !member.is_empty() && member.chars().all(|ch| ch.is_alphanumeric() || ch == '_'),
                "property paths only"
            );
            block.push(Instruction::GetMember(member.into()));
        }
        block.push(Instruction::Return);
        entries.push(program_mut.blocks.len());
        program_mut.blocks.push(block);
        program_mut.metadata.push(BlockMetadata::default());
    }
    let mut session = HostSession::new(program.clone());
    let mut collections = 0;
    session.start(program.entry, vec![], 0, None).unwrap();
    let HostPoll::Ready(Ok(module)) = event(&mut session, &Value::Null, &mut collections) else {
        panic!("module setup must complete without Host I/O");
    };
    let mut reads = 0;
    for (index, expression) in outer.iter().enumerate() {
        session
            .start(entries[index], vec![module.clone()], 0, None)
            .unwrap();
        let result = loop {
            match event(&mut session, &module, &mut collections) {
                HostPoll::Ready(result) => break result,
                HostPoll::Read { token, uri } => {
                    reads += 1;
                    println!("host-read={uri} calls={reads}");
                    for (inner_index, expression) in inner.iter().enumerate() {
                        session
                            .reenter(
                                &token,
                                entries[outer.len() + inner_index],
                                vec![module.clone()],
                                0,
                                None,
                            )
                            .unwrap();
                        let HostPoll::Ready(result) =
                            event(&mut session, &module, &mut collections)
                        else {
                            panic!("nested property must not issue another Host read");
                        };
                        capture(
                            out,
                            &format!("inner-{reads}-{inner_index}"),
                            source,
                            expression,
                            result,
                        );
                        assert_eq!(session.context_depth(), 1);
                    }
                    assert!(matches!(
                        event(&mut session, &module, &mut collections),
                        HostPoll::Read { token: pending, .. } if pending == token
                    ));
                    session
                        .resume(&token, Ok(Value::string("resource")), 0)
                        .unwrap();
                }
                HostPoll::Yielded => unreachable!(),
            }
        };
        capture(out, &format!("outer-{index}"), source, expression, result);
        assert_eq!(session.context_depth(), 0);
        println!("attempt={index} calls={reads}");
    }
    session.close();
    let stats = session.heap_stats();
    assert_eq!(stats.slots, 0, "terminal close releases the entire heap");
    fs::write(
        out.join("architecture.json"),
        format!(
            "{{\"collections\":{collections},\"reads\":{reads},\"terminal_slots\":{}}}\n",
            stats.slots
        ),
    )
    .expect("write architecture observations");
}
