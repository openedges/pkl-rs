//! Development-only source-language annotation/Host comparison adapter.
use pkl_runtime::evaluation::runtime_error;
use pkl_runtime::host::{HostPoll, HostSession};
use pkl_runtime::parser::parse_module;
use pkl_runtime::resolver::resolve_module;
use pkl_runtime::source::{Source, SourceMap, Span};
use pkl_runtime::value::{RuntimeError, RuntimeSource, Value};
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
            runtime_error(source, URI, NAME, expression, error, None),
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

struct Probe<'a> {
    session: HostSession,
    module: Value,
    collections: usize,
    reads: usize,
    out: &'a Path,
    source: &'a Source,
    levels: Vec<Vec<(String, usize)>>,
}

impl Probe<'_> {
    fn evaluate(&mut self, depth: usize) -> Result<Value, RuntimeError> {
        loop {
            match event(&mut self.session, &self.module, &mut self.collections) {
                HostPoll::Ready(result) => return result,
                HostPoll::Read { token, uri } => {
                    self.reads += 1;
                    let call = self.reads;
                    if self.levels.len() > 1 {
                        println!("host-read={uri} calls={call} depth={}", depth + 1);
                    } else {
                        println!("host-read={uri} calls={call}");
                    }
                    let callbacks = self
                        .levels
                        .get(depth)
                        .expect("configured callback depth")
                        .clone();
                    for (index, (expression, entry)) in callbacks.into_iter().enumerate() {
                        self.session
                            .reenter(&token, entry, vec![self.module.clone()], 0, None)
                            .unwrap();
                        let result = self.evaluate(depth + 1);
                        capture(
                            self.out,
                            &format!("inner-{call}-{index}"),
                            self.source,
                            &expression,
                            result,
                        );
                        assert_eq!(self.session.context_depth(), depth + 1);
                    }
                    assert!(matches!(
                        event(&mut self.session, &self.module, &mut self.collections),
                        HostPoll::Read { token: pending, .. } if pending == token
                    ));
                    self.session
                        .resume(&token, Ok(Value::string("resource")), 0)
                        .unwrap();
                }
                HostPoll::Yielded => unreachable!(),
            }
        }
    }
}

fn main() {
    let args: Vec<_> = std::env::args().collect();
    assert!(
        args.len() >= 5,
        "source outer-expressions inner-expressions output [deeper-inner-expressions ...]"
    );
    let text = fs::read_to_string(&args[1]).expect("read source");
    let outer = expressions(Path::new(&args[2]));
    let mut inner_levels = vec![expressions(Path::new(&args[3]))];
    inner_levels.extend(args[5..].iter().map(|path| expressions(Path::new(path))));
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
    for expression in outer.iter().chain(inner_levels.iter().flatten()) {
        let mut block = vec![Instruction::Input(0)];
        let mut spans = vec![None];
        let mut prefix_end = 0;
        for member in expression.split('.') {
            assert!(
                !member.is_empty() && member.chars().all(|ch| ch.is_alphanumeric() || ch == '_'),
                "property paths only"
            );
            block.push(Instruction::GetMember(member.into()));
            prefix_end += usize::from(prefix_end > 0) + member.len();
            spans.push(Some(Span::new(0, prefix_end)));
        }
        block.push(Instruction::Return);
        spans.push(None);
        entries.push(program_mut.blocks.len());
        program_mut.blocks.push(block);
        let mut metadata = BlockMetadata::default();
        metadata.source = Some(Arc::new(RuntimeSource {
            text: expression.clone().into(),
            uri: "repl:text".into(),
            module_name: "".into(),
            line_anchors: false,
        }));
        metadata.spans = spans;
        program_mut.metadata.push(metadata);
    }
    let mut session = HostSession::new(program.clone());
    let mut collections = 0;
    session.start(program.entry, vec![], 0, None).unwrap();
    let HostPoll::Ready(Ok(module)) = event(&mut session, &Value::Null, &mut collections) else {
        panic!("module setup must complete without Host I/O");
    };
    let mut remaining_entries = entries[outer.len()..].iter().copied();
    let levels = inner_levels
        .into_iter()
        .map(|expressions| {
            expressions
                .into_iter()
                .map(|expression| {
                    (
                        expression,
                        remaining_entries.next().expect("callback entry"),
                    )
                })
                .collect()
        })
        .collect();
    let mut probe = Probe {
        session,
        module,
        collections,
        reads: 0,
        out,
        source,
        levels,
    };
    for (index, expression) in outer.iter().enumerate() {
        probe
            .session
            .start(entries[index], vec![probe.module.clone()], 0, None)
            .unwrap();
        let result = probe.evaluate(0);
        capture(out, &format!("outer-{index}"), source, expression, result);
        assert_eq!(probe.session.context_depth(), 0);
        println!("attempt={index} calls={}", probe.reads);
    }
    let Probe {
        mut session,
        collections,
        reads,
        ..
    } = probe;
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
