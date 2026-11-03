use super::*;
use crate::logging::EvaluationContext;
use crate::parser::parse_module;
use crate::resolver::resolve_module;
use crate::value::RuntimeSource;
use crate::vm::{Vm, VmPoll, compile_module};
use std::sync::Arc;

#[test]
fn module_key_resolution_is_generic_and_preserves_already_parsed_escapes() {
    // IoUtils.resolve(URI, URI): module keys are not limited by the bounded
    // default resource reader's file-only relative resolution profile.
    for (base, operand, expected) in [
        (
            "memo:/selected/holder.pkl",
            "./child.pkl",
            "memo:/selected/child.pkl",
        ),
        (
            "memo:/selected/holder.pkl",
            "./a%20b.pkl",
            "memo:/selected/a%20b.pkl",
        ),
        (
            "https://example.test",
            "child.pkl",
            "https://example.test/child.pkl",
        ),
        (
            "memo:/selected/holder.pkl",
            "/a/../b.pkl",
            "memo:/a/../b.pkl",
        ),
        (
            "memo:/selected/holder.pkl?q",
            "#part",
            "memo:/selected/holder.pkl?q#part",
        ),
        (
            "jar:file:///archive.jar!/dir/holder.pkl",
            "../child.pkl",
            "jar:file:///archive.jar!/child.pkl",
        ),
        ("pkl:base", "child.pkl", "child.pkl"),
    ] {
        assert_eq!(
            resolve_module_key_uri(&operand.into(), &base.into()).unwrap(),
            expected
        );
    }
    assert_eq!(
        prepare_module_uri(
            &"./a b.pkl".into(),
            Some(&"memo:/selected/holder.pkl".into())
        )
        .unwrap()
        .0,
        "memo:/selected/a%20b.pkl"
    );
    assert!(
        ResourceUri::from_authored(&"child.pkl".into())
            .unwrap()
            .resolve(Some(&"memo:/selected/holder.pkl".into()))
            .is_err()
    );
}

// Pinned IoUtils/java.net.URI observations from Stage38's Java acquisition.
#[test]
fn authored_uri_path_escaping_and_normalization_are_separate() {
    let base = "file:///fixture/base%20dir/modules/module.pkl";
    for (authored, expected) in [
        ("life:/a/../text", "life:/text"),
        ("life:a/../text", "life:a/../text"),
        ("life:/a/%2E%2E/text", "life:/a/%2E%2E/text"),
        ("life:\\text", "life:%5Ctext"),
        (
            "../data/a b.txt",
            "file:///fixture/base%20dir/data/a%20b.txt",
        ),
        (
            "../data/a%20b.txt",
            "file:///fixture/base%20dir/data/a%2520b.txt",
        ),
        ("", "file:///fixture/base%20dir/modules/"),
        ("file:///data/a/../b", "file:/data/b"),
        ("a#b", "file:///fixture/base%20dir/modules/a%23b"),
        ("a?b", "file:///fixture/base%20dir/modules/a%3Fb"),
        ("life:/a//b/../c", "life:/a/c"),
        ("life:/../x", "life:/../x"),
    ] {
        let actual = ResourceUri::from_authored(&authored.into())
            .unwrap()
            .resolve(Some(&base.into()))
            .unwrap()
            .normalize();
        assert_eq!(actual.text, expected, "{authored}");
    }
}

#[test]
fn malformed_uri_precedes_policy_and_missing_uses_authored_operand() {
    for (authored, reason) in [
        ("life:bad space", "Illegal character in opaque part"),
        ("life:bad%", "Malformed escape pair"),
        ("life:bad%ZZ", "Malformed escape pair"),
        ("1life:text", "Illegal character in scheme name"),
        ("_life:text", "Illegal character in scheme name"),
        ("env:", "Expected scheme-specific part"),
        (
            "http://[invalid",
            "Expected closing bracket for IPv6 address",
        ),
    ] {
        let error = default_result(&request(authored)).unwrap_err();
        assert_eq!(
            error.message,
            format!("Resource URI `{authored}` has invalid syntax.")
        );
        assert_eq!(
            error.hint.as_ref().and_then(|text| text.as_str()),
            Some(reason)
        );
    }
    for authored in ["prop:/a/../missing", "env:PATH", "prop:java.version"] {
        assert_eq!(
            default_result(&request(authored)).unwrap_err().message,
            format!("Cannot find resource `{authored}`.")
        );
    }
    for authored in [
        "ENV:PATH",
        "envx:PATH",
        "http://localhost/resource",
        "another+scheme:/a/../b",
    ] {
        let error = default_result(&request(authored)).unwrap_err();
        assert_eq!(error.kind, ErrorKind::Host);
        assert!(error.message.starts_with("Refusing to read resource `"));
    }
    assert_eq!(
        default_result(&request("https://localhost/resource"))
            .unwrap_err()
            .kind,
        ErrorKind::Unsupported
    );
    let mut missing = request("env:PATH");
    missing.nullable = true;
    assert_eq!(default_result(&missing), Ok(Value::Null));
    missing.uri = "unknown:resource".into();
    assert_eq!(default_result(&missing).unwrap_err().kind, ErrorKind::Host);
    for authored in ["http:opaque", "http://bad_host/x", "http://host:bad/x"] {
        assert_eq!(
            default_result(&request(authored)).unwrap_err().kind,
            ErrorKind::Unsupported
        );
    }
    for authored in ["http://host:70000/x", "http://host/x#frag"] {
        assert!(
            default_result(&request(authored))
                .unwrap_err()
                .message
                .starts_with("Refusing to read resource `")
        );
    }
}

fn request(uri: &str) -> ReadRequest {
    ReadRequest {
        id: 1,
        uri: uri.into(),
        resolved_uri: None,
        module_uri: Some("file:///test/module.pkl".into()),
        nullable: false,
    }
}

fn event(vm: &mut Vm, root: &[Value]) -> VmPoll {
    for _ in 0..100_000 {
        match vm.poll(1) {
            VmPoll::Yielded => {
                vm.collect(root);
            }
            event => return event,
        }
    }
    panic!("VM did not settle")
}

#[test]
fn denied_forcing_retries_after_gc_and_custom_host_can_still_complete() {
    let text = "obj=new Dynamic { x=read(\"unknown:resource\"); y=7 }\nresult=trace(obj).x\n";
    let program = compile_module(&resolve_module(&parse_module(text).unwrap()).unwrap()).unwrap();
    let entry = program.entry;
    let mut vm = Vm::with_context(
        program,
        EvaluationContext {
            primary_source: Some(Arc::new(RuntimeSource {
                text: text.into(),
                uri: "file:///primary.pkl".into(),
                module_name: "primary".into(),
            })),
            ..Default::default()
        },
    );
    vm.start(entry, vec![]).unwrap();
    let VmPoll::Ready(Ok(module)) = event(&mut vm, &[]) else {
        panic!("setup failed")
    };
    let roots = [module];
    let mut first = None;
    let mut last_request = 0;
    for attempt in 0..3 {
        vm.start_member(roots[0].as_object().unwrap(), "result")
            .unwrap();
        let VmPoll::Pending(request) = event(&mut vm, &roots) else {
            panic!("missing read on attempt {attempt}")
        };
        assert!(request.id > last_request);
        last_request = request.id;
        assert_eq!(request.uri, "unknown:resource");
        assert_eq!(request.module_uri, Some("file:///primary.pkl".into()));
        vm.collect(&roots);
        if attempt == 2 {
            vm.resume(request.id, Ok(Value::string("custom Host")))
                .unwrap();
            assert_eq!(
                event(&mut vm, &roots),
                VmPoll::Ready(Ok(Value::string("custom Host")))
            );
        } else {
            vm.resume(request.id, default_result(&request)).unwrap();
            let VmPoll::Ready(Err(error)) = event(&mut vm, &roots) else {
                panic!("read denial not propagated")
            };
            assert!(!error.frames.is_empty());
            if let Some(first) = &first {
                assert_eq!(first, &error);
            } else {
                first = Some(error);
            }
        }
    }
    vm.start_member(roots[0].as_object().unwrap(), "result")
        .unwrap();
    assert_eq!(
        event(&mut vm, &roots),
        VmPoll::Ready(Ok(Value::string("custom Host")))
    );
    vm.close();
    assert_eq!(vm.heap_stats().slots, 0);
}

#[test]
fn pending_read_uses_declaration_source_and_keeps_custom_host_operand_raw() {
    let text = "function foreign()=read(\"../data/a%20b.txt\")\nresult=foreign()\n";
    let mut program =
        compile_module(&resolve_module(&parse_module(text).unwrap()).unwrap()).unwrap();
    let foreign = Arc::new(RuntimeSource {
        text: text.into(),
        uri: "file:///foreign/module.pkl".into(),
        module_name: "foreign".into(),
    });
    let mutable = Arc::make_mut(&mut program);
    for (block, metadata) in mutable.blocks.iter().zip(mutable.metadata.iter_mut()) {
        if block
            .iter()
            .any(|instruction| matches!(instruction, crate::vm::Instruction::ReadResource))
        {
            metadata.source = Some(foreign.clone());
        }
    }
    let entry = program.entry;
    let mut vm = Vm::with_context(
        program,
        EvaluationContext {
            primary_source: Some(Arc::new(RuntimeSource {
                text: text.into(),
                uri: "file:///caller/module.pkl".into(),
                module_name: "caller".into(),
            })),
            ..Default::default()
        },
    );
    vm.start(entry, vec![]).unwrap();
    let VmPoll::Ready(Ok(module)) = event(&mut vm, &[]) else {
        panic!("setup failed")
    };
    vm.start_member(module.as_object().unwrap(), "result")
        .unwrap();
    let VmPoll::Pending(request) = event(&mut vm, &[module]) else {
        panic!("missing read")
    };
    assert_eq!(request.uri, "../data/a%20b.txt");
    assert_eq!(
        request.module_uri,
        Some("file:///foreign/module.pkl".into())
    );
    vm.close();
}

#[test]
fn default_file_reader_validates_decoded_ssp_before_unimplemented_io() {
    let reason = "File URIs must have a path that starts with `/` (e.g. file:/path/to/my_module.pkl).\nTo resolve relative paths, remove the scheme prefix (remove \"file:\").";
    for authored in [
        "file:path/to/foo.txt",
        "file:%70ath/to/foo.txt",
        "file:%252Ftmp",
    ] {
        let error = default_result(&request(authored)).unwrap_err();
        assert_eq!(error.kind, ErrorKind::Host);
        assert!(error.host_language_exception);
        assert_eq!(
            error.message,
            format!("Resource URI `{authored}` has invalid syntax.")
        );
        assert_eq!(
            error.hint.as_ref().and_then(PklString::as_str),
            Some(reason)
        );
        assert!(error.foreign_exception.is_none());
    }
    // IoUtils validates decoded scheme-specific text exactly once. These pass
    // that check but remain explicit missing-I/O capabilities in this helper.
    for authored in [
        "file:/tmp",
        "file:///tmp",
        "file:%2Ftmp",
        "file:%2ftmp#fragment",
    ] {
        let error = default_result(&request(authored)).unwrap_err();
        assert_eq!(error.kind, ErrorKind::Unsupported);
        assert!(error.hint.is_none());
    }
    let malformed = default_result(&request("file:bad%zz")).unwrap_err();
    assert_eq!(
        malformed.hint.as_ref().and_then(PklString::as_str),
        Some("Malformed escape pair")
    );
    let denied = default_result(&request("unknown:resource")).unwrap_err();
    assert!(denied.message.starts_with("Refusing to read resource `"));
    assert!(denied.hint.is_none());
}

#[test]
fn file_uri_reader_failure_prevents_later_reads_and_preserves_custom_host_boundary() {
    let text = "result = read(\"file:path/to/foo.txt\") + read(\"prop:later\")\n";
    let program = compile_module(&resolve_module(&parse_module(text).unwrap()).unwrap()).unwrap();
    let entry = program.entry;
    let mut vm = Vm::with_context(
        program,
        EvaluationContext {
            primary_source: Some(Arc::new(RuntimeSource {
                text: text.into(),
                uri: "file:///stage150/main.pkl".into(),
                module_name: "stage150".into(),
            })),
            ..Default::default()
        },
    );
    vm.start(entry, vec![]).unwrap();
    let VmPoll::Ready(Ok(module)) = event(&mut vm, &[]) else {
        panic!("initial module")
    };
    let roots = [module];
    let mut previous = 0;
    for _ in 0..2 {
        vm.start_member(roots[0].as_object().unwrap(), "result")
            .unwrap();
        let VmPoll::Pending(read) = event(&mut vm, &roots) else {
            panic!("file reader request")
        };
        assert_eq!(read.uri, "file:path/to/foo.txt");
        assert!(read.id > previous);
        previous = read.id;
        vm.collect(&roots);
        vm.resume(read.id, default_result(&read)).unwrap();
        let VmPoll::Ready(Err(error)) = event(&mut vm, &roots) else {
            panic!("file validation must precede prop:later")
        };
        assert_eq!(error.kind, ErrorKind::Host);
        assert!(error.host_language_exception);
        assert_eq!(
            error.message,
            "Resource URI `file:path/to/foo.txt` has invalid syntax."
        );
        assert!(!error.frames.is_empty());
    }
    // Default-reader validation is not a blanket URI parser restriction: a
    // caller-supplied Host can still fulfill the same typed request explicitly.
    vm.start_member(roots[0].as_object().unwrap(), "result")
        .unwrap();
    let VmPoll::Pending(read) = event(&mut vm, &roots) else {
        panic!("custom first read")
    };
    vm.resume(read.id, Ok(Value::string("custom"))).unwrap();
    let VmPoll::Pending(read) = event(&mut vm, &roots) else {
        panic!("later read after custom success")
    };
    assert_eq!(read.uri, "prop:later");
    vm.resume(read.id, Ok(Value::string(" later"))).unwrap();
    assert_eq!(
        event(&mut vm, &roots),
        VmPoll::Ready(Ok(Value::string("custom later")))
    );
    vm.close();
    assert_eq!(vm.collect(&[]).slots, 0);
}
