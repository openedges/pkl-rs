//! Development typed Resource capture; no URI normalization or resource cache in this Host.
use pkl_runtime::{
    host::{IoFailure, ReadFailure},
    probe::SourceProbe,
    resources::{ReadResult, Resource},
    value::PklString,
};
use std::{collections::HashMap, fs, path::Path, sync::Arc};

fn write(out: &Path, name: &str, value: impl AsRef<[u8]>) {
    fs::write(out.join(name), value).expect("write annotation capture");
}
fn save_text(out: &Path, prefix: &str, channel: &str, units: &[u16]) {
    write(
        out,
        &format!("{prefix}.{channel}.code-units.bin"),
        units
            .iter()
            .flat_map(|unit| unit.to_be_bytes())
            .collect::<Vec<_>>(),
    );
    write(
        out,
        &format!("{prefix}.{channel}.bin"),
        PklString::from_utf16(units.to_vec()).to_java_utf8(),
    );
}
fn capture(
    out: &Path,
    source: &str,
    uri: &str,
    name: &str,
    expressions: &[String],
    payload_root: &Path,
) {
    fs::create_dir_all(out).expect("create annotation session output");
    let mut probe = match SourceProbe::new_expressions_text(
        source,
        uri,
        name,
        expressions,
        "compact",
        false,
        false,
        true,
    ) {
        Ok(probe) => probe,
        Err(error) => {
            save_text(
                out,
                "constructor",
                "stderr",
                &error.encode_utf16().collect::<Vec<_>>(),
            );
            write(out, "constructor.status.bin", "constructor-error\n");
            return;
        }
    };
    write(
        out,
        "module-policy-profile.bin",
        "shared-default-import-trust-and-bundled-resolution-v1\n",
    );
    let mut module_requests = 0usize;
    let mut reads = 0usize;
    let mut attempts = HashMap::<String, usize>::new();
    let mut collections = 0usize;
    for (index, expression) in expressions.iter().enumerate() {
        let prefix = format!("outer-{index}");
        write(out, &format!("{prefix}.expression.bin"), expression);
        write(
            out,
            &format!("{prefix}.reads-before.bin"),
            reads.to_string(),
        );
        save_text(out, &prefix, "stdout", &[]);
        save_text(out, &prefix, "stderr", &[]);
        if let Err(error) = probe.start_path(index as u32, 0, None) {
            write(out, &format!("{prefix}.status.bin"), "admission-error\n");
            write(
                out,
                &format!("{prefix}.error-kind.bin"),
                format!("{:?}", error.kind),
            );
            save_text(
                out,
                &prefix,
                "stderr",
                &error.message.encode_utf16().collect::<Vec<_>>(),
            );
        } else {
            let mut settled = false;
            for _ in 0..1_000_000 {
                let event = match probe.poll(0, 1) {
                    Ok(event) => event,
                    Err(error) => {
                        write(out, &format!("{prefix}.status.bin"), "poll-error\n");
                        write(
                            out,
                            &format!("{prefix}.error-kind.bin"),
                            format!("{:?}", error.kind),
                        );
                        save_text(
                            out,
                            &prefix,
                            "stderr",
                            &error.message.encode_utf16().collect::<Vec<_>>(),
                        );
                        settled = true;
                        break;
                    }
                };
                probe.collect();
                collections += 1;
                match event.kind {
                    "yield" => {}
                    "module-request" => {
                        let token = event.token.as_deref().expect("module token");
                        let request = probe.module_request(token).expect("typed module request");
                        write(
                            out,
                            &format!("module-{module_requests}.request.bin"),
                            format!("{request:?}"),
                        );
                        module_requests += 1;
                        assert!(
                            probe
                                .complete_default_module(token, 0)
                                .expect("default module transport"),
                            "unsupported request in explicitly configured default module profile"
                        );
                    }
                    "read" => {
                        reads += 1;
                        let token = event.token.expect("read token");
                        let requested = probe.resolved_read_uri(&token).expect("resolved read URI");
                        write(out, &format!("read-{}.uri.bin", reads - 1), &requested);
                        let attempt = attempts.entry(requested.clone()).or_default();
                        *attempt += 1;
                        let result = if requested.starts_with("mem:") {
                            Ok(ReadResult::Resource(Resource {
                                uri: requested.clone().into(),
                                bytes: Arc::from(format!("payload-{reads}").into_bytes()),
                            }))
                        } else if let Some(index) = requested.strip_prefix("res:/b64-") {
                            let index: usize = index.parse().expect("captured b64 index");
                            let bytes =
                                fs::read(payload_root.join(format!("b64-{index}.units.bin")))
                                    .expect("captured UTF16 String reader input");
                            assert_eq!(bytes.len() % 2, 0);
                            let units = bytes
                                .as_chunks::<2>()
                                .0
                                .iter()
                                .map(|pair| u16::from_be_bytes(*pair))
                                .collect::<Vec<_>>();
                            Ok(ReadResult::Text(PklString::from_utf16(units)))
                        } else if requested.ends_with("/missing") {
                            Ok(ReadResult::Missing)
                        } else if requested.ends_with("/retry") && *attempt == 1 {
                            Err(ReadFailure::Io(IoFailure {
                                message: Some("reader-first".into()),
                            }))
                        } else if requested == "res:/resource" {
                            Ok(ReadResult::Resource(Resource {
                                uri: "res:/different-returned-uri".into(),
                                bytes: Arc::from([65u8, 13, 10, 0, 66]),
                            }))
                        } else if let Some(vector) = requested.strip_prefix("res:/vector") {
                            let number: usize = vector
                                .parse()
                                .expect("vector index supplied by observer input");
                            let bytes = fs::read(payload_root.join(format!("vector{number}.bin")))
                                .expect("captured resource bytes");
                            Ok(ReadResult::Resource(Resource {
                                uri: format!("res:/returned/vector{number}").into(),
                                bytes: Arc::from(bytes),
                            }))
                        } else {
                            Ok(ReadResult::Text(
                                format!("payload-{reads} {requested}").into(),
                            ))
                        };
                        if let Err(error) = probe.complete_read(&token, result, 0) {
                            write(out, &format!("{prefix}.status.bin"), "transport-error\n");
                            save_text(
                                out,
                                &prefix,
                                "stderr",
                                &error.message.encode_utf16().collect::<Vec<_>>(),
                            );
                            settled = true;
                            break;
                        }
                    }
                    "value" | "error" => {
                        let (status, channel, units) = if event.kind == "value" {
                            ("success\n", "stdout", event.value.expect("value units"))
                        } else {
                            ("error\n", "stderr", event.error.expect("error units"))
                        };
                        write(out, &format!("{prefix}.status.bin"), status);
                        save_text(out, &prefix, channel, &units);
                        write(
                            out,
                            &format!("{prefix}.phase.bin"),
                            event.phase.unwrap_or(""),
                        );
                        write(
                            out,
                            &format!("{prefix}.disposition.bin"),
                            event.disposition.unwrap_or(""),
                        );
                        write(
                            out,
                            &format!("{prefix}.reason.bin"),
                            event.reason.unwrap_or_default(),
                        );
                        settled = true;
                        break;
                    }
                    other => {
                        write(out, &format!("{prefix}.status.bin"), "transport-error\n");
                        write(out, &format!("{prefix}.transport-error.bin"), other);
                        settled = true;
                        break;
                    }
                }
            }
            if !settled {
                write(out, &format!("{prefix}.status.bin"), "poll-bound\n");
            }
        }
        write(out, &format!("{prefix}.reads-after.bin"), reads.to_string());
    }
    probe.close();
    write(out, "terminal-slots.bin", probe.collect().slots.to_string());
    write(out, "collections.bin", collections.to_string());
    write(out, "reads.bin", reads.to_string());
}
fn main() {
    let args = std::env::args().collect::<Vec<_>>();
    assert!(
        args.len() == 8,
        "usage: stage46_resource_probe SOURCE URI MODULE_NAME EXPRESSIONS OUTPUT PAYLOAD_ROOT SESSIONS"
    );
    let source = fs::read_to_string(&args[1]).expect("source");
    let expressions = fs::read_to_string(&args[4])
        .expect("expressions")
        .lines()
        .map(str::to_owned)
        .collect::<Vec<_>>();
    for index in 0..args[7].parse::<usize>().expect("session count") {
        capture(
            &Path::new(&args[5]).join(format!("session-{index}")),
            &source,
            &args[2],
            &args[3],
            &expressions,
            Path::new(&args[6]),
        );
    }
}
