//! Exact Java guest catch payloads. Terminal and constructor boundaries are explicit.
use pkl_runtime::{
    probe::{SourceEvent, SourceProbe},
    value::{ErrorKind, PklString, Value},
};
use std::{
    fs,
    path::{Path, PathBuf},
};

fn raw(root: &Path, name: &str) -> Vec<u8> {
    fs::read(root.join(name)).unwrap()
}
fn text(root: &Path, name: &str) -> String {
    fs::read_to_string(root.join(name)).unwrap()
}
fn units(root: &Path, name: &str) -> Vec<u16> {
    let data = raw(root, name);
    let (pairs, rest) = data.as_chunks::<2>();
    assert!(rest.is_empty());
    pairs.iter().map(|p| u16::from_be_bytes(*p)).collect()
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
        "{} {stem}",
        root.display()
    );
}
fn fixture(group: &str, id: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/m2-guest-catch")
        .join(group)
        .join("workspaces")
        .join(id)
}
fn expected(group: &str, id: &str, repeat: usize) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/m2-guest-catch")
        .join(group)
        .join("cases")
        .join(id)
        .join(repeat.to_string())
        .join("calls")
}
fn lines(root: &Path, name: &str) -> Vec<String> {
    text(root, name).lines().map(str::to_owned).collect()
}
fn new_probe(root: &Path) -> Result<SourceProbe, PklString> {
    let mut expressions = lines(root, "expressions.txt");
    expressions.extend(lines(root, "callbacks.txt"));
    SourceProbe::new_expressions_text(
        &text(root, "main.pkl"),
        &text(root, "uri.txt"),
        "main",
        &expressions,
        "compact",
        true,
        false,
        true,
    )
}
fn compare_event(event: &SourceEvent, root: &Path, prefix: &str) {
    let (status, channel, data) = match event.kind {
        "value" => (
            b"success\n".as_slice(),
            "stdout",
            event.value.as_ref().unwrap(),
        ),
        "error" => {
            assert_eq!(event.disposition, Some("language-error"));
            (
                b"error\n".as_slice(),
                "stderr",
                event.error.as_ref().unwrap(),
            )
        }
        other => panic!("unexpected completion {other}: {event:?}"),
    };
    assert_eq!(
        raw(root, &format!("{prefix}.status.bin")),
        status,
        "{prefix}"
    );
    compare(data, root, &format!("{prefix}.{channel}"));
}
struct Host {
    mode: String,
    expected: PathBuf,
    callbacks: Vec<String>,
    outer_len: usize,
    resolves: usize,
    sources: usize,
    reads: usize,
    logs: usize,
    callback_count: usize,
    callbacks_done: bool,
    events: usize,
}
impl Host {
    fn event(&mut self, value: &str) {
        assert_eq!(
            value.as_bytes(),
            raw(&self.expected, &format!("event-{}.bin", self.events))
        );
        self.events += 1;
    }
    fn completion(&mut self, event: &SourceEvent, prefix: &str) {
        compare_event(event, &self.expected, prefix);
        let status = if event.kind == "value" {
            assert!(raw(&self.expected, &format!("{prefix}.error-class.bin")).is_empty());
            "success"
        } else {
            assert_eq!(
                raw(&self.expected, &format!("{prefix}.error-class.bin")),
                b"org.pkl.core.PklException"
            );
            "error org.pkl.core.PklException"
        };
        self.event(&format!("{prefix} {status}"));
    }
    fn callbacks(&mut self, probe: &mut SourceProbe, token: &str) {
        if self.callbacks_done {
            return;
        }
        self.callbacks_done = true;
        self.event("callback-start");
        for offset in 0..self.callbacks.len() {
            probe
                .reenter_path(token, (self.outer_len + offset) as u32, 0, None)
                .unwrap();
            let prefix = format!("inner-{}", self.callback_count);
            self.callback_count += 1;
            let event = self.settle(probe, 0);
            self.completion(&event, &prefix);
        }
        self.event("callback-end");
    }
    fn log(&mut self, event: SourceEvent, probe: &mut SourceProbe) {
        let token = event.token.unwrap();
        let log = event.log.unwrap();
        let stem = format!("log-{}", self.logs);
        self.logs += 1;
        assert_eq!(
            log.level.as_bytes(),
            raw(&self.expected, &format!("{stem}.level.bin"))
        );
        compare(&log.message, &self.expected, &format!("{stem}.message"));
        compare(&log.module_uri, &self.expected, &format!("{stem}.uri"));
        assert_eq!(
            raw(&self.expected, &format!("{stem}.member-null.bin")),
            if log.member_name.is_none() {
                b"true".as_slice()
            } else {
                b"false".as_slice()
            }
        );
        compare(
            log.member_name.as_deref().unwrap_or(&[]),
            &self.expected,
            &format!("{stem}.member"),
        );
        assert_eq!(
            format!(
                "{}:{}:{}:{}",
                log.start_line, log.start_column, log.end_line, log.end_column
            )
            .as_bytes(),
            raw(&self.expected, &format!("{stem}.position.bin"))
        );
        assert_eq!(
            log.source_lines.len().to_string().as_bytes(),
            raw(&self.expected, &format!("{stem}.source-count.bin"))
        );
        for (index, line) in log.source_lines.iter().enumerate() {
            compare(line, &self.expected, &format!("{stem}.source-{index}"));
        }
        self.event(&format!(
            "trace {}",
            PklString::from_utf16(log.message).to_java_utf8()
        ));
        if self.mode == "logger-reentry" {
            self.callbacks(probe, &token);
        }
        probe.acknowledge_log(&token, None, 0).unwrap();
    }
    fn settle(&mut self, probe: &mut SourceProbe, now: u32) -> SourceEvent {
        for _ in 0..200_000 {
            let event = probe.poll(now, 1).unwrap();
            probe.collect();
            match event.kind {
                "yield" => {}
                "log" => self.log(event, probe),
                "module-request" => {
                    let token = event.token.unwrap();
                    if probe.complete_default_module(&token, now).unwrap() {
                        continue;
                    }
                    let request = event.module.unwrap();
                    assert_eq!(request.original_uri, "catch:/child");
                    match request.kind {
                        "resolve" => {
                            self.resolves += 1;
                            self.event(&format!(
                                "resolve {} attempt={}",
                                request.original_uri, self.resolves
                            ));
                            assert_eq!(request.import_uri.as_deref(), Some("catch:/child"));
                            if self.mode == "resolve-retry" && self.resolves == 1 {
                                probe
                                    .reject_module_text(
                                        &token,
                                        false,
                                        "io",
                                        Some("resolve-first".into()),
                                        now,
                                    )
                                    .unwrap();
                            } else {
                                probe
                                    .complete_module_resolution(
                                        &token,
                                        "catch:/child",
                                        "catch:/child",
                                        "catch:/child",
                                        now,
                                    )
                                    .unwrap();
                            }
                        }
                        "load-source" => {
                            self.sources += 1;
                            let attempt = self.sources;
                            self.event(&format!(
                                "source-start {} attempt={attempt}",
                                request.original_uri
                            ));
                            assert_eq!(request.resolved_uri.as_deref(), Some("catch:/child"));
                            assert_eq!(request.source_uri.as_deref(), Some("catch:/child"));
                            if self.mode == "source-reentry" && attempt == 1 {
                                self.callbacks(probe, &token);
                            }
                            if matches!(self.mode.as_str(), "source-cache" | "source-reentry")
                                && attempt == 1
                            {
                                probe
                                    .reject_module_text(
                                        &token,
                                        true,
                                        "io",
                                        Some("source-first".into()),
                                        now,
                                    )
                                    .unwrap();
                            } else {
                                self.event(&format!(
                                    "source-end {} attempt={attempt}",
                                    request.original_uri
                                ));
                                probe
                                    .complete_module_source(&token, "value = 9\n", "child", now)
                                    .unwrap();
                            }
                        }
                        other => panic!("unexpected module request {other}"),
                    }
                }
                "read" => {
                    let token = event.token.unwrap();
                    let uri = event.uri.unwrap();
                    assert!(uri.starts_with("catch:"), "unexpected read {uri}");
                    self.reads += 1;
                    self.event(&format!("read {uri} attempt={}", self.reads));
                    if self.mode == "read-reentry" && self.reads == 1 {
                        self.callbacks(probe, &token);
                    }
                    if matches!(self.mode.as_str(), "read-retry" | "read-reentry")
                        && self.reads == 1
                    {
                        probe
                            .reject_read_io(&token, Some("read-first".into()), now)
                            .unwrap();
                    } else {
                        let value = if self.mode == "renderer" {
                            let name = match uri.as_str() {
                                "catch:/units" => "payload-units",
                                "catch:/white" => "payload-white",
                                "catch:/blank" => "payload-blank",
                                _ => panic!("unknown payload {uri}"),
                            };
                            PklString::from_utf16(units(
                                &self.expected,
                                &format!("{name}.code-units.bin"),
                            ))
                        } else {
                            "9".into()
                        };
                        probe.respond(&token, Value::String(value), now).unwrap();
                    }
                }
                "value" | "error" => return event,
                other => panic!("unexpected event {other}"),
            }
        }
        panic!("guest catch did not settle");
    }
}
fn ordinary(group: &str, id: &str) {
    let f = fixture(group, id);
    let expressions = lines(&f, "expressions.txt");
    let mode = text(&f, "mode.txt");
    for repeat in 0..2 {
        let expected = expected(group, id, repeat);
        let mut probe = new_probe(&f).unwrap();
        let mut host = Host {
            mode: mode.clone(),
            expected: expected.clone(),
            callbacks: lines(&f, "callbacks.txt"),
            outer_len: expressions.len(),
            resolves: 0,
            sources: 0,
            reads: 0,
            logs: 0,
            callback_count: 0,
            callbacks_done: false,
            events: 0,
        };
        for (index, expression) in expressions.iter().enumerate() {
            assert_eq!(
                expression.as_bytes(),
                raw(&expected, &format!("outer-{index}.expression.bin"))
            );
            probe.start_path(index as u32, 0, None).unwrap();
            let event = host.settle(&mut probe, 0);
            host.completion(&event, &format!("outer-{index}"));
        }
        let counter = text(&expected, "counters.bin");
        for (key, count) in [
            ("resolve", host.resolves),
            ("source", host.sources),
            ("read", host.reads),
            ("log", host.logs),
            ("callback", host.callback_count),
            ("event", host.events),
        ] {
            assert_eq!(
                counter
                    .lines()
                    .find_map(|l| l.strip_prefix(&format!("{key}=")))
                    .unwrap()
                    .parse::<usize>()
                    .unwrap(),
                count,
                "{id} {key}"
            );
        }
        probe.close();
        assert_eq!(probe.collect().slots, 0);
    }
}
macro_rules! java_case {
    ($name:ident,$group:literal,$id:literal) => {
        #[test]
        fn $name() {
            ordinary($group, $id);
        }
    };
}
java_case!(
    normal_nested_and_argument_failures,
    "initial",
    "normal-error-nested-catch"
);
java_case!(
    failed_lazy_value_and_caught_success_cache,
    "initial",
    "lazy-guest-failure-and-cache"
);
java_case!(
    resource_retry_after_guest_catch,
    "initial",
    "resource-retry-after-guest-catch"
);
java_case!(
    resource_callback_nested_guest_catch,
    "initial",
    "read-callback-nested-catch"
);
java_case!(
    logger_callback_nested_guest_catch,
    "initial",
    "logger-callback-nested-catch"
);
java_case!(
    renderer_java_lines_strip_and_unpaired_units,
    "initial",
    "renderer-profile-lines-strip-utf16"
);
java_case!(
    stdlib_singletons_and_builtin_declaration_identity,
    "corrected",
    "stdlib-singleton-and-builtin-identity"
);
java_case!(
    module_resolve_failure_retries,
    "corrected",
    "resolve-retry-after-guest-catch"
);
java_case!(
    module_source_failure_caches_original_frames,
    "corrected",
    "source-failure-cache-after-guest-catch"
);
java_case!(
    completed_source_callback_survives_outer_caught_failure,
    "corrected",
    "source-callback-inner-publication-and-catch"
);

#[test]
fn original_blocked_sources_keep_constructor_boundary_separate_from_api_followups() {
    for id in [
        "stdlib-singleton-and-builtin-identity",
        "resolve-retry-after-guest-catch",
        "source-failure-cache-after-guest-catch",
        "source-callback-inner-publication-and-catch",
    ] {
        let f = fixture("initial", id);
        let error = match new_probe(&f) {
            Ok(_) => panic!("original authored syntax unexpectedly accepted"),
            Err(error) => error,
        };
        // Java rejects at evaluateExpressionString; this adapter rejects at construction.
        // Verify text only, retaining all Java followups as distinct unexecuted API boundaries.
        for repeat in 0..2 {
            let e = expected("initial", id, repeat);
            for index in 0..lines(&f, "expressions.txt").len() {
                compare(
                    &error.encode_utf16().collect::<Vec<_>>(),
                    &e,
                    &format!("outer-{index}.stderr"),
                );
            }
        }
    }
}

fn next(probe: &mut SourceProbe, now: u32) -> SourceEvent {
    for _ in 0..200_000 {
        let event = probe.poll(now, 1).unwrap();
        probe.collect();
        if event.kind == "module-request"
            && probe
                .complete_default_module(event.token.as_deref().unwrap(), now)
                .unwrap()
        {
            continue;
        }
        if event.kind != "yield" {
            return event;
        }
    }
    panic!("terminal control did not settle");
}
#[test]
fn timeout_bypasses_guest_catch_and_releases_pending_host_token() {
    for id in [
        "read-timeout-not-ordinary-catch",
        "source-timeout-not-ordinary-catch",
    ] {
        for repeat in 0..2 {
            let f = fixture("initial", id);
            let expected = expected("initial", id, repeat);
            let mut probe = new_probe(&f).unwrap();
            probe.start_path(0, 0, Some(3000)).unwrap();
            compare_event(&next(&mut probe, 0), &expected, "outer-0");
            probe.start_path(1, 0, Some(3000)).unwrap();
            let mut event = next(&mut probe, 0);
            if event.kind == "module-request" && event.module.as_ref().unwrap().kind == "resolve" {
                probe
                    .complete_module_resolution(
                        event.token.as_deref().unwrap(),
                        "catch:/child",
                        "catch:/child",
                        "catch:/child",
                        0,
                    )
                    .unwrap();
                event = next(&mut probe, 0);
            }
            assert!(matches!(event.kind, "read" | "module-request"));
            let token = event.token.unwrap();
            let failure = next(&mut probe, 3001);
            assert_eq!(failure.kind, "error");
            // Existing development frontend classification maps Timeout to LanguageError.
            // The completed timeout closes the source session; later admission reports Closed.
            // Java later executor failures remain a separate unimplemented API boundary.
            assert_eq!(failure.disposition, Some("language-error"));
            assert_eq!(
                probe.start_path(2, 3001, None).unwrap_err().kind,
                ErrorKind::Closed
            );
            let aborted = if id.starts_with("read-") {
                probe.drain_cancelled_reads()
            } else {
                probe.drain_cancelled_modules()
            };
            assert_eq!(aborted, vec![token]);
            assert!(probe.drain_cancelled_reads().is_empty());
            assert!(probe.drain_cancelled_modules().is_empty());
            // Java's later RejectedExecutionException is retained raw, not imitated or counted compatible.
            probe.close();
            assert_eq!(probe.collect().slots, 0);
        }
    }
}
#[test]
fn close_allows_current_guest_body_to_return_then_denies_new_calls() {
    for id in [
        "read-close-not-ordinary-catch",
        "source-close-not-ordinary-catch",
    ] {
        for repeat in 0..2 {
            let f = fixture("initial", id);
            let expected = expected("initial", id, repeat);
            let mut probe = new_probe(&f).unwrap();
            probe.start_path(0, 0, None).unwrap();
            compare_event(&next(&mut probe, 0), &expected, "outer-0");
            probe.start_path(1, 0, None).unwrap();
            let mut event = next(&mut probe, 0);
            if event.kind == "module-request" && event.module.as_ref().unwrap().kind == "resolve" {
                probe
                    .complete_module_resolution(
                        event.token.as_deref().unwrap(),
                        "catch:/child",
                        "catch:/child",
                        "catch:/child",
                        0,
                    )
                    .unwrap();
                event = next(&mut probe, 0);
            }
            let token = event.token.unwrap();
            probe.close_from_callback(&token).unwrap();
            if event.kind == "read" {
                probe.respond(&token, Value::String("9".into()), 0).unwrap();
            } else {
                assert_eq!(event.module.unwrap().kind, "load-source");
                probe
                    .complete_module_source(&token, "value = 9\n", "child", 0)
                    .unwrap();
            }
            // Current-call language error is exact Java; later cancellation is an explicit boundary.
            compare_event(&next(&mut probe, 0), &expected, "outer-1");
            assert!(probe.start_path(2, 0, None).is_err());
            assert!(probe.start_path(3, 0, None).is_err());
            probe.close();
            assert_eq!(probe.collect().slots, 0);
        }
    }
}

java_case!(
    returned_lazy_object_stays_unforced,
    "lazy-return",
    "returned-lazy-object-versus-forced-undefined"
);
java_case!(
    nearest_guest_handler_inside_outer_undefined_force,
    "corrected-force",
    "nearest-guest-handler-inside-allow-undefined-force"
);
#[test]
fn initial_force_source_keeps_constructor_boundary() {
    let id = "nearest-guest-handler-inside-allow-undefined-force";
    let f = fixture("initial-force", id);
    let error = match new_probe(&f) {
        Ok(_) => panic!("original authored syntax unexpectedly accepted"),
        Err(error) => error,
    };
    for repeat in 0..2 {
        let e = expected("initial-force", id, repeat);
        for index in 0..lines(&f, "expressions.txt").len() {
            compare(
                &error.encode_utf16().collect::<Vec<_>>(),
                &e,
                &format!("outer-{index}.stderr"),
            );
        }
    }
}
