//! Host/GC and continuation ownership checks, separate from Java compatibility rows.
use pkl_runtime::{
    host::{HostPoll, HostSession},
    parser::parse_module,
    resolver::resolve_module,
    value::{ErrorKind, RuntimeError, Value},
    vm::{Instruction as I, compile_module},
};
use std::sync::Arc;
fn event(session: &mut HostSession) -> HostPoll {
    for _ in 0..200_000 {
        session.collect(&[]);
        match session.poll(0, 1) {
            HostPoll::Yielded => {}
            event => return event,
        }
    }
    panic!("conversion Host poll bound")
}
fn ready(session: &mut HostSession) -> Result<Value, RuntimeError> {
    match event(session) {
        HostPoll::Ready(value) => value,
        event => panic!("unexpected {event:?}"),
    }
}
fn read(session: &mut HostSession, expected: &str) -> String {
    match event(session) {
        HostPoll::Read { token, uri } => {
            assert_eq!(&*uri, expected);
            token
        }
        event => panic!("unexpected {event:?}"),
    }
}
fn setup(source: &str, names: &[&str]) -> (HostSession, Value, Vec<usize>) {
    let mut program = (*compile_module(&resolve_module(&parse_module(source).unwrap()).unwrap())
        .unwrap())
    .clone();
    let entries = names
        .iter()
        .map(|name| {
            let index = program.blocks.len();
            program
                .blocks
                .push(vec![I::Input(0), I::GetMember((*name).into()), I::Return]);
            index
        })
        .collect();
    let entry = program.entry;
    let mut session = HostSession::new(Arc::new(program));
    session.start(entry, vec![], 0, None).unwrap();
    let module = ready(&mut session).unwrap();
    (session, module, entries)
}
const SOURCE: &str = r#"
class Key { text: String }
parent = read("mem:outer")
selected = List(1,2).toMap(
  (n) -> if (read("mem:key/\(n)") == "ok") new Key { text = read("mem:hash/\(n)") } else throw("key"),
  (n) -> if (read("mem:value/\(n)") == "ok") n else throw("value")
).values.toBytes()
setAdded = Set(new Key { text = "old" }).add(new Key { text = read("mem:set") }).length
converted = (new Dynamic { a = read("mem:parent"); "ignored" }) { b = read("mem:child") }.toMap()["b"]
bytesFor = new Listing { for (b in Bytes(128,255)) { if (read("mem:byte/\(b)") == "ok") b else 0 } }.toList()
bytesSpread = new Listing { ...Bytes(128,255) }.toList().toBytes()
defaultLookup = new Mapping { default = (key) -> read("mem:lookup-default") }.getOrDefault("missing")
hashLookup = new Mapping { [new Key { text = read("mem:lookup-key") }] = read("mem:lookup-value") }.getOrNull(new Key { text = "key" })
"#;
#[test]
fn selector_callbacks_and_key_hashes_survive_gc_reentry_and_retry() {
    let (mut session, module, entries) = setup(SOURCE, &["parent", "selected", "bytesSpread"]);
    session
        .start(entries[0], vec![module.clone()], 0, None)
        .unwrap();
    let outer = read(&mut session, "mem:outer");
    session
        .reenter(&outer, entries[1], vec![module.clone()], 0, None)
        .unwrap();
    for phase in ["key", "value"] {
        let token = read(&mut session, &format!("mem:{phase}/1"));
        session.resume(&token, Ok(Value::string("ok")), 0).unwrap();
    }
    let hash = read(&mut session, "mem:hash/1");
    session
        .reenter(&hash, entries[2], vec![module.clone()], 0, None)
        .unwrap();
    assert_eq!(ready(&mut session), Ok(Value::Bytes([128, 255].into())));
    assert_eq!(read(&mut session, "mem:hash/1"), hash);
    session
        .resume(
            &hash,
            Err(RuntimeError::new(ErrorKind::Host, "retry key hash")),
            0,
        )
        .unwrap();
    assert_eq!(ready(&mut session).unwrap_err().kind, ErrorKind::Host);
    assert_eq!(read(&mut session, "mem:outer"), outer);
    session
        .reenter(&outer, entries[1], vec![module], 0, None)
        .unwrap();
    for n in 1..=2 {
        for phase in ["key", "value", "hash"] {
            let token = read(&mut session, &format!("mem:{phase}/{n}"));
            let value = if phase == "hash" {
                format!("key{n}")
            } else {
                "ok".into()
            };
            session.resume(&token, Ok(Value::string(value)), 0).unwrap();
        }
    }
    assert_eq!(ready(&mut session), Ok(Value::Bytes([1, 2].into())));
    assert_eq!(read(&mut session, "mem:outer"), outer);
    session.resume(&outer, Ok(Value::Null), 0).unwrap();
    assert_eq!(ready(&mut session), Ok(Value::Null));
    session.close();
    assert_eq!(session.collect(&[]).slots, 0);
}
#[test]
fn child_first_shallow_forcing_and_set_add_hashing_resume() {
    let (mut session, module, entries) = setup(SOURCE, &["converted", "setAdded"]);
    session
        .start(entries[0], vec![module.clone()], 0, None)
        .unwrap();
    let child = read(&mut session, "mem:child");
    session
        .resume(&child, Ok(Value::string("child")), 0)
        .unwrap();
    let parent = read(&mut session, "mem:parent");
    session
        .resume(&parent, Ok(Value::string("parent")), 0)
        .unwrap();
    assert_eq!(ready(&mut session), Ok(Value::string("child")));
    session.start(entries[1], vec![module], 0, None).unwrap();
    let hash = read(&mut session, "mem:set");
    session.resume(&hash, Ok(Value::string("new")), 0).unwrap();
    assert_eq!(ready(&mut session), Ok(Value::Int(2.into())));
    session.close();
    assert_eq!(session.collect(&[]).slots, 0);
}
#[test]
fn lazy_adapters_retain_source_without_creator_and_cache_successful_reads() {
    for (expression, properties) in [
        (
            "(new Data { value = read(\"mem:delegate\") }).toDynamic()",
            vec!["value"],
        ),
        (
            "(new Dynamic { value = read(\"mem:delegate\") }).toTyped(Data)",
            vec!["value"],
        ),
        (
            "Map(\"child\", new Data { value = read(\"mem:delegate\") }).toTyped(Box)",
            vec!["child", "value"],
        ),
    ] {
        let source = format!(
            "class Data {{ value: String }}\nclass Box {{ child: Data }}\nresult = {expression}"
        );
        let mut program =
            (*compile_module(&resolve_module(&parse_module(&source).unwrap()).unwrap()).unwrap())
                .clone();
        let module_entry = program.entry;
        let conversion = program.blocks.len();
        program
            .blocks
            .push(vec![I::Input(0), I::GetMember("result".into()), I::Return]);
        let lookup = program.blocks.len();
        let mut code = vec![I::Input(0)];
        code.extend(properties.into_iter().map(|name| I::GetMember(name.into())));
        code.push(I::Return);
        program.blocks.push(code);
        let mut session = HostSession::new(Arc::new(program));
        session.start(module_entry, vec![], 0, None).unwrap();
        let module = ready(&mut session).unwrap();
        session.start(conversion, vec![module], 0, None).unwrap();
        let wrapper = ready(&mut session).unwrap();
        session.collect(std::slice::from_ref(&wrapper));
        session
            .start(lookup, vec![wrapper.clone()], 0, None)
            .unwrap();
        let first = read(&mut session, "mem:delegate");
        session
            .resume(
                &first,
                Err(RuntimeError::new(ErrorKind::Host, "retry delegate")),
                0,
            )
            .unwrap();
        assert_eq!(ready(&mut session).unwrap_err().kind, ErrorKind::Host);
        session.collect(std::slice::from_ref(&wrapper));
        session
            .start(lookup, vec![wrapper.clone()], 0, None)
            .unwrap();
        let second = read(&mut session, "mem:delegate");
        assert_ne!(first, second);
        session
            .resume(&second, Ok(Value::string("ready")), 0)
            .unwrap();
        assert_eq!(ready(&mut session), Ok(Value::string("ready")));
        session.collect(std::slice::from_ref(&wrapper));
        session.start(lookup, vec![wrapper], 0, None).unwrap();
        assert_eq!(ready(&mut session), Ok(Value::string("ready")));
        session.close();
        assert_eq!(session.collect(&[]).slots, 0);
    }
}
#[test]
fn pending_conversion_and_iterator_work_cancels_or_closes_without_roots() {
    for close in [false, true] {
        for (member, uri) in [
            ("selected", "mem:key/1"),
            ("converted", "mem:child"),
            ("setAdded", "mem:set"),
            ("bytesFor", "mem:byte/-128"),
            ("defaultLookup", "mem:lookup-default"),
            ("hashLookup", "mem:lookup-key"),
        ] {
            let (mut session, module, entries) = setup(SOURCE, &[member]);
            session.start(entries[0], vec![module], 0, None).unwrap();
            let token = read(&mut session, uri);
            if close {
                session.close();
            } else {
                session.cancel();
            }
            assert_eq!(
                ready(&mut session).unwrap_err().kind,
                if close {
                    ErrorKind::Closed
                } else {
                    ErrorKind::Cancelled
                }
            );
            assert_eq!(session.collect(&[]).slots, 0);
            assert_eq!(
                session.resume(&token, Ok(Value::Null), 0).unwrap_err().kind,
                ErrorKind::Closed
            );
        }
    }
}

#[test]
fn lazy_lookup_defaults_and_cached_string_entries_survive_gc_and_retry() {
    let source = r#"
class Data { name: String? = "base"; greeting: String = if (this.name == null) "unexpected" else read("mem:prototype") }
prototype = new Dynamic { name = null }.toTyped(Data).greeting
explicitNull = Map("name", null).toTyped(Data).name
lookup = new Mapping { default = (key) -> read("mem:default/\(key)"); ["present"] = read("mem:present") }
listing = new Listing { default = (index) -> read("mem:index/\(index)"); read("mem:element") }
entry = new Dynamic { ["same"] = read("mem:entry") }
selected = lookup.getOrDefault("absent")
present = lookup.getOrNull("present")
index = listing.getOrDefault(-1)
element = listing.getOrNull(0)
before = entry.hasProperty("same")
forced = entry["same"]
after = entry.hasProperty("same")
"#;
    let (mut session, module, entries) = setup(
        source,
        &[
            "prototype",
            "explicitNull",
            "selected",
            "present",
            "index",
            "element",
            "before",
            "forced",
            "after",
        ],
    );
    session
        .start(entries[1], vec![module.clone()], 0, None)
        .unwrap();
    assert_eq!(ready(&mut session), Ok(Value::Null));
    session
        .start(entries[6], vec![module.clone()], 0, None)
        .unwrap();
    assert_eq!(ready(&mut session), Ok(Value::Bool(false)));
    for (entry, uri) in [
        (0, "mem:prototype"),
        (2, "mem:default/absent"),
        (3, "mem:present"),
        (4, "mem:index/-1"),
        (5, "mem:element"),
        (7, "mem:entry"),
    ] {
        session
            .start(entries[entry], vec![module.clone()], 0, None)
            .unwrap();
        let first = read(&mut session, uri);
        session
            .resume(
                &first,
                Err(RuntimeError::new(ErrorKind::Host, "retry lookup")),
                0,
            )
            .unwrap();
        assert_eq!(ready(&mut session).unwrap_err().kind, ErrorKind::Host);
        session.collect(std::slice::from_ref(&module));
        session
            .start(entries[entry], vec![module.clone()], 0, None)
            .unwrap();
        let second = read(&mut session, uri);
        session
            .resume(&second, Ok(Value::string("ready")), 0)
            .unwrap();
        assert_eq!(ready(&mut session), Ok(Value::string("ready")));
    }
    session.start(entries[8], vec![module], 0, None).unwrap();
    assert_eq!(ready(&mut session), Ok(Value::Bool(true)));
    session.close();
    assert_eq!(session.collect(&[]).slots, 0);
}

#[test]
fn lookup_key_equality_and_member_read_resume_with_live_source() {
    let (mut session, module, entries) = setup(SOURCE, &["hashLookup"]);
    session.start(entries[0], vec![module], 0, None).unwrap();
    let key = read(&mut session, "mem:lookup-key");
    session.resume(&key, Ok(Value::string("key")), 0).unwrap();
    let value = read(&mut session, "mem:lookup-value");
    session
        .resume(&value, Ok(Value::string("found")), 0)
        .unwrap();
    assert_eq!(ready(&mut session), Ok(Value::string("found")));
    session.close();
    assert_eq!(session.collect(&[]).slots, 0);
}

#[test]
fn inherited_default_type_failure_retries_without_publishing_bad_value() {
    let source = r#"
operator = (key) -> key
result = new Mapping { default = read("mem:default-type") }.getOrDefault("answer")
"#;
    let (mut session, module, entries) = setup(source, &["operator", "result"]);
    session
        .start(entries[0], vec![module.clone()], 0, None)
        .unwrap();
    let operator = ready(&mut session).unwrap();
    session
        .start(entries[1], vec![module.clone()], 0, None)
        .unwrap();
    let bad = read(&mut session, "mem:default-type");
    session.resume(&bad, Ok(Value::Int(1.into())), 0).unwrap();
    assert_eq!(ready(&mut session).unwrap_err().kind, ErrorKind::Type);
    session.collect(&[module.clone(), operator.clone()]);
    session.start(entries[1], vec![module], 0, None).unwrap();
    let good = read(&mut session, "mem:default-type");
    session.resume(&good, Ok(operator), 0).unwrap();
    assert_eq!(ready(&mut session), Ok(Value::string("answer")));
    session.close();
    assert_eq!(session.collect(&[]).slots, 0);
}

#[test]
fn authored_nul_and_surrogate_properties_keep_distinct_caches_across_host_reentry() {
    let source = r#"
class Box { value: String }
result = (Map("\u{d800}", new Box { value = read("mem:high") }).toDynamic()) {
  `<NUL>property:[55296]` = new Box { value = read("mem:nul") }
}
high = result.getProperty("\u{d800}").value
nul = result.getProperty("\u{0}property:[55296]").value
"#
    .replace("<NUL>", "\0");
    let (mut session, module, entries) = setup(&source, &["high", "nul"]);
    session
        .start(entries[0], vec![module.clone()], 0, None)
        .unwrap();
    let high = read(&mut session, "mem:high");
    session
        .reenter(&high, entries[1], vec![module.clone()], 0, None)
        .unwrap();
    let nul = read(&mut session, "mem:nul");
    session
        .resume(&nul, Ok(Value::string("NUL value")), 0)
        .unwrap();
    assert_eq!(ready(&mut session), Ok(Value::string("NUL value")));
    assert_eq!(read(&mut session, "mem:high"), high);
    session
        .resume(
            &high,
            Err(RuntimeError::new(ErrorKind::Host, "retry high property")),
            0,
        )
        .unwrap();
    assert_eq!(ready(&mut session).unwrap_err().kind, ErrorKind::Host);
    session.collect(std::slice::from_ref(&module));
    session
        .start(entries[0], vec![module.clone()], 0, None)
        .unwrap();
    let high = read(&mut session, "mem:high");
    session
        .resume(&high, Ok(Value::string("surrogate value")), 0)
        .unwrap();
    assert_eq!(ready(&mut session), Ok(Value::string("surrogate value")));
    session.collect(std::slice::from_ref(&module));
    session.start(entries[1], vec![module], 0, None).unwrap();
    assert_eq!(ready(&mut session), Ok(Value::string("NUL value")));
    session.close();
    assert_eq!(session.collect(&[]).slots, 0);
}
