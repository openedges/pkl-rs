//! Internal Host/GC contracts for shared YAML rendering. These authored
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
    panic!("YAML renderer Host poll bound")
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
fn options_mapping_root_member_and_directive_survive_reentry_and_gc() {
    let source = r#"
hidden input = new Dynamic { a=read("mem:value"); b=new RenderDirective { text=read("mem:directive"); bytes=throw("unused") } }
hidden renderer = new YamlRenderer {
 mode=read("mem:mode"); indentWidth=read("mem:indent"); omitNullProperties=read("mem:omit"); isStream=read("mem:stream")
 converters=if(read("mem:converters")=="go") new Mapping { ["^"]=(v)->if(read("mem:root")=="go") v else throw("root"); ["a"]=(v)->read("mem:converted") } else throw("converters")
 convertPropertyTransformers=if(read("mem:transformers")=="go") new Mapping {} else throw("transformers")
}
result=renderer.renderDocument(input)
parent=read("mem:parent")
nested=read("mem:nested")
"#;
    let (mut s, m, e) = setup(source, &["parent", "result", "nested"]);
    s.start(e[0], vec![m.clone()], 0, None).unwrap();
    let p = read(&mut s, &m, "mem:parent");
    s.reenter(&p, e[1], vec![m.clone()], 0, None).unwrap();
    resume(&mut s, &m, "mem:mode", Value::string("compat"));
    resume(&mut s, &m, "mem:indent", Value::Int(2.into()));
    resume(&mut s, &m, "mem:omit", Value::Bool(true));
    resume(&mut s, &m, "mem:stream", Value::Bool(false));
    for uri in ["mem:converters", "mem:transformers", "mem:root"] {
        resume(&mut s, &m, uri, Value::string("go"));
    }
    resume(&mut s, &m, "mem:value", Value::string("original"));
    let c = read(&mut s, &m, "mem:converted");
    s.reenter(&c, e[2], vec![m.clone()], 0, None).unwrap();
    resume(&mut s, &m, "mem:nested", Value::string("nested"));
    assert_eq!(ready(&mut s, &m), Ok(Value::string("nested")));
    assert_eq!(read(&mut s, &m, "mem:converted"), c);
    s.resume(&c, Ok(Value::string("hello!")), 0).unwrap();
    resume(&mut s, &m, "mem:directive", Value::string(" RAW\n"));
    assert_eq!(ready(&mut s, &m), Ok(Value::string("a: hello!\nb: RAW\n")));
    assert_eq!(read(&mut s, &m, "mem:parent"), p);
    s.resume(&p, Ok(Value::Null), 0).unwrap();
    assert_eq!(ready(&mut s, &m), Ok(Value::Null));
    s.close();
    assert_eq!(s.collect(&[]).slots, 0);
}
#[test]
fn lazy_value_then_key_hash_then_conversion_retry_retains_pending_keys() {
    let source = r#"
hidden input=new Mapping {[new Dynamic {n=read("mem:key")}]=read("mem:value")}
hidden renderer=new YamlRenderer { converters {[Dynamic]=(v)->read("mem:converted-key")} }
result=renderer.renderDocument(input)
parent=read("mem:parent")
"#;
    let (mut s, m, e) = setup(source, &["parent", "result"]);
    s.start(e[0], vec![m.clone()], 0, None).unwrap();
    let p = read(&mut s, &m, "mem:parent");
    s.reenter(&p, e[1], vec![m.clone()], 0, None).unwrap();
    resume(&mut s, &m, "mem:value", Value::string("value"));
    let key = read(&mut s, &m, "mem:key");
    s.resume(
        &key,
        Err(RuntimeError::new(ErrorKind::Host, "retry key")),
        0,
    )
    .unwrap();
    assert_eq!(ready(&mut s, &m).unwrap_err().kind, ErrorKind::Host);
    assert_eq!(read(&mut s, &m, "mem:parent"), p);
    s.reenter(&p, e[1], vec![m.clone()], 0, None).unwrap();
    resume(&mut s, &m, "mem:key", Value::Int(7.into()));
    resume(&mut s, &m, "mem:converted-key", Value::string("safe"));
    assert_eq!(ready(&mut s, &m), Ok(Value::string("safe: value\n")));
    s.resume(&p, Ok(Value::Null), 0).unwrap();
    assert_eq!(ready(&mut s, &m), Ok(Value::Null));
    s.close();
    assert_eq!(s.collect(&[]).slots, 0);
}
#[test]
fn annotation_nested_renderer_and_stream_force_keep_distinct_state() {
    let source = r#"
class Tag extends ConvertProperty {render=(p,r)->Pair("renamed",new RenderDirective {text=(r as ValueRenderer).renderValue(read("mem:annotation"));bytes=throw("unused")})}
class Holder {@Tag {} value:String=read("mem:member")}
result=new YamlRenderer {}.renderDocument(new Holder {})
stream=new YamlRenderer {isStream=true;converters {[Int]=(v)->throw("stream element converter must not run")}}.renderDocument(new Listing {read("mem:first");read("mem:last")})
parent=read("mem:parent")
"#;
    let (mut s, m, e) = setup(source, &["parent", "result", "stream"]);
    s.start(e[0], vec![m.clone()], 0, None).unwrap();
    let p = read(&mut s, &m, "mem:parent");
    s.reenter(&p, e[1], vec![m.clone()], 0, None).unwrap();
    resume(&mut s, &m, "mem:member", Value::string("original"));
    let a = read(&mut s, &m, "mem:annotation");
    s.reenter(&a, e[2], vec![m.clone()], 0, None).unwrap();
    resume(&mut s, &m, "mem:first", Value::Int(1.into()));
    resume(&mut s, &m, "mem:last", Value::Int(2.into()));
    assert_eq!(ready(&mut s, &m), Ok(Value::string("1\n--- 2\n")));
    assert_eq!(read(&mut s, &m, "mem:annotation"), a);
    s.resume(&a, Ok(Value::string("nested!")), 0).unwrap();
    assert_eq!(ready(&mut s, &m), Ok(Value::string("renamed:nested!\n")));
    s.resume(&p, Ok(Value::Null), 0).unwrap();
    assert_eq!(ready(&mut s, &m), Ok(Value::Null));
    s.close();
    assert_eq!(s.collect(&[]).slots, 0);
}
#[test]
fn pending_yaml_continuations_cancel_close_and_timeout_release_all_roots() {
    let sources = [
        r#"result=new YamlRenderer {mode=read("mem:pending")}.renderValue(1)"#,
        r#"result=new YamlRenderer {indentWidth=read("mem:pending")}.renderValue(1)"#,
        r#"result=new YamlRenderer {omitNullProperties=read("mem:pending")}.renderValue(1)"#,
        r#"result=new YamlRenderer {isStream=read("mem:pending")}.renderValue(1)"#,
        r#"result=new YamlRenderer {converters {["^"]=(x)->read("mem:pending")}}.renderValue(1)"#,
        r#"result=new YamlRenderer {}.renderDocument(new Mapping {[new Dynamic {x=read("mem:pending")}]=1})"#,
        r#"result=new YamlRenderer {}.renderDocument(new Mapping {[new RenderDirective {text=read("mem:pending")}]=1})"#,
        r#"result=new YamlRenderer {}.renderDocument(new Dynamic {a=read("mem:pending")})"#,
        r#"result=new YamlRenderer {isStream=true}.renderDocument(new Listing {1;read("mem:pending")})"#,
        r#"result=new YamlRenderer {}.renderDocument(new Dynamic {a=new RenderDirective {text=read("mem:pending")}})"#,
        r#"class Tag extends ConvertProperty {render=(p,r)->Pair(p.first,(r as ValueRenderer).renderValue(read("mem:pending")))}
class Holder {@Tag {} x:Int=1}
result=new YamlRenderer {}.renderDocument(new Holder {})"#,
    ];
    for reason in [ErrorKind::Cancelled, ErrorKind::Closed, ErrorKind::Timeout] {
        for source in sources {
            let (mut s, m, e) = setup(source, &["result"]);
            s.start(e[0], vec![m.clone()], 0, Some(10)).unwrap();
            let t = read(&mut s, &m, "mem:pending");
            match reason {
                ErrorKind::Cancelled => s.cancel(),
                ErrorKind::Closed => s.close(),
                ErrorKind::Timeout => (),
                _ => unreachable!(),
            }
            let now = if reason == ErrorKind::Timeout { 11 } else { 1 };
            let HostPoll::Ready(Err(err)) = event(&mut s, &[], now) else {
                panic!("terminal YAML lifecycle")
            };
            assert_eq!(err.kind, reason);
            assert!(s.resume(&t, Ok(Value::Null), now).is_err());
            assert_eq!(
                s.start(e[0], vec![m], now, None).unwrap_err().kind,
                ErrorKind::Closed
            );
            assert_eq!(s.collect(&[]).slots, 0);
        }
    }
}
