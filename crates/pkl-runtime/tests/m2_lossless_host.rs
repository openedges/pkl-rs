//! Actual pinned Java SPI failures and producer diagnostics, with exact units.
use pkl_runtime::{
    probe::SourceProbe,
    value::{PklString, Value},
};
use std::{fs, path::Path};

fn raw(root: &Path, name: &str) -> Vec<u8> {
    fs::read(root.join(name)).unwrap()
}
fn units(root: &Path, name: &str) -> Vec<u16> {
    let data = raw(root, name);
    let (chunks, rest) = data.as_chunks::<2>();
    assert!(rest.is_empty());
    chunks
        .iter()
        .map(|pair| u16::from_be_bytes(*pair))
        .collect()
}
fn compare(actual: &[u16], root: &Path, stem: &str) {
    assert_eq!(
        actual,
        units(root, &format!("{stem}.code-units.bin")),
        "{} {stem}",
        root.display()
    );
    assert_eq!(
        PklString::from_utf16(actual.to_vec())
            .to_java_utf8()
            .as_bytes(),
        raw(root, &format!("{stem}.bin")),
        "{stem}"
    );
}
fn payload() -> PklString {
    PklString::from_utf16(vec![
        0xd83d, 0x7c, 0xdc00, 0x7c, 0xd83d, 0xde00, 0x7c, 0x3f, 0x7c, 0xfffd, 0x7c, 0,
    ])
}
fn case(id: &str, mode: &str, cause: &str) {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/m2-lossless-host");
    let source = fs::read_to_string(root.join(format!("workspaces/{id}/main.pkl"))).unwrap();
    let uri = fs::read_to_string(root.join(format!("workspaces/{id}/uri.txt"))).unwrap();
    let expressions: Vec<_> =
        fs::read_to_string(root.join(format!("workspaces/{id}/expressions.txt")))
            .unwrap()
            .lines()
            .map(str::to_owned)
            .collect();
    let cause = match cause {
        "units" => Some(payload()),
        "null" => None,
        "empty" => Some("".into()),
        _ => panic!("unknown cause"),
    };
    for repeat in 0..2 {
        let expected = root.join(format!("cases/{id}/{repeat}/calls"));
        let mut probe = SourceProbe::new_expressions_text(
            &source,
            &uri,
            "main",
            &expressions,
            "compact",
            false,
            false,
            true,
        )
        .unwrap();
        let (mut resolves, mut loads, mut reads) = (0, 0, 0);
        for (index, expression) in expressions.iter().enumerate() {
            assert_eq!(
                expression.as_bytes(),
                raw(&expected, &format!("call-{index}.expression.bin"))
            );
            probe.start_path(index as u32, 0, None).unwrap();
            let mut done = false;
            for _ in 0..100_000 {
                let event = probe.poll(0, 1).unwrap();
                probe.collect();
                match event.kind {
                    "yield" => {}
                    "module-request" => {
                        let token = event.token.unwrap();
                        let request = event.module.unwrap();
                        match request.kind {
                            "resolve" => {
                                resolves += 1;
                                if mode == "module-resolve" && resolves == 1 {
                                    probe
                                        .reject_module_text(&token, false, "io", cause.clone(), 0)
                                        .unwrap();
                                } else {
                                    probe
                                        .complete_module_resolution(
                                            &token,
                                            "lossless:/child",
                                            "lossless:/child",
                                            "lossless:/child",
                                            0,
                                        )
                                        .unwrap();
                                }
                            }
                            "load-source" => {
                                loads += 1;
                                if mode == "module-source" && loads == 1 {
                                    probe
                                        .reject_module_text(&token, true, "io", cause.clone(), 0)
                                        .unwrap();
                                } else {
                                    probe
                                        .complete_module_source(&token, "value = 9\n", "child", 0)
                                        .unwrap();
                                }
                            }
                            kind => panic!("unexpected module request {kind}"),
                        }
                    }
                    "read" => {
                        let token = event.token.unwrap();
                        if mode == "unknown-format" {
                            assert_eq!(event.uri.as_deref(), Some("prop:pkl.outputFormat"));
                            probe.respond(&token, Value::String(payload()), 0).unwrap();
                        } else {
                            reads += 1;
                            if reads == 1 {
                                probe.reject_read_io(&token, cause.clone(), 0).unwrap();
                            } else {
                                probe.respond(&token, Value::String("9".into()), 0).unwrap();
                            }
                        }
                    }
                    "value" | "error" => {
                        let stem = format!("call-{index}");
                        let (channel, actual) = if event.kind == "value" {
                            assert_eq!(raw(&expected, &format!("{stem}.status.bin")), b"success\n");
                            ("stdout", event.value.unwrap())
                        } else {
                            assert_eq!(raw(&expected, &format!("{stem}.status.bin")), b"error\n");
                            assert_eq!(event.disposition, Some("language-error"));
                            ("stderr", event.error.unwrap())
                        };
                        compare(&actual, &expected, &format!("{stem}.{channel}"));
                        done = true;
                        break;
                    }
                    other => panic!("unexpected {other}"),
                }
            }
            assert!(done, "{id} call {index} did not finish");
        }
        assert_eq!(
            format!("resolve={resolves}\nsource={loads}\nread={reads}\n").as_bytes(),
            raw(&expected, "counters.bin"),
            "{id}"
        );
        probe.close();
        assert_eq!(probe.collect().slots, 0);
    }
}
#[test]
fn original_diagnostic_producers_preserve_full_units_and_output_text_operation() {
    case("invalid-converter", "converter", "units");
    case("unknown-output-format", "unknown-format", "units");
}
#[test]
fn actual_module_io_null_empty_and_units_preserve_failure_cache_boundaries() {
    for mode in ["module-resolve", "module-source"] {
        for cause in ["null", "empty", "units"] {
            case(&format!("{mode}-{cause}"), mode, cause);
        }
    }
}
#[test]
fn actual_resource_io_null_empty_and_units_preserve_retry_boundary() {
    for cause in ["null", "empty", "units"] {
        case(&format!("resource-{cause}"), "resource", cause);
    }
}

fn next(probe: &mut SourceProbe) -> pkl_runtime::probe::SourceEvent {
    for _ in 0..100_000 {
        let event = probe.poll(0, 1).unwrap();
        probe.collect();
        if event.kind != "yield" {
            return event;
        }
    }
    panic!("source session did not settle");
}
fn lifecycle_probe() -> SourceProbe {
    SourceProbe::new_expressions_text(
        "resource=read(\"lossless:/resource\")\nchild=import(\"lossless:/child\")\nstable=7\n",
        "file:///lossless-lifecycle.pkl",
        "main",
        &["resource".into(), "child".into(), "stable".into()],
        "compact",
        false,
        false,
        false,
    )
    .unwrap()
}
#[test]
fn typed_rejection_preserves_pending_ownership_reentry_and_raw_injection() {
    let mut probe = lifecycle_probe();
    probe.start_path(0, 0, None).unwrap();
    let read = next(&mut probe);
    let token = read.token.unwrap();
    assert_eq!(read.kind, "read");
    assert!(
        probe
            .reject_module_text(&token, false, "io", None, 0)
            .is_err()
    );
    assert_eq!(next(&mut probe).token.as_deref(), Some(token.as_str()));
    probe.reenter_path(&token, 2, 0, None).unwrap();
    assert_eq!(next(&mut probe).value.unwrap(), vec![55]);
    assert_eq!(next(&mut probe).token.as_deref(), Some(token.as_str()));
    probe.reject_text(&token, payload(), 0).unwrap();
    let error = next(&mut probe).error.unwrap();
    let message: Vec<_> = payload().encode_utf16().collect();
    assert!(error.windows(message.len()).any(|window| window == message));
    assert!(
        !PklString::from_utf16(error)
            .to_java_utf8()
            .contains("IOException")
    );
    assert!(probe.reject_read_io(&token, None, 0).is_err());
    probe.start_path(1, 0, None).unwrap();
    let module = next(&mut probe);
    let token = module.token.unwrap();
    assert_eq!(module.kind, "module-request");
    assert!(probe.reject_read_io(&token, None, 0).is_err());
    assert!(
        probe
            .reject_module_text(&token, false, "denied", None, 0)
            .is_err()
    );
    assert!(
        probe
            .reject_module_text(&token, true, "io", Some(payload()), 0)
            .is_err()
    );
    assert_eq!(next(&mut probe).token.as_deref(), Some(token.as_str()));
    probe
        .reject_module_text(&token, false, "denied", Some(payload()), 0)
        .unwrap();
    assert_eq!(next(&mut probe).kind, "error");
    probe.start_path(2, 0, None).unwrap();
    assert_eq!(next(&mut probe).value.unwrap(), vec![55]);
    probe.close();
    assert_eq!(probe.collect().slots, 0);
}
#[test]
fn typed_rejection_cannot_revive_cancelled_or_closed_read() {
    for close in [false, true] {
        let mut probe = lifecycle_probe();
        probe.start_path(0, 0, None).unwrap();
        let token = next(&mut probe).token.unwrap();
        if close {
            probe.close();
        } else {
            probe.cancel();
        }
        assert_eq!(probe.drain_cancelled_reads(), vec![token.clone()]);
        assert!(probe.drain_cancelled_reads().is_empty());
        assert!(probe.reject_read_io(&token, Some(payload()), 0).is_err());
        probe.close();
        assert_eq!(probe.collect().slots, 0);
    }
}
#[test]
fn output_text_continuation_uses_same_receiver_and_callback_stack() {
    let mut probe = SourceProbe::new_expressions_text(
        "output { text=read(\"lossless:/text\") }\nstable=7\n",
        "file:///output-text-reentry.pkl",
        "main",
        &["output.text".into(), "stable".into()],
        "compact",
        false,
        false,
        false,
    )
    .unwrap();
    probe.start_path(0, 0, None).unwrap();
    let token = next(&mut probe).token.unwrap();
    probe.reenter_path(&token, 1, 0, None).unwrap();
    assert_eq!(next(&mut probe).value.unwrap(), vec![55]);
    assert_eq!(next(&mut probe).token.as_deref(), Some(token.as_str()));
    probe.respond(&token, Value::String(payload()), 0).unwrap();
    let result = next(&mut probe).value.unwrap();
    assert_eq!(result, payload().encode_utf16().collect::<Vec<_>>());
    probe.start_path(0, 0, None).unwrap();
    assert_eq!(next(&mut probe).value.unwrap(), result);
    probe.close();
    assert_eq!(probe.collect().slots, 0);
    assert_eq!(result, payload().encode_utf16().collect::<Vec<_>>());
}

#[test]
fn scalar_and_session_output_text_share_the_same_canonical_operation() {
    let source = "output { text=\"\\u{d83d}|\\u{dc00}\" }\n";
    let expressions = vec!["output.text".into(), "output.text".into()];
    let results = pkl_runtime::evaluation::scalar_expressions_text_with_options(
        source,
        "file:///output-text-api.pkl",
        "main",
        &expressions,
        Default::default(),
    )
    .unwrap();
    let expected = vec![0xd83d, 0x7c, 0xdc00];
    let mut probe = SourceProbe::new_expressions_text(
        source,
        "file:///output-text-api.pkl",
        "main",
        &expressions,
        "compact",
        false,
        false,
        false,
    )
    .unwrap();
    for (index, result) in results.into_iter().enumerate() {
        assert_eq!(result.unwrap().encode_utf16().collect::<Vec<_>>(), expected);
        probe.start_path(index as u32, 0, None).unwrap();
        assert_eq!(next(&mut probe).value.unwrap(), expected);
    }
    probe.close();
    assert_eq!(probe.collect().slots, 0);
}
