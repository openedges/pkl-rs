//! Development renderer API capture. Exact authored reader table; no Host resolver or cache.
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
    reader_plan: &HashMap<String, (String, String)>,
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
    let shared_resource_bytes: Arc<[u8]> = Arc::from([1u8, 2, 3]);
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
                    "read" => {
                        reads += 1;
                        let token = event.token.expect("read token");
                        let requested = probe.resolved_read_uri(&token).expect("resolved read URI");
                        write(out, &format!("read-{}.uri.bin", reads - 1), &requested);
                        let attempt = attempts.entry(requested.clone()).or_default();
                        *attempt += 1;
                        write(
                            out,
                            &format!("read-{}.attempt.bin", reads - 1),
                            attempt.to_string(),
                        );
                        let (kind, payload) = reader_plan
                            .get(&requested)
                            .expect("unconfigured reader URI: no fallback");
                        let result = if kind == "shared-resource" {
                            write(out, &format!("read-{}.kind.bin", reads - 1), "resource");
                            write(
                                out,
                                &format!("read-{}.resource-uri.bin", reads - 1),
                                &requested,
                            );
                            write(
                                out,
                                &format!("read-{}.bytes.bin", reads - 1),
                                &shared_resource_bytes,
                            );
                            Ok(ReadResult::Resource(Resource {
                                uri: requested.clone().into(),
                                bytes: Arc::clone(&shared_resource_bytes),
                            }))
                        } else if kind == "missing" {
                            write(out, &format!("read-{}.kind.bin", reads - 1), "missing");
                            Ok(ReadResult::Missing)
                        } else if kind == "io-first" && *attempt == 1 {
                            write(out, &format!("read-{}.kind.bin", reads - 1), "io");
                            write(
                                out,
                                &format!("read-{}.message.bin", reads - 1),
                                "renderer-reader-first",
                            );
                            Err(ReadFailure::Io(IoFailure {
                                message: Some("renderer-reader-first".into()),
                            }))
                        } else {
                            assert!(kind == "text" || kind == "io-first", "unknown reader kind");
                            let bytes = fs::read(payload_root.join(payload))
                                .expect("authored UTF16BE payload");
                            assert_eq!(bytes.len() % 2, 0, "even UTF16BE payload");
                            let units = bytes
                                .as_chunks::<2>()
                                .0
                                .iter()
                                .map(|pair| u16::from_be_bytes(*pair))
                                .collect::<Vec<_>>();
                            write(out, &format!("read-{}.kind.bin", reads - 1), "text");
                            save_text(out, &format!("read-{}", reads - 1), "value", &units);
                            Ok(ReadResult::Text(PklString::from_utf16(units)))
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
        "usage: stage47_renderer_probe SOURCE URI MODULE_NAME EXPRESSIONS OUTPUT WORKSPACE SESSIONS"
    );
    let source = fs::read_to_string(&args[1]).expect("source");
    let expressions = fs::read_to_string(&args[4])
        .expect("expressions")
        .lines()
        .map(str::to_owned)
        .collect::<Vec<_>>();
    let mut reader_plan = HashMap::new();
    for line in fs::read_to_string(Path::new(&args[6]).join("reader.tsv"))
        .expect("reader table")
        .lines()
    {
        let fields = line.split('\t').collect::<Vec<_>>();
        assert_eq!(fields.len(), 3, "reader URI/kind/payload");
        assert!(
            reader_plan
                .insert(
                    fields[0].to_owned(),
                    (fields[1].to_owned(), fields[2].to_owned())
                )
                .is_none(),
            "duplicate reader URI"
        );
    }
    for index in 0..args[7].parse::<usize>().expect("session count") {
        capture(
            &Path::new(&args[5]).join(format!("session-{index}")),
            &source,
            &args[2],
            &args[3],
            &expressions,
            Path::new(&args[6]),
            &reader_plan,
        );
    }
}
