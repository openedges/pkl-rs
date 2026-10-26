//! Internal Host/GC contracts for shared Plist rendering. These authored
//! lifecycle assertions are distinct from the Java byte comparisons.
use pkl_runtime::{
    host::{HostPoll, HostSession},
    parser::parse_module,
    resolver::resolve_module,
    value::{ErrorKind, RuntimeError, Value},
    vm::{Instruction as I, compile_module},
};
use std::sync::Arc;

fn event(session: &mut HostSession, roots: &[Value], now: u64) -> HostPoll {
    for _ in 0..200_000 {
        session.collect(roots);
        let result = session.poll(now, 1);
        // A terminal value is borrowed by the Host until it is explicitly rooted.
        let mut live = roots.to_vec();
        if let HostPoll::Ready(Ok(value)) = &result {
            live.push(value.clone());
        }
        session.collect(&live);
        if !matches!(result, HostPoll::Yielded) {
            return result;
        }
    }
    panic!("Plist renderer Host poll bound")
}
fn ready(session: &mut HostSession, module: &Value) -> Result<Value, RuntimeError> {
    match event(session, std::slice::from_ref(module), 0) {
        HostPoll::Ready(result) => result,
        other => panic!("unexpected {other:?}"),
    }
}
fn read(session: &mut HostSession, module: &Value, uri: &str) -> String {
    match event(session, std::slice::from_ref(module), 0) {
        HostPoll::Read { token, uri: actual } => {
            assert_eq!(&*actual, uri);
            token
        }
        other => panic!("expected {uri}: {other:?}"),
    }
}
fn resume(session: &mut HostSession, module: &Value, uri: &str, value: Value) {
    let token = read(session, module, uri);
    session.resume(&token, Ok(value), 0).unwrap();
}
fn setup(source: &str, names: &[&str]) -> (HostSession, Value, Vec<usize>) {
    let mut program = (*compile_module(&resolve_module(&parse_module(source).unwrap()).unwrap())
        .unwrap())
    .clone();
    let entries = names
        .iter()
        .map(|name| {
            let block = program.blocks.len();
            program
                .blocks
                .push(vec![I::Input(0), I::GetMember((*name).into()), I::Return]);
            block
        })
        .collect();
    let entry = program.entry;
    let mut session = HostSession::new(Arc::new(program));
    session.start(entry, vec![], 0, None).unwrap();
    let module = match event(&mut session, &[], 0) {
        HostPoll::Ready(Ok(value)) => value,
        other => panic!("module setup: {other:?}"),
    };
    (session, module, entries)
}

#[test]
fn options_shallow_values_converters_and_directive_keep_gc_roots_during_reentry() {
    let source = r#"
hidden input = new Dynamic {
  a = read("mem:value")
  b = new RenderDirective { text = read("mem:directive"); bytes = throw("unused") }
}
hidden renderer = new PListRenderer {
  indent = read("mem:indent")
  converters = if (read("mem:converters") == "go") new Mapping {
    ["^"] = (v) -> if (read("mem:root") == "go") v else throw("root")
    ["a"] = (v) -> read("mem:converted")
  } else throw("converters")
  convertPropertyTransformers = if (read("mem:transformers") == "go") new Mapping {} else throw("transformers")
}
result = renderer.renderValue(input)
parent = read("mem:parent")
nested = read("mem:nested")
"#;
    let (mut session, module, entries) = setup(source, &["parent", "result", "nested"]);
    session
        .start(entries[0], vec![module.clone()], 0, None)
        .unwrap();
    let parent = read(&mut session, &module, "mem:parent");
    session
        .reenter(&parent, entries[1], vec![module.clone()], 0, None)
        .unwrap();
    for (uri, value) in [
        ("mem:indent", "  "),
        ("mem:converters", "go"),
        ("mem:transformers", "go"),
        ("mem:root", "go"),
        ("mem:value", "original"),
    ] {
        resume(&mut session, &module, uri, Value::string(value));
    }
    let conversion = read(&mut session, &module, "mem:converted");
    session
        .reenter(&conversion, entries[2], vec![module.clone()], 0, None)
        .unwrap();
    resume(&mut session, &module, "mem:nested", Value::string("nested"));
    assert_eq!(ready(&mut session, &module), Ok(Value::string("nested")));
    assert_eq!(read(&mut session, &module, "mem:converted"), conversion);
    session
        .resume(&conversion, Ok(Value::string("<&")), 0)
        .unwrap();
    resume(
        &mut session,
        &module,
        "mem:directive",
        Value::string("<raw>&"),
    );
    let rendered = ready(&mut session, &module).unwrap();
    assert_eq!(
        rendered,
        Value::string(
            "<dict>\n  <key>a</key>\n  <string>&lt;&amp;</string>\n  <key>b</key>\n  <raw>&\n</dict>"
        )
    );
    assert_eq!(read(&mut session, &module, "mem:parent"), parent);
    session.resume(&parent, Ok(Value::Null), 0).unwrap();
    assert_eq!(ready(&mut session, &module), Ok(Value::Null));
    session.close();
    assert_eq!(session.collect(&[]).slots, 0);
}

#[test]
fn converted_directive_key_waits_for_value_conversion_and_survives_retry() {
    let source = r#"
hidden input = new Mapping { ["key"] = read("mem:value") }
hidden renderer = new PListRenderer {
  converters {
    [String] = (v) -> if (v == "key") new RenderDirective {
      text = read("mem:key")
      bytes = throw("unused")
    } else read("mem:converted")
  }
}
result = renderer.renderValue(input)
parent = read("mem:parent")
"#;
    let (mut session, module, entries) = setup(source, &["parent", "result"]);
    session
        .start(entries[0], vec![module.clone()], 0, None)
        .unwrap();
    let parent = read(&mut session, &module, "mem:parent");
    session
        .reenter(&parent, entries[1], vec![module.clone()], 0, None)
        .unwrap();
    resume(
        &mut session,
        &module,
        "mem:value",
        Value::string("original"),
    );
    resume(
        &mut session,
        &module,
        "mem:converted",
        Value::string("value"),
    );
    let key = read(&mut session, &module, "mem:key");
    session
        .resume(
            &key,
            Err(RuntimeError::new(ErrorKind::Host, "retry key")),
            0,
        )
        .unwrap();
    assert_eq!(
        ready(&mut session, &module).unwrap_err().kind,
        ErrorKind::Host
    );
    assert_eq!(read(&mut session, &module, "mem:parent"), parent);
    session
        .reenter(&parent, entries[1], vec![module.clone()], 0, None)
        .unwrap();
    // The Mapping value is cached. Both conversions are performed again, and
    // the directive key's text must remain after the value converter.
    resume(
        &mut session,
        &module,
        "mem:converted",
        Value::string("value"),
    );
    resume(&mut session, &module, "mem:key", Value::string("<&"));
    assert_eq!(
        ready(&mut session, &module),
        Ok(Value::string(
            "<dict>\n  <key>&lt;&amp;</key>\n  <string>value</string>\n</dict>"
        ))
    );
    session.resume(&parent, Ok(Value::Null), 0).unwrap();
    assert_eq!(ready(&mut session, &module), Ok(Value::Null));
    session.close();
    assert_eq!(session.collect(&[]).slots, 0);
}

#[test]
fn cancellation_and_close_at_each_pending_renderer_boundary_discard_roots() {
    let source = r#"
hidden input = new Mapping { ["key"] = read("mem:value") }
result = (new PListRenderer {
  indent = read("mem:indent")
  converters = if (read("mem:converters") == "go") new Mapping {
    ["^"] = (v) -> if (read("mem:root") == "go") v else throw("root")
    [String] = (v) -> if (v == "key") new RenderDirective { text = read("mem:key") } else read("mem:converted")
  } else throw("converters")
  convertPropertyTransformers = if (read("mem:transformers") == "go") new Mapping {} else throw("transformers")
}).renderValue(input)
"#;
    let boundaries = [
        "mem:indent",
        "mem:converters",
        "mem:transformers",
        "mem:root",
        "mem:value",
        "mem:converted",
        "mem:key",
    ];
    for stop in 0..boundaries.len() {
        let (mut session, module, entries) = setup(source, &["result"]);
        session
            .start(entries[0], vec![module.clone()], 0, None)
            .unwrap();
        for uri in &boundaries[..stop] {
            resume(
                &mut session,
                &module,
                uri,
                Value::string(if *uri == "mem:indent" { "  " } else { "go" }),
            );
        }
        let token = read(&mut session, &module, boundaries[stop]);
        session.cancel();
        let HostPoll::Ready(Err(error)) = event(&mut session, &[], 0) else {
            panic!("cancelled renderer terminal");
        };
        assert_eq!(error.kind, ErrorKind::Cancelled);
        assert!(session.resume(&token, Ok(Value::Null), 0).is_err());
        session.close();
        assert_eq!(
            session.collect(&[]).slots,
            0,
            "cancelled {}",
            boundaries[stop]
        );
    }
}
