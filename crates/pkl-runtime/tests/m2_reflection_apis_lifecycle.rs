//! Shared Host/GC lifecycle checks. Complete Java output/diagnostic comparisons
//! belong to m2_reflection_apis_oracle; these assertions do not replace them.
#[path = "support/module_keys.rs"]
mod module_keys;
use pkl_runtime::{
    host::ReadFailure,
    probe::{SourceEvent, SourceProbe},
    value::{ErrorKind, PklString},
};

const URI: &str = "file:///stage103/lifecycle.pkl";
const SOURCE: &str = r#"module stage103.lifecycle
import "pkl:reflect"
class Declaration {
  hidden payload: String = read("mem:reflection-default")
}
hidden mirror = reflect.Class(Declaration)
"#;

fn next(probe: &mut SourceProbe) -> SourceEvent {
    for _ in 0..300_000 {
        probe.collect();
        let event = probe.poll(0, 1).unwrap();
        probe.collect();
        match event.kind {
            "yield" => (),
            "module-request" => assert!(
                module_keys::complete_probe(probe, event.token.as_deref().unwrap(), 0).unwrap()
            ),
            _ => return event,
        }
    }
    panic!("reflection lifecycle scheduler did not settle");
}
fn begin(probe: &mut SourceProbe, expression: &str) {
    probe
        .evaluate_module_source(URI, Some(SOURCE), expression, 0, None)
        .unwrap();
}
fn text(event: SourceEvent) -> Vec<u16> {
    assert_eq!(event.kind, "value", "{event:?}");
    event.value.unwrap()
}

#[test]
fn metadata_stays_lazy_and_default_value_survives_failure_retry_reentry_and_gc() {
    let mut probe = SourceProbe::new_module_evaluator("compact", false, false, false).unwrap();
    begin(&mut probe, "mirror.properties[\"payload\"].name");
    // A metadata read must not evaluate the deliberately effectful initializer.
    assert_eq!(
        text(next(&mut probe)),
        "payload".encode_utf16().collect::<Vec<_>>()
    );
    begin(&mut probe, "mirror.properties[\"payload\"].defaultValue");
    let first = next(&mut probe);
    assert_eq!(first.kind, "read");
    assert_eq!(first.uri.as_deref(), Some("mem:reflection-default"));
    let failed = first.token.unwrap();
    probe
        .complete_read(
            &failed,
            Err(ReadFailure::SecurityManagerException {
                message: Some("reflection lifecycle retry".into()),
            }),
            0,
        )
        .unwrap();
    assert_eq!(next(&mut probe).kind, "error");

    begin(&mut probe, "mirror.properties[\"payload\"].defaultValue");
    let retry = next(&mut probe);
    assert_eq!(retry.kind, "read");
    let token = retry.token.unwrap();
    assert_ne!(failed, token);
    assert_eq!(
        probe
            .complete_read_text(&failed, "stale".into(), 0)
            .unwrap_err()
            .kind,
        ErrorKind::Host
    );
    probe
        .reenter_module_source(
            &token,
            URI,
            Some(SOURCE),
            "mirror.properties[\"payload\"].name",
            0,
            None,
        )
        .unwrap();
    assert_eq!(
        text(next(&mut probe)),
        "payload".encode_utf16().collect::<Vec<_>>()
    );
    let resumed = next(&mut probe);
    assert_eq!(resumed.kind, "read");
    assert_eq!(resumed.token.as_deref(), Some(token.as_str()));
    let units = vec![0x41, 0xd800, 0, 0x0d, 0x0a, 0xdc00];
    probe
        .complete_read_text(&token, PklString::from_utf16(units.clone()), 0)
        .unwrap();
    assert_eq!(text(next(&mut probe)), units);
    // A successful default retains its value across queries and collections.
    // Any second Host read would fail the terminal-value assertion.
    begin(&mut probe, "mirror.properties[\"payload\"].defaultValue");
    assert_eq!(text(next(&mut probe)), units);
    probe.close();
    assert_eq!(probe.collect().slots, 0);
}

#[test]
fn location_transform_uses_the_shared_host_boundary_and_lossless_uri() {
    let mut probe = SourceProbe::new_module_evaluator("compact", false, true, false).unwrap();
    begin(&mut probe, "mirror.location.displayUri");
    let pending = next(&mut probe);
    assert_eq!(pending.kind, "transform-frame");
    let token = pending.token.unwrap();
    let mut frame = pending.frame.unwrap();
    assert_eq!(frame.module_uri, URI.encode_utf16().collect::<Vec<_>>());
    assert!(frame.member_name.is_none());
    assert!(frame.source_lines.is_empty());
    assert!(frame.start_line > 0 && frame.start_column > 0);
    probe
        .reenter_module_source(&token, URI, Some(SOURCE), "mirror.name", 0, None)
        .unwrap();
    assert_eq!(
        text(next(&mut probe)),
        "Declaration".encode_utf16().collect::<Vec<_>>()
    );
    let resumed = next(&mut probe);
    assert_eq!(resumed.kind, "transform-frame");
    assert_eq!(resumed.token.as_deref(), Some(token.as_str()));
    let uri = vec![0x6d, 0x65, 0x6d, 0x3a, 0xd800, 0x20, 0xdc00];
    frame.module_uri = uri.clone();
    probe
        .complete_frame_transform(&token, Some(frame), None, 0)
        .unwrap();
    assert_eq!(text(next(&mut probe)), uri);
    probe.close();
    assert_eq!(probe.collect().slots, 0);
}

#[test]
fn cancellation_closes_evaluator_and_fresh_evaluator_rejects_old_transform_token() {
    let mut probe = SourceProbe::new_module_evaluator("compact", false, true, false).unwrap();
    begin(&mut probe, "mirror.location.displayUri");
    let pending = next(&mut probe);
    assert_eq!(pending.kind, "transform-frame");
    let stale = pending.token.unwrap();
    let frame = pending.frame.unwrap();
    probe.cancel();
    assert_eq!(next(&mut probe).kind, "error");
    assert_eq!(
        probe
            .complete_frame_transform(&stale, Some(frame.clone()), None, 0)
            .unwrap_err()
            .kind,
        ErrorKind::Closed
    );
    assert_eq!(
        probe
            .evaluate_module_source(URI, Some(SOURCE), "mirror.location.displayUri", 0, None)
            .unwrap_err()
            .kind,
        ErrorKind::Closed
    );
    assert_eq!(probe.collect().slots, 0);

    let mut probe = SourceProbe::new_module_evaluator("compact", false, true, false).unwrap();
    begin(&mut probe, "mirror.location.displayUri");
    let pending = next(&mut probe);
    assert_eq!(pending.kind, "transform-frame");
    let token = pending.token.unwrap();
    assert_ne!(stale, token);
    assert_eq!(
        probe
            .complete_frame_transform(&stale, Some(frame), None, 0)
            .unwrap_err()
            .kind,
        ErrorKind::Host
    );
    probe
        .complete_frame_transform(&token, pending.frame, None, 0)
        .unwrap();
    assert_eq!(
        text(next(&mut probe)),
        URI.encode_utf16().collect::<Vec<_>>()
    );
    probe.close();
    assert_eq!(probe.collect().slots, 0);
}

#[test]
fn type_mirrors_use_module_namespaces_without_forcing_class_value_collisions() {
    const SOURCE: &str = r#"module stage103.namespaceLifecycle
import "pkl:reflect"
import "pkl:base" as b
class Declaration {
  String: Int = read("mem:must-not-read-simple")
  b: Int = read("mem:must-not-read-qualified")
  simple: String
  qualified: b.String
  namedModule: b
}
hidden mirror = reflect.Class(Declaration)
"#;
    let mut probe = SourceProbe::new_module_evaluator("compact", false, false, false).unwrap();
    for property in ["simple", "qualified"] {
        probe
            .evaluate_module_source(
                URI,
                Some(SOURCE),
                format!("mirror.properties[\"{property}\"].type.referent.name"),
                0,
                None,
            )
            .unwrap();
        // The step-by-step collector exercises the declaration lookup and lazy
        // base-module import. Any read event exposes an incorrectly forced field.
        assert_eq!(
            text(next(&mut probe)),
            "String".encode_utf16().collect::<Vec<_>>()
        );
    }
    probe
        .evaluate_module_source(
            URI,
            Some(SOURCE),
            "mirror.properties[\"namedModule\"].type.referent.name",
            0,
            None,
        )
        .unwrap();
    let named = text(next(&mut probe));
    probe
        .evaluate_module_source(
            URI,
            Some(SOURCE),
            "mirror.enclosingDeclaration.name",
            0,
            None,
        )
        .unwrap();
    let enclosing = text(next(&mut probe));
    // The named imported module denotes its own class, not the current module.
    assert_ne!(named, enclosing);
    assert!(!named.is_empty());
    probe.close();
    assert_eq!(probe.collect().slots, 0);
}

#[test]
fn canonical_class_type_erases_arguments_without_resolving_them() {
    const SOURCE: &str = r#"module stage103.classErasure
import "pkl:reflect"
import "pkl:base" as b
typealias ClassAlias<T> = Class<T>
class Declaration {
  simple: Class<String>
  qualified: b.Class<DoesNotExist>
  alias: ClassAlias<String>
}
hidden mirror = reflect.Class(Declaration)
hidden explicit = reflect.DeclaredType(reflect.Class(Class)).withTypeArgument(reflect.DeclaredType(reflect.Class(String)))
"#;
    let mut probe = SourceProbe::new_module_evaluator("compact", false, false, false).unwrap();
    for (expression, expected) in [
        (
            r#"mirror.properties["simple"].type.typeArguments[0].getClass().simpleName"#,
            "UnknownType",
        ),
        (
            r#"mirror.properties["qualified"].type.typeArguments[0].getClass().simpleName"#,
            "UnknownType",
        ),
        (
            r#"mirror.properties["alias"].type.typeArguments[0].referent.name"#,
            "String",
        ),
        (
            r#"mirror.properties["alias"].type.referent.referent.typeArguments[0].getClass().simpleName"#,
            "UnknownType",
        ),
        (r#"explicit.typeArguments[0].referent.name"#, "String"),
    ] {
        probe
            .evaluate_module_source(URI, Some(SOURCE), expression, 0, None)
            .unwrap();
        assert_eq!(
            text(next(&mut probe)),
            expected.encode_utf16().collect::<Vec<_>>()
        );
    }
    probe.close();
    assert_eq!(probe.collect().slots, 0);
}
