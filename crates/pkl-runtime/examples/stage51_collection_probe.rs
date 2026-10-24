//! Collection differential capture through the public shared evaluator.
//! The Host supplies an exact virtual primary module and explicit reader rules.
//! No Java administrative events or expected evaluation results are synthesized.
use pkl_runtime::{
    host::{IoFailure, ReadFailure},
    modules::{ModuleKeyKind, ModuleRequestKind},
    probe::{SourceEvent, SourceLog, SourceProbe},
    resources::ReadResult,
    value::PklString,
};
use std::{collections::HashMap, fs, path::Path};

fn write(out: &Path, name: &str, value: impl AsRef<[u8]>) {
    fs::write(out.join(name), value).expect("write collection observation");
}
fn units(out: &Path, name: &str, value: &[u16]) {
    write(
        out,
        &format!("{name}.utf16be.bin"),
        value
            .iter()
            .flat_map(|unit| unit.to_be_bytes())
            .collect::<Vec<_>>(),
    );
    write(
        out,
        &format!("{name}.bin"),
        PklString::from_utf16(value.to_vec()).to_java_utf8(),
    );
}
fn text(out: &Path, name: &str, value: &str) {
    units(out, name, &value.encode_utf16().collect::<Vec<_>>());
}
fn decode(value: &str) -> String {
    assert!(value.len().is_multiple_of(2), "even input hex");
    let bytes = (0..value.len())
        .step_by(2)
        .map(|index| u8::from_str_radix(&value[index..index + 2], 16).expect("hex input"))
        .collect::<Vec<_>>();
    String::from_utf8(bytes).expect("UTF8 input")
}
fn resource_policy(uri: &str) -> Result<(), String> {
    // The pinned CollectionProbe configures Pattern.compile("ct:.*").
    // SecurityManagers.checkRead uses lookingAt(), not matches(): the literal
    // prefix is sufficient even when the remainder contains a line terminator.
    if uri.starts_with("ct:") {
        Ok(())
    } else {
        Err(format!(
            "Refusing to read resource `{uri}` because it does not match any entry in the resource allowlist (`--allowed-resources`)."
        ))
    }
}
struct Input {
    source: String,
    uri: String,
    name: String,
    operations: Vec<(String, String)>,
    readers: HashMap<String, (String, String)>,
}
#[derive(Default)]
struct Provider {
    keys: HashMap<u64, String>,
    resolved: HashMap<u64, (u64, String)>,
    attempts: HashMap<String, usize>,
    requests: usize,
    resource_policy_requests: usize,
    reads: usize,
    logs: usize,
    effects: usize,
    collections: usize,
}
impl Provider {
    fn module(&mut self, probe: &mut SourceProbe, event: &SourceEvent, input: &Input, out: &Path) {
        let token = event.token.as_deref().expect("module token");
        let request = probe.module_request(token).expect("typed module request");
        write(
            out,
            &format!("module-{}.request.bin", self.requests),
            format!("{request:?}"),
        );
        write(out, &format!("module-{}.token.bin", self.requests), token);
        self.requests += 1;
        match &request.kind {
            ModuleRequestKind::SelectKey { id, uri, .. } if uri.as_ref() == input.uri => {
                let handle = format!("fixture-key:{}", id.0);
                assert!(self.keys.insert(id.0, handle.clone()).is_none());
                probe
                    .complete_module_key(token, &id.0.to_string(), uri, "host", &handle, 0)
                    .expect("primary key completion");
                return;
            }
            ModuleRequestKind::QueryCachePolicy { key }
            | ModuleRequestKind::QueryFragmentPaths { key }
                if key.uri.as_ref() == input.uri && key.kind == ModuleKeyKind::Host =>
            {
                assert_eq!(
                    self.keys.get(&key.id.0).map(String::as_str),
                    Some(key.handle.as_ref())
                );
                if matches!(&request.kind, ModuleRequestKind::QueryCachePolicy { .. }) {
                    probe
                        .complete_module_cache_policy(token, &key.id.0.to_string(), true, 0)
                        .expect("primary cache policy");
                } else {
                    probe
                        .complete_module_fragment_paths(token, &key.id.0.to_string(), false, 0)
                        .expect("primary fragment policy");
                }
                return;
            }
            ModuleRequestKind::Resolve {
                key,
                resolved_key_id,
                ..
            }
            | ModuleRequestKind::ResolveKey {
                key,
                resolved_key_id,
                ..
            } if key.uri.as_ref() == input.uri && key.kind == ModuleKeyKind::Host => {
                assert_eq!(
                    self.keys.get(&key.id.0).map(String::as_str),
                    Some(key.handle.as_ref())
                );
                let handle = format!("collection-resolved:{}", resolved_key_id.0);
                assert!(
                    self.resolved
                        .insert(resolved_key_id.0, (key.id.0, handle.clone()))
                        .is_none()
                );
                probe
                    .complete_module_resolved_key(
                        token,
                        &key.id.0.to_string(),
                        &resolved_key_id.0.to_string(),
                        &handle,
                        &input.uri,
                        &input.uri,
                        &input.uri,
                        0,
                    )
                    .expect("primary resolution");
                return;
            }
            ModuleRequestKind::LoadSource { key, resolved }
                if key.key.uri.as_ref() == input.uri && key.key.kind == ModuleKeyKind::Host =>
            {
                assert_eq!(
                    self.keys.get(&key.key.id.0).map(String::as_str),
                    Some(key.key.handle.as_ref())
                );
                assert_eq!(
                    self.resolved.get(&key.id.0),
                    Some(&(key.key.id.0, key.handle.to_string()))
                );
                assert_eq!(resolved.original_uri.as_ref(), input.uri);
                assert_eq!(resolved.resolved_uri.as_ref(), input.uri);
                assert_eq!(resolved.source_uri.as_ref(), input.uri);
                probe
                    .complete_module_source(token, &input.source, &input.name, 0)
                    .expect("primary source");
                return;
            }
            ModuleRequestKind::Capabilities { key: Some(key), .. }
                if key.uri.as_ref() == input.uri && key.kind == ModuleKeyKind::Host =>
            {
                assert_eq!(
                    self.keys.get(&key.id.0).map(String::as_str),
                    Some(key.handle.as_ref())
                );
                probe
                    .complete_module_capabilities(token, true, true, false, 0)
                    .expect("primary capabilities");
                return;
            }
            _ => {}
        }
        assert!(
            probe
                .complete_default_module(token, 0)
                .expect("default module completion"),
            "unconfigured module operation: {request:?}"
        );
    }
    fn prefix(&mut self, out: &Path, kind: &str, call: usize) -> String {
        let prefix = format!("effect-{}", self.effects);
        self.effects += 1;
        text(out, &format!("{prefix}.kind"), kind);
        text(out, &format!("{prefix}.call-index"), &call.to_string());
        prefix
    }
    fn log(&mut self, out: &Path, call: usize, log: SourceLog) {
        self.logs += 1;
        let p = self.prefix(out, "log", call);
        text(out, &format!("{p}.level"), log.level);
        units(out, &format!("{p}.message"), &log.message);
        units(out, &format!("{p}.module-uri"), &log.module_uri);
        text(
            out,
            &format!("{p}.member-null"),
            if log.member_name.is_none() {
                "true"
            } else {
                "false"
            },
        );
        units(
            out,
            &format!("{p}.member"),
            log.member_name.as_deref().unwrap_or(&[]),
        );
        text(
            out,
            &format!("{p}.position"),
            &format!(
                "{}:{}:{}:{}",
                log.start_line, log.start_column, log.end_line, log.end_column
            ),
        );
        text(
            out,
            &format!("{p}.source-lines"),
            &log.source_lines.len().to_string(),
        );
        for (index, line) in log.source_lines.iter().enumerate() {
            units(out, &format!("{p}.source-{index}"), line);
        }
    }
    fn read(
        &mut self,
        probe: &mut SourceProbe,
        event: &SourceEvent,
        input: &Input,
        out: &Path,
        call: usize,
    ) {
        let token = event.token.as_deref().expect("read token");
        let uri = probe.resolved_read_uri(token).expect("resolved read URI");
        assert_eq!(event.uri.as_deref(), Some(uri.as_str()));
        let policy = resource_policy(&uri);
        let p = format!("resource-policy-{}", self.resource_policy_requests);
        self.resource_policy_requests += 1;
        text(out, &format!("{p}.call-index"), &call.to_string());
        text(out, &format!("{p}.uri"), &uri);
        text(
            out,
            &format!("{p}.allowed"),
            if policy.is_ok() { "true" } else { "false" },
        );
        text(
            out,
            &format!("{p}.message"),
            policy.as_ref().err().map_or("", String::as_str),
        );
        text(out, &format!("{p}.token"), token);
        if let Err(message) = policy {
            probe
                .reject_read_security_manager(token, Some(message.into()), 0)
                .expect("typed resource security rejection");
            return;
        }
        // Reader configuration and attempt/effect accounting begin only after
        // permission. An allowed but unconfigured URI is an acquisition error.
        let (kind, value) = input.readers.get(&uri).expect("unconfigured reader URI");
        let attempt = self.attempts.entry(uri.clone()).or_default();
        *attempt += 1;
        let attempt = *attempt;
        self.reads += 1;
        let p = self.prefix(out, "read", call);
        text(out, &format!("{p}.uri"), &uri);
        text(out, &format!("{p}.attempt"), &attempt.to_string());
        let response = if kind == "io" || (kind == "io-first" && attempt == 1) {
            text(out, &format!("{p}.result"), "io");
            text(out, &format!("{p}.message"), value);
            Err(ReadFailure::Io(IoFailure {
                message: Some(value.as_str().into()),
            }))
        } else {
            assert!(kind == "text" || kind == "io-first");
            let value = if kind == "io-first" {
                "recovered"
            } else {
                value
            };
            text(out, &format!("{p}.result"), "text");
            text(out, &format!("{p}.value"), value);
            Ok(ReadResult::Text(value.into()))
        };
        probe
            .complete_read(token, response, 0)
            .expect("typed read completion");
    }
}
fn capture(input: &Input, out: &Path) {
    fs::create_dir(out).expect("fresh collection session");
    let mut probe = SourceProbe::new_module_evaluator("compact", true, false, false)
        .expect("public evaluator construction");
    let mut host = Provider::default();
    let mut index = 0;
    let mut polls = 0;
    let mut retained = Vec::new();
    for (operation, expression) in &input.operations {
        match operation.as_str() {
            "gc" => {
                assert!(expression.is_empty());
                probe.collect();
                host.collections += 1;
                continue;
            }
            "close" => {
                assert!(expression.is_empty());
                probe.close();
                continue;
            }
            "evaluate" => {}
            _ => panic!("unknown input operation"),
        }
        let p = format!("call-{index}");
        text(out, &format!("{p}.expression"), expression);
        text(out, &format!("{p}.reads-before"), &host.reads.to_string());
        text(out, &format!("{p}.logs-before"), &host.logs.to_string());
        units(out, &format!("{p}.stdout"), &[]);
        units(out, &format!("{p}.stderr"), &[]);
        text(out, &format!("{p}.disposition"), "");
        text(out, &format!("{p}.phase"), "");
        if let Err(error) =
            probe.evaluate_module_source(&input.uri, None, expression, 0, Some(8000))
        {
            text(out, &format!("{p}.status"), "api-error\n");
            units(
                out,
                &format!("{p}.stderr"),
                &error.message.encode_utf16().collect::<Vec<_>>(),
            );
            write(
                out,
                &format!("{p}.api-error-kind.bin"),
                format!("{:?}", error.kind),
            );
        } else {
            loop {
                polls += 1;
                assert!(polls <= 1_000_000, "capture poll bound exceeded");
                let event = probe.poll(0, 1).expect("poll collection evaluator");
                probe.collect();
                host.collections += 1;
                match event.kind {
                    "yield" => {}
                    "module-request" => host.module(&mut probe, &event, input, out),
                    "read" => host.read(&mut probe, &event, input, out, index),
                    "log" => {
                        host.log(out, index, event.log.expect("logger payload"));
                        probe
                            .acknowledge_log(event.token.as_deref().expect("logger token"), None, 0)
                            .expect("logger acknowledgement");
                    }
                    "value" | "error" => {
                        let (status, channel, value) = if event.kind == "value" {
                            ("success\n", "stdout", event.value.expect("result units"))
                        } else {
                            ("error\n", "stderr", event.error.expect("error units"))
                        };
                        text(out, &format!("{p}.status"), status);
                        units(out, &format!("{p}.{channel}"), &value);
                        text(
                            out,
                            &format!("{p}.disposition"),
                            event.disposition.unwrap_or(""),
                        );
                        text(out, &format!("{p}.phase"), event.phase.unwrap_or(""));
                        retained.push(value);
                        break;
                    }
                    other => panic!("unexpected collection event: {other}"),
                }
            }
        }
        text(out, &format!("{p}.reads-after"), &host.reads.to_string());
        text(out, &format!("{p}.logs-after"), &host.logs.to_string());
        index += 1;
    }
    probe.close();
    probe.collect();
    host.collections += 1;
    text(out, "calls", &index.to_string());
    text(out, "reads", &host.reads.to_string());
    text(out, "logs", &host.logs.to_string());
    text(out, "effects", &host.effects.to_string());
    text(out, "module-requests", &host.requests.to_string());
    text(
        out,
        "resource-policy-requests",
        &host.resource_policy_requests.to_string(),
    );
    text(out, "collections", &host.collections.to_string());
    text(out, "terminal-slots", &probe.heap_slots().to_string());
    text(out, "retained-output-buffers", &retained.len().to_string());
    // Owned output arrays survive close; they are not retained guest heap values.
    for (index, value) in retained.iter().enumerate() {
        units(out, &format!("retained-{index}"), value);
    }
    host.keys.clear();
    host.resolved.clear();
    host.attempts.clear();
    text(
        out,
        "provider-entries",
        &(host.keys.len() + host.resolved.len() + host.attempts.len()).to_string(),
    );
}
fn main() {
    let args = std::env::args().collect::<Vec<_>>();
    assert_eq!(
        args.len(),
        7,
        "SOURCE URI NAME OPERATIONS READER_PLAN OUTPUT"
    );
    let operations = fs::read_to_string(&args[4])
        .expect("operations")
        .lines()
        .map(|line| {
            let fields = line.split('\t').collect::<Vec<_>>();
            assert_eq!(fields.len(), 2);
            (fields[0].to_owned(), decode(fields[1]))
        })
        .collect();
    let mut readers = HashMap::new();
    for line in fs::read_to_string(&args[5]).expect("reader rules").lines() {
        let fields = line.split('\t').collect::<Vec<_>>();
        assert_eq!(fields.len(), 3);
        assert!(matches!(fields[1], "text" | "io-first" | "io"));
        assert!(
            readers
                .insert(
                    fields[0].to_owned(),
                    (fields[1].to_owned(), decode(fields[2]))
                )
                .is_none()
        );
    }
    let input = Input {
        source: fs::read_to_string(&args[1]).expect("source UTF8"),
        uri: args[2].clone(),
        name: args[3].clone(),
        operations,
        readers,
    };
    let out = Path::new(&args[6]);
    fs::create_dir(out).expect("fresh process output");
    for session in 0..2 {
        capture(&input, &out.join(format!("session-{session}")));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        path::PathBuf,
        sync::atomic::{AtomicUsize, Ordering},
    };

    fn output_path(name: &str) -> PathBuf {
        static SEQUENCE: AtomicUsize = AtomicUsize::new(0);
        let root = std::env::var_os("PKL_STAGE51_POLICY_TEST_OUTPUT").map_or_else(
            || Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/m2"),
            PathBuf::from,
        );
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = root.join(format!(
            "stage51-policy-{name}-{}-{stamp}-{}",
            std::process::id(),
            SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        assert!(!path.exists(), "fresh test observation directory");
        // capture() creates its directory. Keeping these raw observations also
        // preserves the original outputs when a regression fails.
        path
    }

    fn input(operations: &[&str]) -> Input {
        Input {
            source: "value=1\n".into(),
            uri: "file:///stage51-resource-policy.pkl".into(),
            name: "stage51-resource-policy".into(),
            operations: operations
                .iter()
                .map(|expression| ("evaluate".into(), (*expression).into()))
                .collect(),
            readers: HashMap::new(),
        }
    }

    fn observed(out: &Path, name: &str) -> String {
        let utf8 = fs::read(out.join(format!("{name}.bin"))).unwrap();
        let utf16 = fs::read(out.join(format!("{name}.utf16be.bin"))).unwrap();
        let (pairs, remainder) = utf16.as_chunks::<2>();
        assert!(remainder.is_empty());
        let value = String::from_utf16(
            &pairs
                .iter()
                .map(|pair| u16::from_be_bytes(*pair))
                .collect::<Vec<_>>(),
        )
        .unwrap();
        assert_eq!(utf8, value.as_bytes(), "both recorded encodings: {name}");
        value
    }

    #[test]
    fn policy_matches_java_looking_at_prefix_not_whole_string_or_case_folding() {
        // This is the policy's string matcher, not URI parsing. lookingAt can
        // match just `ct:` before a line terminator, because .* may be empty.
        for uri in [
            "ct:",
            "ct:/value",
            "ct:SomeValue",
            "ct:\nrest",
            "ct:\r\nrest",
            "ct:\u{2028}rest",
        ] {
            assert_eq!(resource_policy(uri), Ok(()), "{uri:?}");
        }
        for uri in [
            "CT:/value",
            "Ct:/value",
            "ct",
            "xct:/value",
            "ctX:/value",
            "prop:pkl.outputFormat",
        ] {
            assert_eq!(
                resource_policy(uri),
                Err(format!(
                    "Refusing to read resource `{uri}` because it does not match any entry in the resource allowlist (`--allowed-resources`)."
                ))
            );
        }
    }

    #[test]
    fn output_policy_denial_precedes_reader_dispatch_and_keeps_a_separate_ledger() {
        let mut input = input(&[
            "output.text",
            "read(\"ct:known\")",
            "read(\"ct:known\")",
            "read(\"prop:blocked\")",
        ]);
        input
            .readers
            .insert("ct:known".into(), ("text".into(), "reader-value".into()));
        // Even a configured disallowed URI must never enter this reader rule.
        input
            .readers
            .insert("prop:blocked".into(), ("io".into(), "must-not-read".into()));
        let out = output_path("ordering");
        capture(&input, &out);
        assert_eq!(observed(&out, "calls"), "4");
        assert_eq!(observed(&out, "resource-policy-requests"), "3");
        assert_eq!(observed(&out, "reads"), "1");
        assert_eq!(observed(&out, "logs"), "0");
        assert_eq!(observed(&out, "effects"), "1");
        assert_eq!(observed(&out, "terminal-slots"), "0");
        assert_eq!(observed(&out, "provider-entries"), "0");
        assert_eq!(observed(&out, "call-0.status"), "error\n");
        assert_eq!(observed(&out, "call-0.reads-before"), "0");
        assert_eq!(observed(&out, "call-0.reads-after"), "0");
        assert_eq!(observed(&out, "call-3.status"), "error\n");
        for call in [1, 2] {
            assert_eq!(observed(&out, &format!("call-{call}.status")), "success\n");
            assert_eq!(
                observed(&out, &format!("call-{call}.stdout")),
                "reader-value"
            );
            assert_eq!(observed(&out, &format!("call-{call}.reads-after")), "1");
        }
        let mut tokens = Vec::new();
        for (index, call, uri, allowed) in [
            (0, 0, "prop:pkl.outputFormat", false),
            (1, 1, "ct:known", true),
            (2, 3, "prop:blocked", false),
        ] {
            let prefix = format!("resource-policy-{index}");
            assert_eq!(
                observed(&out, &format!("{prefix}.call-index")),
                call.to_string()
            );
            assert_eq!(observed(&out, &format!("{prefix}.uri")), uri);
            assert_eq!(
                observed(&out, &format!("{prefix}.allowed")),
                allowed.to_string()
            );
            assert_eq!(
                observed(&out, &format!("{prefix}.message")),
                resource_policy(uri).err().unwrap_or_default()
            );
            let token = observed(&out, &format!("{prefix}.token"));
            assert!(!token.is_empty());
            assert!(!tokens.contains(&token));
            tokens.push(token);
        }
        assert_eq!(observed(&out, "effect-0.kind"), "read");
        assert_eq!(observed(&out, "effect-0.call-index"), "1");
        assert_eq!(observed(&out, "effect-0.uri"), "ct:known");
        assert_eq!(observed(&out, "effect-0.attempt"), "1");
        assert!(!out.join("effect-1.kind.bin").exists());
    }

    #[test]
    fn allowed_unconfigured_reader_remains_an_acquisition_failure_after_policy() {
        let input = input(&["read(\"ct:missing\")"]);
        let out = output_path("unconfigured");
        fs::create_dir(&out).unwrap();
        let mut probe = SourceProbe::new_module_evaluator("compact", true, false, false).unwrap();
        let mut host = Provider::default();
        probe
            .evaluate_module_source(&input.uri, None, &input.operations[0].1, 0, Some(8000))
            .unwrap();
        let mut pending = None;
        for _ in 0..100_000 {
            let event = probe.poll(0, 1).unwrap();
            probe.collect();
            match event.kind {
                "yield" => {}
                "module-request" => host.module(&mut probe, &event, &input, &out),
                "read" => {
                    pending = Some(event);
                    break;
                }
                other => panic!("expected actual read, got {other}"),
            }
        }
        let pending = pending.expect("bounded read acquisition");
        let failure = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            host.read(&mut probe, &pending, &input, &out, 0);
        }))
        .expect_err("unconfigured allowed reader must fail acquisition");
        let message = failure
            .downcast_ref::<String>()
            .map(String::as_str)
            .or_else(|| failure.downcast_ref::<&str>().copied());
        assert_eq!(message, Some("unconfigured reader URI"));
        assert_eq!(host.resource_policy_requests, 1);
        assert_eq!(host.reads, 0);
        assert_eq!(host.effects, 0);
        assert!(host.attempts.is_empty());
        assert_eq!(observed(&out, "resource-policy-0.allowed"), "true");
        assert_eq!(observed(&out, "resource-policy-0.message"), "");
        assert_eq!(observed(&out, "resource-policy-0.uri"), "ct:missing");
        assert_eq!(
            observed(&out, "resource-policy-0.token"),
            pending.token.clone().unwrap()
        );
        // No fabricated guest failure or read response consumed the request.
        assert_eq!(probe.poll(0, 1).unwrap().token, pending.token);
        probe.close();
        assert_eq!(probe.collect().slots, 0);
    }
}
