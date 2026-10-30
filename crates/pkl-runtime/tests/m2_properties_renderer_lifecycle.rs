//! Internal Host/GC contracts for shared Properties rendering. These authored
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
    panic!("Properties renderer Host poll bound")
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
            assert_eq!(actual, uri);
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
fn option_mapping_root_member_and_directive_order_survives_reentry_and_gc() {
    let source = r#"
hidden input = new Dynamic {
  a = read("mem:value")
  b = new RenderDirective { text = read("mem:directive"); bytes = throw("unused bytes") }
}
hidden renderer = new PropertiesRenderer {
  omitNullProperties = read("mem:omit")
  restrictCharset = read("mem:charset")
  converters = if (read("mem:converters") == "go") new Mapping {
    ["^"] = (v) -> if (read("mem:root") == "go") v else throw("root")
    ["a"] = (v) -> read("mem:converted")
  } else throw("converters")
  convertPropertyTransformers = if (read("mem:transformers") == "go") new Mapping {} else throw("transformers")
}
result = renderer.renderDocument(input)
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
    resume(&mut session, &module, "mem:omit", Value::Bool(true));
    resume(&mut session, &module, "mem:charset", Value::Bool(false));
    resume(&mut session, &module, "mem:converters", Value::string("go"));
    resume(
        &mut session,
        &module,
        "mem:transformers",
        Value::string("go"),
    );
    resume(&mut session, &module, "mem:root", Value::string("go"));
    resume(
        &mut session,
        &module,
        "mem:value",
        Value::string("original"),
    );
    let converter = read(&mut session, &module, "mem:converted");
    session
        .reenter(&converter, entries[2], vec![module.clone()], 0, None)
        .unwrap();
    resume(&mut session, &module, "mem:nested", Value::string("nested"));
    assert_eq!(ready(&mut session, &module), Ok(Value::string("nested")));
    assert_eq!(read(&mut session, &module, "mem:converted"), converter);
    session
        .resume(&converter, Ok(Value::string("hello!")), 0)
        .unwrap();
    resume(
        &mut session,
        &module,
        "mem:directive",
        Value::string("RAW:=\n"),
    );
    let rendered = ready(&mut session, &module).unwrap();
    assert_eq!(rendered, Value::string("a = hello\\!\nb = RAW:=\n\n"));
    assert_eq!(read(&mut session, &module, "mem:parent"), parent);
    session.resume(&parent, Ok(Value::Null), 0).unwrap();
    assert_eq!(ready(&mut session, &module), Ok(Value::Null));
    session.close();
    assert_eq!(session.collect(&[]).slots, 0);
    assert_eq!(rendered, Value::string("a = hello\\!\nb = RAW:=\n\n"));
}

#[test]
fn converted_path_keys_force_late_and_restart_cleanly_after_host_failure() {
    for directive in [false, true] {
        let key = if directive {
            r#"new RenderDirective { text = read("mem:key"); bytes = throw("unused") }"#
        } else {
            r#"new Dynamic { n = read("mem:key") }"#
        };
        let source = format!(
            r#"
hidden input = new Mapping {{ ["key"] = read("mem:value") }}
hidden renderer = new PropertiesRenderer {{
  converters {{ [String] = (v) -> if (v == "key") {key} else v }}
}}
result = renderer.renderDocument(input)
parent = read("mem:parent")
"#
        );
        let (mut session, module, entries) = setup(&source, &["parent", "result"]);
        session
            .start(entries[0], vec![module.clone()], 0, None)
            .unwrap();
        let parent = read(&mut session, &module, "mem:parent");
        session
            .reenter(&parent, entries[1], vec![module.clone()], 0, None)
            .unwrap();
        resume(&mut session, &module, "mem:value", Value::string("value"));
        let key = read(&mut session, &module, "mem:key");
        session
            .resume(
                &key,
                Err(RuntimeError::new(ErrorKind::Host, "retry path key")),
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
        // The source Mapping value is cached; conversion and key rendering restart.
        resume(
            &mut session,
            &module,
            "mem:key",
            if directive {
                Value::string("raw:key")
            } else {
                Value::Int(7.into())
            },
        );
        assert_eq!(
            ready(&mut session, &module),
            Ok(Value::string(if directive {
                "raw:key = value\n"
            } else {
                "new\\ Dynamic\\ {\\ n\\ \\=\\ 7\\ } = value\n"
            }))
        );
        session.resume(&parent, Ok(Value::Null), 0).unwrap();
        assert_eq!(ready(&mut session, &module), Ok(Value::Null));
        session.close();
        assert_eq!(session.collect(&[]).slots, 0);
    }
}

#[test]
fn property_annotation_nested_renderer_and_dynamic_listing_conversion_keep_roots() {
    let source = r#"
class Tag extends ConvertProperty {
  render = (p, renderer) -> Pair("renamed", new RenderDirective {
    text = (renderer as ValueRenderer).renderValue(read("mem:annotation"))
    bytes = throw("unused")
  })
}
class Holder { @Tag {} value: String = read("mem:member") }
result = new PropertiesRenderer {}.renderDocument(new Holder {})
wrapped = new PropertiesRenderer {
  converters { [Listing] = (v) -> if (read("mem:listing") == "go") "wrapped" else throw("listing") }
}.renderDocument(new Dynamic { child = new Dynamic { read("mem:unvisited-element") } })
parent = read("mem:parent")
"#;
    let (mut session, module, entries) = setup(source, &["parent", "result", "wrapped"]);
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
        "mem:member",
        Value::string("original"),
    );
    let annotation = read(&mut session, &module, "mem:annotation");
    session
        .reenter(&annotation, entries[2], vec![module.clone()], 0, None)
        .unwrap();
    resume(&mut session, &module, "mem:listing", Value::string("go"));
    assert_eq!(
        ready(&mut session, &module),
        Ok(Value::string("child = wrapped\n"))
    );
    assert_eq!(read(&mut session, &module, "mem:annotation"), annotation);
    session
        .resume(&annotation, Ok(Value::string("nested!")), 0)
        .unwrap();
    // Properties uses the original path even though the annotation renames the pair key.
    assert_eq!(
        ready(&mut session, &module),
        Ok(Value::string("value = nested\\!\n"))
    );
    session.resume(&parent, Ok(Value::Null), 0).unwrap();
    assert_eq!(ready(&mut session, &module), Ok(Value::Null));
    session.close();
    assert_eq!(session.collect(&[]).slots, 0);
}

#[test]
fn pending_properties_paths_cancel_close_and_timeout_without_retaining_gc_roots() {
    let sources = [
        r#"result = new PropertiesRenderer { omitNullProperties = read("mem:pending") }.renderValue("x")"#,
        r#"result = new PropertiesRenderer { restrictCharset = read("mem:pending") }.renderValue("x")"#,
        r#"result = new PropertiesRenderer { converters { ["^"] = (v) -> read("mem:pending") } }.renderValue("x")"#,
        r#"result = new PropertiesRenderer {}.renderDocument(new Dynamic { a = read("mem:pending") })"#,
        r#"result = new PropertiesRenderer { converters { ["a"] = (v) -> read("mem:pending") } }.renderDocument(new Dynamic { a = 1 })"#,
        r#"result = new PropertiesRenderer { converters { [String] = (v) -> if (v == "key") new Dynamic { n = read("mem:pending") } else v } }.renderDocument(new Mapping { ["key"] = 1 })"#,
        r#"result = new PropertiesRenderer { converters { [String] = (v) -> if (v == "key") new RenderDirective { text = read("mem:pending") } else v } }.renderDocument(new Mapping { ["key"] = 1 })"#,
        r#"result = new PropertiesRenderer {}.renderDocument(new Dynamic { a = new RenderDirective { text = read("mem:pending") } })"#,
        r#"result = new PropertiesRenderer { converters { [Listing] = (v) -> read("mem:pending") } }.renderDocument(new Dynamic { a = new Dynamic { 1 } })"#,
        r#"class Tag extends ConvertProperty { render = (p, renderer) -> Pair(p.key, (renderer as ValueRenderer).renderValue(read("mem:pending"))) }
class Holder { @Tag {} a: Int = 1 }
result = new PropertiesRenderer {}.renderDocument(new Holder {})"#,
    ];
    for reason in [ErrorKind::Cancelled, ErrorKind::Closed, ErrorKind::Timeout] {
        for source in sources {
            let (mut session, module, entries) = setup(source, &["result"]);
            session
                .start(entries[0], vec![module.clone()], 0, Some(10))
                .unwrap();
            let pending = read(&mut session, &module, "mem:pending");
            match reason {
                ErrorKind::Cancelled => session.cancel(),
                ErrorKind::Closed => session.close(),
                ErrorKind::Timeout => (),
                _ => unreachable!(),
            }
            let now = if reason == ErrorKind::Timeout { 11 } else { 1 };
            let HostPoll::Ready(Err(error)) = event(&mut session, &[], now) else {
                panic!("terminal renderer error");
            };
            assert_eq!(error.kind, reason);
            assert!(session.resume(&pending, Ok(Value::Null), now).is_err());
            assert_eq!(
                session
                    .start(entries[0], vec![module], now, None)
                    .unwrap_err()
                    .kind,
                ErrorKind::Closed
            );
            assert_eq!(session.collect(&[]).slots, 0);
        }
    }
}
