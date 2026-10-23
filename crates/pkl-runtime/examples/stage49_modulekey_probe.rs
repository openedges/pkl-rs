//! Development evidence adapter. The shared runtime owns import sites, URI/cache semantics and guest evaluation.
//! Host responses use explicit policy inputs and captured raw SecurityManagerException causes only.
use pkl_runtime::{
    modules::{ModuleCompletion, ModuleHostError, ModuleRequest, ModuleRequestKind},
    probe::SourceProbe,
    value::PklString,
};
use std::{fs, path::Path};
#[path = "stage48_policy_probe/file_host.rs"]
mod file_host;
#[path = "stage49_modulekey_probe/module_keys.rs"]
mod module_keys;

fn write(out: &Path, name: &str, value: impl AsRef<[u8]>) {
    fs::write(out.join(name), value).expect("write capture");
}
fn text(out: &Path, prefix: &str, channel: &str, value: &[u16]) {
    write(
        out,
        &format!("{prefix}.{channel}.code-units.bin"),
        value
            .iter()
            .flat_map(|u| u.to_be_bytes())
            .collect::<Vec<_>>(),
    );
    write(
        out,
        &format!("{prefix}.{channel}.bin"),
        PklString::from_utf16(value.to_vec()).to_java_utf8(),
    );
}
fn hex(value: &str) -> Vec<u8> {
    assert!(value.len().is_multiple_of(2));
    (0..value.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&value[i..i + 2], 16).expect("hex"))
        .collect()
}
fn utf8(value: &str) -> String {
    String::from_utf8(hex(value)).expect("UTF8 field")
}
fn utf16(value: &str) -> PklString {
    let bytes = hex(value);
    assert!(bytes.len().is_multiple_of(2));
    PklString::from_utf16(
        bytes
            .as_chunks::<2>()
            .0
            .iter()
            .map(|[a, b]| u16::from_be_bytes([*a, *b]))
            .collect::<Vec<_>>(),
    )
}
struct Cause {
    mode: String,
    kind: String,
    context: String,
    message: Option<PklString>,
}
fn causes(path: &Path) -> Vec<Cause> {
    fs::read_to_string(path)
        .expect("cause plan")
        .lines()
        .map(|line| {
            let f = line.split('\t').collect::<Vec<_>>();
            assert_eq!(f.len(), 5);
            Cause {
                mode: utf8(f[0]),
                kind: f[1].to_owned(),
                context: utf8(f[2]),
                message: match f[3] {
                    "true" => {
                        assert!(f[4].is_empty());
                        None
                    }
                    "false" => Some(utf16(f[4])),
                    _ => panic!("cause null flag"),
                },
            }
        })
        .collect()
}
fn denial(request: &ModuleRequest, mode: &str, inputs: &[Cause]) -> Option<ModuleCompletion> {
    let (kind, context, required) = match &request.kind {
        ModuleRequestKind::CheckImport {
            importer,
            import_uri,
        } => (
            "import",
            format!("{} -> {import_uri}", importer.resolved_uri),
            match mode {
                "trust-base" => import_uri.as_ref() == "pkl:base",
                "trust-ref" => import_uri.as_ref() == "pkl:ref",
                "trust-json" => import_uri.as_ref() == "pkl:json",
                "allow" | "deny-json" | "deny-singletons" | "deny-repl" | "deny-supplied" => false,
                _ => panic!("unknown policy"),
            },
        ),
        ModuleRequestKind::ResolveKey { key, .. } => (
            "module",
            key.uri.to_string(),
            match mode {
                "deny-json" => key.uri.as_ref() == "pkl:json",
                "deny-repl" => key.uri.starts_with("repl:"),
                "deny-supplied" => key.uri.as_ref() == "memo:/supplied.pkl",
                "deny-singletons" => {
                    matches!(key.uri.as_ref(), "pkl:base" | "pkl:ref" | "pkl:test")
                }
                "allow" | "trust-base" | "trust-ref" | "trust-json" => false,
                _ => panic!("unknown policy"),
            },
        ),
        ModuleRequestKind::Resolve { import_uri, .. } => (
            "module",
            import_uri.to_string(),
            match mode {
                "deny-json" => import_uri.as_ref() == "pkl:json",
                "deny-repl" => import_uri.starts_with("repl:"),
                "deny-supplied" => import_uri.as_ref() == "memo:/supplied.pkl",
                "deny-singletons" => {
                    matches!(import_uri.as_ref(), "pkl:base" | "pkl:ref" | "pkl:test")
                }
                "allow" | "trust-base" | "trust-ref" | "trust-json" => false,
                _ => panic!("unknown policy"),
            },
        ),
        _ => return None,
    };
    if !required {
        return None;
    }
    let found = inputs
        .iter()
        .find(|c| c.mode == mode && c.kind == kind && c.context == context);
    let error = if let Some(cause) = found {
        ModuleHostError::SecurityManagerException {
            message: cause.message.clone(),
        }
    } else {
        ModuleHostError::Unsupported("Custom policy requires an exact captured raw cause for this actual context; no fabricated diagnostic or fallback.".into())
    };
    Some(match &request.kind {
        ModuleRequestKind::CheckImport { .. } => ModuleCompletion::ImportChecked(Err(error)),
        ModuleRequestKind::Resolve { .. } | ModuleRequestKind::ResolveKey { .. } => {
            ModuleCompletion::Resolved(Err(error))
        }
        _ => unreachable!(),
    })
}
fn event(out: &Path, index: &mut usize, line: &str) {
    text(
        out,
        &format!("event-{index}"),
        "text",
        &line.encode_utf16().collect::<Vec<_>>(),
    );
    *index += 1;
}
fn counters(requests: usize, imports: usize, resolves: usize, events: usize) -> String {
    format!("requests={requests}\nimport={imports}\nresolve={resolves}\nevent={events}\n")
}
struct CaptureInput<'a> {
    source: &'a str,
    uri: &'a str,
    name: &'a str,
    operations: &'a [(String, String)],
    initial_policy: &'a str,
    causes: &'a [Cause],
    modules: &'a Path,
    public_lifecycle: bool,
    profile: &'a str,
}
fn capture(out: &Path, input: &CaptureInput<'_>) {
    let (source, uri, name, operations, mode, inputs) = (
        input.source,
        input.uri,
        input.name,
        input.operations,
        input.initial_policy,
        input.causes,
    );
    fs::create_dir(out).expect("fresh session");
    let mut modules = module_keys::Factory::new(input.modules);
    write(
        out,
        "module-policy-profile.bin",
        if input.profile == "public-module-source-extended-v1" {
            "custom-key-first/separate-cache-policy/explicit-shared-default/file-host-v1\npublic-module-source-extended-v1\n"
        } else if input.public_lifecycle {
            "custom-key-first/separate-cache-policy/explicit-shared-default/file-host-v1\npublic-module-source-uri-v1\n"
        } else {
            "custom-key-first/separate-cache-policy/explicit-shared-default/file-host-v1\nprovided-primary-text/no-Java-primary-repl-lifecycle\n"
        },
    );
    let expressions = operations
        .iter()
        .filter(|(kind, _)| kind == "evaluate")
        .map(|(_, v)| v.clone())
        .collect::<Vec<_>>();
    let constructed = if input.public_lifecycle {
        SourceProbe::new_module_evaluator("compact", false, false, true)
    } else {
        SourceProbe::new_expressions_text(
            source,
            uri,
            name,
            &expressions,
            "compact",
            false,
            false,
            true,
        )
    };
    let mut probe = match constructed {
        Ok(p) => p,
        Err(e) => {
            text(
                out,
                "constructor",
                "stderr",
                &e.encode_utf16().collect::<Vec<_>>(),
            );
            write(out, "constructor.status.bin", "constructor-error\n");
            return;
        }
    };
    let (mut request_index, mut imports, mut resolves, mut events, mut index, mut collections) =
        (0, 0, 0, 0, 0, 0);
    let mut mode = mode.to_owned();
    for (kind, value) in operations {
        match kind.as_str() {
            "gc" => {
                probe.collect();
                collections += 1;
                event(out, &mut events, "native-explicit-collect");
                continue;
            }
            "cache" => {
                modules.cached = match value.as_str() {
                    "on" => true,
                    "off" => false,
                    _ => panic!("cache operation"),
                };
                event(out, &mut events, &format!("cache-profile {value}"));
                continue;
            }
            "policy" => {
                mode = value.clone();
                event(out, &mut events, &format!("policy-profile {mode}"));
                continue;
            }
            "source" => {
                modules.set_source(value);
                event(out, &mut events, &format!("source-profile {value}"));
                continue;
            }
            "factory" => {
                modules.custom = match value.as_str() {
                    "custom" => true,
                    "builtin" => false,
                    _ => panic!("factory profile"),
                };
                event(out, &mut events, &format!("factory-profile {value}"));
                continue;
            }
            "evaluate" | "evaluate-source" => {}
            _ => panic!("Unsupported operation; no silent cache/factory/lifecycle emulation"),
        }
        let explicit = (kind == "evaluate-source").then(|| {
            assert!(
                input.public_lifecycle,
                "per-operation ModuleSource requires public lifecycle"
            );
            let (profile, expression) = value.split_once(':').expect("profile/expression fields");
            (modules.profile(&utf8(profile)), utf8(expression))
        });
        let expression = explicit
            .as_ref()
            .map_or(value.as_str(), |(_, expression)| expression.as_str());
        let prefix = format!("outer-{index}");
        write(out, &format!("{prefix}.expression.bin"), expression);
        if let Some((profile, _)) = &explicit {
            write(out, &format!("{prefix}.source-uri.bin"), &profile.uri);
            write(
                out,
                &format!("{prefix}.source-kind.bin"),
                if profile.contents.is_none() {
                    "uri"
                } else if profile.uri == "repl:text" {
                    "repl"
                } else {
                    "contents"
                },
            );
            if let Some(contents) = &profile.contents {
                write(
                    out,
                    &format!("{prefix}.source-contents.bin"),
                    contents.as_ref(),
                );
            }
        }
        write(
            out,
            &format!("{prefix}.counters-before.bin"),
            counters(request_index, imports, resolves, events),
        );
        text(out, &prefix, "stdout", &[]);
        text(out, &prefix, "stderr", &[]);
        let mut settled = false;
        let started = if let Some((profile, _)) = &explicit {
            probe.evaluate_module_source(
                &profile.uri,
                profile.contents.as_deref(),
                expression,
                0,
                None,
            )
        } else if input.public_lifecycle {
            probe.evaluate_module_source(uri, None, expression, 0, None)
        } else {
            probe.start_path(index as u32, 0, None)
        };
        if let Err(e) = started {
            write(out, &format!("{prefix}.status.bin"), "admission-error\n");
            text(
                out,
                &prefix,
                "stderr",
                &e.message.encode_utf16().collect::<Vec<_>>(),
            );
            settled = true;
        } else {
            for _ in 0..1_000_000 {
                let e = match probe.poll(0, 1) {
                    Ok(e) => e,
                    Err(e) => {
                        write(out, &format!("{prefix}.status.bin"), "poll-error\n");
                        text(
                            out,
                            &prefix,
                            "stderr",
                            &e.message.encode_utf16().collect::<Vec<_>>(),
                        );
                        settled = true;
                        break;
                    }
                };
                probe.collect();
                collections += 1;
                match e.kind {
                    "yield" => {}
                    "module-request" => {
                        let token = e.token.expect("actual request token");
                        let request = probe
                            .module_request(&token)
                            .expect("typed pending module request");
                        let req = format!("module-{request_index}");
                        write(out, &format!("{req}.token.bin"), &token);
                        write(out, &format!("{req}.id.bin"), request.id.to_string());
                        write(
                            out,
                            &format!("{req}.request.bin"),
                            format!("{:?}", request.kind),
                        );
                        write(out, &format!("{req}.api-index.bin"), index.to_string());
                        write(out, &format!("{req}.policy.bin"), &mode);
                        // These events arise only from actual typed Host requests. No primary, repl, factory, secure-path or API-outcome event is synthesized.
                        match &request.kind {
                            ModuleRequestKind::ResolveImportUri { .. } => {
                                write(out, &format!("{req}.kind.bin"), "resolve-import-uri")
                            }
                            ModuleRequestKind::ResolveKey { .. } => {
                                resolves += 1;
                                write(out, &format!("{req}.kind.bin"), "resolve-key")
                            }
                            ModuleRequestKind::SelectKey { .. } => {
                                write(out, &format!("{req}.kind.bin"), "select-key")
                            }
                            ModuleRequestKind::QueryCachePolicy { .. } => {
                                write(out, &format!("{req}.kind.bin"), "query-cache-policy")
                            }
                            ModuleRequestKind::QueryFragmentPaths { .. } => {
                                write(out, &format!("{req}.kind.bin"), "query-fragment-paths")
                            }
                            ModuleRequestKind::CheckImport {
                                importer,
                                import_uri,
                            } => {
                                imports += 1;
                                write(out, &format!("{req}.kind.bin"), "check-import");
                                write(
                                    out,
                                    &format!("{req}.importer-original-uri.bin"),
                                    importer.original_uri.as_ref(),
                                );
                                write(
                                    out,
                                    &format!("{req}.importer-resolved-uri.bin"),
                                    importer.resolved_uri.as_ref(),
                                );
                                write(out, &format!("{req}.import-uri.bin"), import_uri.as_ref());
                                event(
                                    out,
                                    &mut events,
                                    &format!(
                                        "policy-import {} -> {import_uri}",
                                        importer.resolved_uri
                                    ),
                                );
                            }
                            ModuleRequestKind::Resolve {
                                importer,
                                import_uri,
                                original_uri,
                                ..
                            } => {
                                resolves += 1;
                                write(out, &format!("{req}.kind.bin"), "resolve");
                                write(
                                    out,
                                    &format!("{req}.importer-original-uri.bin"),
                                    importer.original_uri.as_ref(),
                                );
                                write(
                                    out,
                                    &format!("{req}.importer-resolved-uri.bin"),
                                    importer.resolved_uri.as_ref(),
                                );
                                write(out, &format!("{req}.import-uri.bin"), import_uri.as_ref());
                                write(
                                    out,
                                    &format!("{req}.original-uri.bin"),
                                    original_uri.as_ref(),
                                );
                                event(
                                    out,
                                    &mut events,
                                    &format!("policy-resolve-module {import_uri}"),
                                );
                            }
                            ModuleRequestKind::LoadSource { resolved, .. } => {
                                write(out, &format!("{req}.kind.bin"), "source");
                                write(
                                    out,
                                    &format!("{req}.original-uri.bin"),
                                    resolved.original_uri.as_ref(),
                                );
                                write(
                                    out,
                                    &format!("{req}.resolved-uri.bin"),
                                    resolved.resolved_uri.as_ref(),
                                );
                                write(
                                    out,
                                    &format!("{req}.source-uri.bin"),
                                    resolved.source_uri.as_ref(),
                                );
                            }
                            ModuleRequestKind::Capabilities { .. } => {
                                write(out, &format!("{req}.kind.bin"), "capabilities")
                            }
                            ModuleRequestKind::HasElement { .. } => {
                                write(out, &format!("{req}.kind.bin"), "has-element")
                            }
                        }
                        if let Some(completion) = denial(&request, &mode, inputs) {
                            write(
                                out,
                                &format!("{req}.completion.bin"),
                                format!("{:?}", completion),
                            );
                            probe
                                .complete_module(&token, completion, 0)
                                .expect("typed captured policy failure");
                        } else if let Some(completion) = modules.complete(&request) {
                            write(
                                out,
                                &format!("{req}.completion.bin"),
                                format!("{completion:?}"),
                            );
                            probe
                                .complete_module(&token, completion, 0)
                                .expect("typed custom key completion");
                        } else if probe
                            .complete_default_module(&token, 0)
                            .expect("explicit shared default")
                        {
                            write(
                                out,
                                &format!("{req}.completion.bin"),
                                "shared-default-handled\n",
                            );
                        } else {
                            let completion = file_host::complete(&request);
                            write(
                                out,
                                &format!("{req}.completion.bin"),
                                format!("{:?}", completion),
                            );
                            probe
                                .complete_module(&token, completion, 0)
                                .expect("typed file/unsupported completion");
                        }
                        request_index += 1;
                    }
                    "value" | "error" => {
                        let (status, channel, units) = if e.kind == "value" {
                            ("success\n", "stdout", e.value.expect("value units"))
                        } else {
                            ("error\n", "stderr", e.error.expect("error units"))
                        };
                        write(out, &format!("{prefix}.status.bin"), status);
                        text(out, &prefix, channel, &units);
                        write(out, &format!("{prefix}.phase.bin"), e.phase.unwrap_or(""));
                        write(
                            out,
                            &format!("{prefix}.disposition.bin"),
                            e.disposition.unwrap_or(""),
                        );
                        settled = true;
                        break;
                    }
                    other => {
                        write(out, &format!("{prefix}.status.bin"), "transport-error\n");
                        write(out, &format!("{prefix}.unexpected-event.bin"), other);
                        settled = true;
                        break;
                    }
                }
            }
        }
        if !settled {
            write(out, &format!("{prefix}.status.bin"), "poll-bound\n");
        }
        write(
            out,
            &format!("{prefix}.counters-after.bin"),
            counters(request_index, imports, resolves, events),
        );
        index += 1;
    }
    probe.close();
    write(out, "terminal-slots.bin", probe.collect().slots.to_string());
    write(out, "collections.bin", collections.to_string());
    write(
        out,
        "counters.bin",
        counters(request_index, imports, resolves, events),
    );
}
fn main() {
    let a = std::env::args().collect::<Vec<_>>();
    assert_eq!(
        a.len(),
        11,
        "usage: stage49_modulekey_probe SOURCE URI NAME OPERATIONS OUTPUT SESSIONS POLICY CAUSES PROFILE MODULE_PLAN"
    );
    assert!(matches!(
        a[9].as_str(),
        "provided-primary-modulekey-v1"
            | "public-module-source-uri-v1"
            | "public-module-source-extended-v1"
    ));
    let source = fs::read_to_string(&a[1]).expect("source");
    let operations = fs::read_to_string(&a[4])
        .expect("operations")
        .lines()
        .map(|line| {
            let (kind, value) = line.split_once('\t').expect("operation fields");
            (kind.to_owned(), utf8(value))
        })
        .collect::<Vec<_>>();
    let inputs = causes(Path::new(&a[8]));
    let output = Path::new(&a[5]);
    fs::create_dir(output).expect("fresh output");
    for session in 0..a[6].parse::<usize>().expect("session count") {
        capture(
            &output.join(format!("session-{session}")),
            &CaptureInput {
                source: &source,
                uri: &a[2],
                name: &a[3],
                operations: &operations,
                initial_policy: &a[7],
                causes: &inputs,
                modules: Path::new(&a[10]),
                public_lifecycle: a[9].starts_with("public-module-source-"),
                profile: &a[9],
            },
        );
    }
}
