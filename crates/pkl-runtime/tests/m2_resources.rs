//! Resource cache/Host/GC development regressions. Pinned Java acquisition is
//! separate evidence; these tests do not manufacture compatibility expectations.
#[path = "support/module_keys.rs"]
mod module_keys;
use pkl_runtime::{
    host::ReadFailure,
    probe::{SourceEvent, SourceProbe},
    resources::{ReadResult, Resource},
};

fn probe(source: &str, expressions: &[&str]) -> SourceProbe {
    SourceProbe::new_expressions_text(
        source,
        "file:///resources/main.pkl",
        "main",
        &expressions.iter().map(|s| (*s).into()).collect::<Vec<_>>(),
        "compact",
        false,
        false,
        false,
    )
    .unwrap()
}
fn next(probe: &mut SourceProbe) -> SourceEvent {
    for _ in 0..100_000 {
        let event = probe.poll(0, 1).unwrap();
        probe.collect();
        if event.kind == "module-request"
            && module_keys::complete_probe(probe, event.token.as_deref().unwrap(), 0).unwrap()
        {
            continue;
        }
        if event.kind != "yield" {
            return event;
        }
    }
    panic!("resource operation did not settle");
}
fn text(event: SourceEvent) -> String {
    assert_eq!(event.kind, "value", "{event:?}");
    String::from_utf16(&event.value.unwrap()).unwrap()
}

#[test]
fn typed_string_cache_crosses_members_and_calls_but_not_evaluators() {
    for _ in 0..2 {
        let mut probe = probe("a=read(\"life:key\")\nb=read?(\"life:key\")\n", &["a", "b"]);
        probe.start_path(0, 0, None).unwrap();
        let event = next(&mut probe);
        assert_eq!(event.kind, "read");
        probe
            .complete_read(
                event.token.as_deref().unwrap(),
                Ok(ReadResult::Text("cached".into())),
                0,
            )
            .unwrap();
        assert_eq!(text(next(&mut probe)), "cached");
        probe.start_path(1, 0, None).unwrap();
        assert_eq!(text(next(&mut probe)), "cached");
        probe.close();
        assert_eq!(probe.collect().slots, 0);
    }
}

#[test]
fn absent_cache_preserves_each_authored_error_and_nullable_result() {
    let mut probe = probe(
        "import \"pkl:test\"\na=test.catch(() -> read(\"dir/../absent\"))\nb=test.catch(() -> read(\"absent\"))\nc=read?(\"absent\")\n",
        &["a", "b", "c"],
    );
    probe.start_path(0, 0, None).unwrap();
    let event = next(&mut probe);
    assert_eq!(event.kind, "read");
    probe
        .complete_read(event.token.as_deref().unwrap(), Ok(ReadResult::Missing), 0)
        .unwrap();
    assert_eq!(
        text(next(&mut probe)),
        "Cannot find resource `dir/../absent`."
    );
    probe.start_path(1, 0, None).unwrap();
    assert_eq!(text(next(&mut probe)), "Cannot find resource `absent`.");
    probe.start_path(2, 0, None).unwrap();
    assert_eq!(text(next(&mut probe)), "null");
}

#[test]
fn reader_errors_retry_and_invalid_tokens_do_not_publish() {
    let mut probe = probe(
        "a=read?(\"life:retry\")\nb=read(\"life:retry\")\n",
        &["a", "b"],
    );
    probe.start_path(0, 0, None).unwrap();
    let event = next(&mut probe);
    let token = event.token.unwrap();
    assert!(
        probe
            .complete_read("invalid", Ok(ReadResult::Missing), 0)
            .is_err()
    );
    probe
        .complete_read(&token, Err(ReadFailure::Io("retry me".into())), 0)
        .unwrap();
    assert_eq!(next(&mut probe).kind, "error");
    probe.start_path(1, 0, None).unwrap();
    let request = next(&mut probe);
    assert_eq!(request.kind, "read");
    probe
        .complete_read(
            request.token.as_deref().unwrap(),
            Ok(ReadResult::Text("ok".into())),
            0,
        )
        .unwrap();
    assert_eq!(text(next(&mut probe)), "ok");
    probe.start_path(0, 0, None).unwrap();
    assert_eq!(text(next(&mut probe)), "ok");
}

#[test]
fn resource_payload_class_bytes_and_cache_survive_safepoint_collection() {
    let mut probe = probe(
        "import \"pkl:base\" as b\na=read(\"life:resource\")\nother=read(\"life:resource\")\nsameClass = a.getClass() == Resource && Resource == b.Resource\n",
        &[
            "a.uri",
            "a.text",
            "a.base64",
            "sameClass",
            "other.bytes == a.bytes",
            "other.base64",
        ],
    );
    probe.start_path(0, 0, None).unwrap();
    let event = next(&mut probe);
    probe
        .complete_read(
            event.token.as_deref().unwrap(),
            Ok(ReadResult::Resource(Resource {
                uri: "life:returned".into(),
                bytes: b"abc\0\n".as_slice().into(),
            })),
            0,
        )
        .unwrap();
    assert_eq!(text(next(&mut probe)), "life:returned");
    for (index, expected) in [
        (1, "abc\0\n"),
        (2, "YWJjAAo="),
        (3, "true"),
        (4, "true"),
        (5, "YWJjAAo="),
    ] {
        probe.start_path(index, 0, None).unwrap();
        assert_eq!(text(next(&mut probe)), expected);
    }
    probe.close();
    assert_eq!(probe.collect().slots, 0);
}

#[test]
fn authority_equality_preserves_first_resource_and_registry_distinctions() {
    // Pinned ResourceManager observations: stage46-uri-authority-oracle-01.
    for (uris, expected_reads) in [
        (
            vec![
                "mem://HOST:080/item",
                "mem://host:80/item",
                "mem://HOST:00080/item",
            ],
            1,
        ),
        (vec!["mem://HOST/item", "mem://host:/item"], 1),
        (vec!["mem://HOST:0/item", "mem://host:00/item"], 1),
        (vec!["mem://A.1/item", "mem://a.1/item"], 2),
        (vec!["mem://A..B/item", "mem://a..b/item"], 2),
        (vec!["mem://A-/item", "mem://a-/item"], 2),
        (vec!["mem://A_B/item", "mem://a_b/item"], 2),
        (
            vec!["mem://[ABCD::1]:080/item", "mem://[abcd::1]:80/item"],
            1,
        ),
        (vec!["mem://USER@HOST/item", "mem://user@host/item"], 2),
        (vec!["mem://u%3a@HOST/item", "mem://u%3A@host/item"], 1),
    ] {
        let source = uris
            .iter()
            .enumerate()
            .map(|(n, uri)| format!("v{n} = read(\"{uri}\")\n"))
            .collect::<String>();
        let expressions = uris
            .iter()
            .enumerate()
            .map(|(n, _)| format!("v{n}.uri"))
            .collect::<Vec<_>>();
        let mut p = probe(
            &source,
            &expressions.iter().map(String::as_str).collect::<Vec<_>>(),
        );
        let mut reads = 0;
        for (index, uri) in uris.iter().enumerate() {
            p.start_path(index as u32, 0, None).unwrap();
            let event = next(&mut p);
            let value = if event.kind == "read" {
                reads += 1;
                let token = event.token.unwrap();
                assert_eq!(p.resolved_read_uri(&token).unwrap(), *uri);
                p.complete_read(
                    &token,
                    Ok(ReadResult::Resource(Resource {
                        uri: (*uri).into(),
                        bytes: vec![reads].into(),
                    })),
                    0,
                )
                .unwrap();
                next(&mut p)
            } else {
                event
            };
            assert_eq!(text(value), if expected_reads == 1 { uris[0] } else { uri });
        }
        assert_eq!(reads, expected_reads, "{uris:?}");
    }
}

#[test]
fn source_resource_default_bytes_and_java_base64_boundary() {
    let mut p = probe(
        "import \"pkl:test\"\nr = new Resource { uri = \"res:/custom\"; text = \"custom\"; base64 = \"QQ==\" }\ncaught = test.catch(() -> \"AA=\".base64DecodedBytes)\n",
        &[
            "r.bytes.base64",
            "\"AB==\".base64DecodedBytes.base64",
            "\"AAA\".base64DecodedBytes.base64",
            "caught",
        ],
    );
    // The lexical import is exposed through a module property for expression use.
    for (index, expected) in [
        (0, "QQ=="),
        (1, "AA=="),
        (2, "AAA="),
        (
            3,
            "Input byte array has wrong 4-byte ending unit String: \"AA=\"",
        ),
    ] {
        p.start_path(index, 0, None).unwrap();
        assert_eq!(text(next(&mut p)), expected);
    }
}

#[test]
fn unused_glob_helper_is_lazy_and_forcing_retains_explicit_obligation() {
    let mut p = probe(
        "function glob() = read*(\"file:*.pkl\")\nstable = 7\n",
        &["stable", "glob()"],
    );
    p.start_path(0, 0, None).unwrap();
    assert_eq!(text(next(&mut p)), "7");
    p.start_path(1, 0, None).unwrap();
    let event = next(&mut p);
    assert_eq!(event.kind, "error");
    assert_eq!(event.reason, Some("runtime-capability"));
}

#[test]
fn malformed_ipv6_user_info_is_a_language_error_before_host_dispatch() {
    for uri in [
        "mem://%@[::1]/x",
        "mem://%A@[::1]/x",
        "mem://%ZZ@[::1]/x",
        "mem://a[b@[::1]/x",
    ] {
        let mut p = probe(&format!("value = read(\"{uri}\")"), &["value"]);
        p.start_path(0, 0, None).unwrap();
        let event = next(&mut p);
        assert_eq!(event.kind, "error", "{uri}: {event:?}");
        assert_eq!(event.disposition, Some("language-error"));
        p.start_path(0, 0, None).unwrap();
        assert_eq!(next(&mut p).error, event.error);
    }
}
