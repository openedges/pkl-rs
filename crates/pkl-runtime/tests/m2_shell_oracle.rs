//! Explicit-logger Java shell observations; full raw fields, no cross-channel ordering claim.
use pkl_runtime::{
    evaluation::{
        EvaluationOptions,
        session::{SourceSession, SourceSessionOptions},
    },
    host::HostPoll,
    logging::{FrameTransformer, LogDelivery, LogEvent, LogLevel},
    modules::default_completion,
    value::PklString,
};
use std::{
    fs,
    path::{Path, PathBuf},
};
const CASES: [&str; 20] = [
    "empty",
    "ordinary",
    "apostrophe",
    "apostrophe-runs",
    "edge-apostrophes",
    "metacharacters",
    "line-controls",
    "nul",
    "supplementary",
    "unpaired-high",
    "unpaired-low",
    "split-surrogates",
    "wrong-int",
    "wrong-null",
    "missing-argument",
    "extra-argument",
    "argument-throw",
    "arity-versus-argument",
    "argument-trace",
    "cached-import",
];

fn read(folder: &Path, name: &str) -> Vec<u8> {
    fs::read(folder.join(name)).unwrap_or_else(|e| panic!("{}: {e}", folder.join(name).display()))
}
fn utf8(folder: &Path, name: &str) -> String {
    String::from_utf8(read(folder, name)).unwrap_or_else(|e| panic!("{name}: {e}"))
}
fn bytes(folder: &Path, name: &str, actual: &[u8], label: &str, failures: &mut Vec<String>) {
    let expected = read(folder, name);
    if expected != actual {
        failures.push(format!(
            "{label}/{name}: expected {expected:?}; actual {actual:?}"
        ));
    }
}
fn text(folder: &Path, stem: &str, actual: &PklString, label: &str, failures: &mut Vec<String>) {
    bytes(
        folder,
        &format!("{stem}.bin"),
        actual.to_java_utf8().as_bytes(),
        label,
        failures,
    );
    let units = actual
        .encode_utf16()
        .flat_map(u16::to_be_bytes)
        .collect::<Vec<_>>();
    bytes(
        folder,
        &format!("{stem}.code-units.bin"),
        &units,
        label,
        failures,
    );
}

fn evaluate(
    source: &str,
    uri: &str,
    expression: &str,
    power: bool,
) -> (Result<PklString, PklString>, Vec<LogEvent>) {
    let options = SourceSessionOptions {
        evaluation: EvaluationOptions {
            power_assertions: power,
        },
        log_delivery: LogDelivery::Host,
        frame_transformer: FrameTransformer::Default,
        ..Default::default()
    };
    let mut session =
        match SourceSession::compile_text(source, uri, "main", &[expression.to_owned()], options) {
            Ok(session) => session,
            Err(error) => return (Err(error), Vec::new()),
        };
    let mut events = Vec::new();
    let start = session.start_path(0, 0, None);
    let result = match start {
        Err(error) => Err(session.format_error_text(error)),
        Ok(()) => {
            let mut result = None;
            for _ in 0..1_000_000 {
                match session.poll(0, 128) {
                    HostPoll::Yielded => {}
                    HostPoll::Module { token, request } => {
                        let completion =
                            default_completion(&request).expect("default bundled module request");
                        session.complete_module(&token, completion, 0).unwrap();
                    }
                    HostPoll::Read { token, uri } => {
                        assert!(
                            uri.starts_with("prop:") || uri.starts_with("env:"),
                            "unexpected resource read {uri}"
                        );
                        session.resume_default_read(&token, &[], &[], 0).unwrap();
                    }
                    HostPoll::Log { token, event } => {
                        events.push((*event).clone());
                        session.acknowledge_log(&token, Ok(()), 0).unwrap();
                    }
                    HostPoll::TransformFrame { token, frame } => {
                        let transformed = FrameTransformer::Default.transform((*frame).clone());
                        session
                            .complete_frame_transform(&token, Ok(transformed), 0)
                            .unwrap();
                    }
                    HostPoll::Ready(Ok(value)) => {
                        result = Some(
                            SourceSession::result_string(value)
                                .map_err(|e| session.format_error_text(e)),
                        );
                        break;
                    }
                    HostPoll::Ready(Err(error)) => {
                        result = Some(Err(session.format_error_text(error)));
                        break;
                    }
                    other => panic!("unexpected Host event: {other:?}"),
                }
            }
            result.expect("shell evaluation exceeded the control budget")
        }
    };
    session.close();
    (result, events)
}

fn count(folder: &Path, name: &str) -> usize {
    let raw = read(folder, name);
    let digits = raw.strip_suffix(b"\n").expect("count record newline");
    let number: usize = std::str::from_utf8(digits)
        .expect("ASCII count")
        .parse()
        .expect("count integer");
    assert_eq!(format!("{number}\n").as_bytes(), raw, "noncanonical count");
    number
}

fn compare_events(folder: &Path, events: &[LogEvent], label: &str, failures: &mut Vec<String>) {
    bytes(
        folder,
        "events.count.bin",
        format!("{}\n", events.len()).as_bytes(),
        label,
        failures,
    );
    let expected_count = count(folder, "events.count.bin");
    for (i, event) in events.iter().take(expected_count).enumerate() {
        let prefix = format!("event-{i}.");
        let level = match event.level {
            LogLevel::Trace => "trace",
            LogLevel::Warn => "warn",
        };
        text(
            folder,
            &(prefix.clone() + "level"),
            &PklString::from(level),
            label,
            failures,
        );
        text(
            folder,
            &(prefix.clone() + "message"),
            &event.message,
            label,
            failures,
        );
        text(
            folder,
            &(prefix.clone() + "uri"),
            &event.frame.module_uri,
            label,
            failures,
        );
        let present = if event.frame.member_name.is_some() {
            b"1".as_slice()
        } else {
            b"0".as_slice()
        };
        bytes(
            folder,
            &(prefix.clone() + "member.present"),
            present,
            label,
            failures,
        );
        if let Some(member) = &event.frame.member_name
            && read(folder, &(prefix.clone() + "member.present")) == b"1"
        {
            text(
                folder,
                &(prefix.clone() + "member"),
                member,
                label,
                failures,
            );
        }
        let frame = &event.frame;
        let position = format!(
            "{}:{}:{}:{}\n",
            frame.start_line, frame.start_column, frame.end_line, frame.end_column
        );
        bytes(
            folder,
            &(prefix.clone() + "position.bin"),
            position.as_bytes(),
            label,
            failures,
        );
        bytes(
            folder,
            &(prefix.clone() + "source-count.bin"),
            format!("{}\n", frame.source_lines.len()).as_bytes(),
            label,
            failures,
        );
        let expected_lines = count(folder, &(prefix.clone() + "source-count.bin"));
        for (line, value) in frame.source_lines.iter().take(expected_lines).enumerate() {
            text(
                folder,
                &format!("{prefix}source-{line}"),
                value,
                label,
                failures,
            );
        }
    }
}

#[test]
fn java_shell_matches_value_diagnostic_and_ordered_logger_events() {
    let root =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/m2-shell-oracle");
    let roster = utf8(&root, "cases.txt");
    assert_eq!(roster.lines().collect::<Vec<_>>(), CASES);
    let mut failures = Vec::new();
    for case in CASES {
        let input = root.join(case);
        let source = utf8(&input, "module.pkl");
        let uri = utf8(&input, "uri.txt");
        let module = utf8(&input, "module-name.txt");
        let expression = utf8(&input, "expression.txt");
        assert_eq!(uri, format!("file:///stage143-shell/{case}/main.pkl"));
        assert_eq!(module, "main");
        assert_eq!(expression, "output.text");
        for (mode, power) in [("on", true), ("off", false)] {
            let folder = input.join(mode);
            let label = format!("{case}/{mode}");
            let (result, events) = evaluate(&source, &uri, &expression, power);
            let (status, stdout, stderr) = match result {
                Ok(value) => (b"success\n".as_slice(), value, PklString::default()),
                Err(error) => (b"error\n".as_slice(), PklString::default(), error),
            };
            bytes(&folder, "result.status.bin", status, &label, &mut failures);
            text(
                &folder,
                "expression",
                &PklString::from(expression.as_str()),
                &label,
                &mut failures,
            );
            text(&folder, "result.stdout", &stdout, &label, &mut failures);
            text(&folder, "result.stderr", &stderr, &label, &mut failures);
            // Java's class is fixture provenance: Rust has no Java exception class.
            // Verify both raw class fields against the separately admitted Java status,
            // without pretending that this proves Rust/Java class identity.
            let class = match read(&folder, "result.status.bin").as_slice() {
                b"success\n" => "",
                b"error\n" => "org.pkl.core.PklException",
                other => panic!("unadmitted Java status: {other:?}"),
            };
            text(
                &folder,
                "result.error-class",
                &PklString::from(class),
                &label,
                &mut failures,
            );
            compare_events(&folder, &events, &label, &mut failures);
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
